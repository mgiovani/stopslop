use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::ProseDoc;
use crate::prose_words::{ADJACENT_HEDGE_STACK, HEDGE_PHRASES};
use crate::registry::RuleDef;
use crate::rules::fragmentation::earliest_qualifying;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP015",
    name: "Hedging & filler-phrase density",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

/// Brazilian-Portuguese twin of `HEDGE_PHRASES`. `[ée]`/`[íi]`/`[áa]` classes cover the
/// unaccented misspelling that's common online ("e importante ressaltar", "ate certo ponto") --
/// `(?i)` alone doesn't fold a missing accent, only case. `relativamente`/`praticamente`/
/// `basicamente`/`essencialmente` were tried and cut: on the pt-BR corpus (Wikipedia +
/// public-domain fiction) they read as ordinary adverbs, not hedges, and pushed several clean
/// articles over the density floor.
///
/// Split into two groups instead of one `\b(?:...)\b`: the ASCII-initial alternatives sit under
/// an ASCII-scoped boundary on both ends, while the two alternatives that open on the accented
/// "é" (`[ée] (importante|...) ...`, `[ée] poss[íi]vel que`) drop the leading boundary --
/// `(?-u:\b)` right before an accented letter never matches -- and keep only the trailing one,
/// since every one of those two ends in an ASCII letter (`r`, `e`).
static HEDGE_PHRASES_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:(?-u:\b)(?:vale (?:a pena )?(?:ressaltar|destacar|lembrar|mencionar|notar)|de certa forma|de certo modo|em certa medida|at[ée] certo ponto|de alguma (?:forma|maneira)|a princ[íi]pio|em tese|na maioria dos casos|pode(?:-se)? dizer que|de um modo geral|de maneira geral|via de regra|tende a ser|um tanto(?: quanto)?)(?-u:\b)|(?:[ée] (?:importante|v[áa]lido|interessante) (?:ressaltar|destacar|notar|lembrar|mencionar|observar)|[ée] poss[íi]vel que)(?-u:\b))",
    )
    .unwrap()
});

/// Brazilian-Portuguese twin of `ADJACENT_HEDGE_STACK`. Same independent-of-density gate: a
/// stacked hedge is a defect on a single occurrence.
static ADJACENT_HEDGE_STACK_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?-u:\b)(?:pode(?:ria)? (?:potencialmente|possivelmente|talvez|eventualmente)|talvez (?:possivelmente|potencialmente|eventualmente)|provavelmente (?:talvez|possivelmente)|em geral,? geralmente)(?-u:\b)",
    )
    .unwrap()
});

/// Counts hedging/filler-phrase occurrences in the masked prose stream (headings in scope,
/// frontmatter and URLs skipped). A single "in conclusion" or "it's worth noting" is completely
/// normal in human writing, so this never fires on one hit. Flags once, document-level,
/// anchored at the first occurrence, when the total N meets the density floor
/// (`N >= max(3, ceil(3 * words / 1000))`) OR any single phrase repeats `>= 2` times. Separately,
/// below, an adjacent hedge stack ("might potentially") fires on every match regardless of
/// density -- see the comment at that loop for why it can't share this gate.
///
/// Two natural-language panels (English, Portuguese) feed one tally: under the default union
/// each runs when its language is enabled and every hit -- regardless of which panel matched --
/// merges into the same `total`, `per_phrase`, and `first_byte` (by min, not "whichever panel
/// ran first"; see filler.rs's FILLER_PHRASES/FILLER_ADVERBS merge for the same idiom).
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let mut total = 0usize;
    let mut first_byte: Option<usize> = None;
    let mut per_phrase: HashMap<String, (usize, usize)> = HashMap::new();

    let en = ctx.natlangs.contains(&NatLang::En);
    let pt_br = ctx.natlangs.contains(&NatLang::PtBr);

    if en {
        count_hedges(
            doc,
            &HEDGE_PHRASES,
            &mut total,
            &mut first_byte,
            &mut per_phrase,
        );
    }
    if pt_br {
        count_hedges(
            doc,
            &HEDGE_PHRASES_PT_BR,
            &mut total,
            &mut first_byte,
            &mut per_phrase,
        );
    }

    // Integer ceil of 3 * words / 1000, floored at an absolute minimum of 3.
    let threshold = (3 * doc.words).div_ceil(1000).max(3);
    // The two trigger conditions need different wording: "N occurrences vs threshold N" reads as
    // a failed comparison when total < threshold and it's really the phrase-repeated branch that
    // fired (e.g. total=2, threshold=3), so branch the message on which condition actually fired.
    let repeated_phrase = earliest_qualifying(&per_phrase, 2);
    if total >= threshold || repeated_phrase.is_some() {
        let (line, col) = doc.line_col(first_byte.unwrap());
        let message = if total >= threshold {
            format!(
                "high density of hedging/filler phrases ({total} occurrences vs threshold {threshold})"
            )
        } else {
            let phrase = repeated_phrase.unwrap();
            format!("hedging/filler phrase repeated: \"{phrase}\" appears multiple times")
        };
        out.push(Diagnostic::at(rule, ctx, line, col, message));
    }

    // Checked outside the density gate, on every match: that gate exists to avoid flagging one
    // ordinary hedge, so a doubled-up hedge would get averaged away rather than surfaced.
    // Stacking two hedges is a defect regardless of context.
    if en {
        push_adjacent_stacks(doc, &ADJACENT_HEDGE_STACK, rule, ctx, out);
    }
    if pt_br {
        push_adjacent_stacks(doc, &ADJACENT_HEDGE_STACK_PT_BR, rule, ctx, out);
    }
}

/// One language panel's contribution to the document-level tally: every match outside
/// frontmatter/URLs bumps `total`, tracks the earliest byte in `first_byte`, and tallies its
/// lowercased text in `per_phrase` for the repeated-phrase branch.
fn count_hedges(
    doc: &ProseDoc,
    re: &Regex,
    total: &mut usize,
    first_byte: &mut Option<usize>,
    per_phrase: &mut HashMap<String, (usize, usize)>,
) {
    for m in re.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) {
            continue;
        }
        *total += 1;
        *first_byte = Some(first_byte.map_or(byte, |b| b.min(byte)));
        let entry = per_phrase
            .entry(m.as_str().to_lowercase())
            .or_insert((0, byte));
        entry.0 += 1;
        entry.1 = entry.1.min(byte);
    }
}

/// One language panel's adjacent-hedge-stack diagnostics, pushed on every match regardless of
/// the density gate (see the comment at its call site).
fn push_adjacent_stacks(
    doc: &ProseDoc,
    re: &Regex,
    rule: &'static RuleDef,
    ctx: &LintContext,
    out: &mut Vec<Diagnostic>,
) {
    for m in re.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) {
            continue;
        }
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            format!(
                "adjacent hedge stack: \"{}\" hedges twice in a row",
                m.as_str()
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        diagnostics_for_natlangs(src, crate::lang::ALL_NATLANGS)
    }

    fn diagnostics_for_natlangs(src: &str, natlangs: &[NatLang]) -> Vec<Diagnostic> {
        let doc = ProseDoc::parse(src);
        let ctx = LintContext {
            display_path: "test.md".to_string(),
            source: src,
            index: None,
            lang: Lang::Md,
            comments: &doc.ignore_comments,
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: Some(&doc),
            image: None,
            natlangs,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn flags_three_distinct_hedges() {
        let src = "It's important to note that caching reduces database load significantly.\n\nIn conclusion, the cache should be enabled for all production services.\n\nFirst and foremost, monitor hit rates before tuning eviction policies.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP015");
    }

    #[test]
    fn flags_repeated_single_phrase() {
        // total=2 < threshold=3: this only fires via the repeated-phrase branch, so the message
        // must not claim a "vs threshold" comparison that reads as failed (2 < 3).
        let src = "In conclusion, ship it. Later, in conclusion, ship it again.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("repeated"));
        assert!(!diags[0].message.contains("vs threshold"));
    }

    #[test]
    fn clean_two_hedges_below_floor() {
        // N=2, no repeated phrase -> below the absolute floor of 3, must not fire.
        let src =
            "It's worth noting that latency improved. In conclusion, the rollout succeeded.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_single_adjacent_hedge_stack_independent_of_density() {
        // A single hedge, nowhere near the density floor -- must still fire via the separate
        // adjacent-stack path.
        let src = "This change might potentially fail under sustained load.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP015");
        assert!(diags[0].message.contains("adjacent hedge stack"));
    }

    #[test]
    fn clean_non_adjacent_hedge_words() {
        // "could" and "possibly" both appear, but not stacked back to back -- ordinary hedging,
        // not the doubled-up shape this path targets.
        let src = "This could work, but possibly not for every workload.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_three_distinct_pt_br_hedges() {
        let src = "Vale ressaltar que o cache reduz bastante a carga no banco.\n\nDe certa forma, o sistema ficou mais previsível depois da mudança.\n\nEm tese, a nova versão deveria resolver o problema de latência.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP015");
    }

    #[test]
    fn flags_repeated_single_pt_br_phrase() {
        let src = "Em tese, o serviço deveria escalar sozinho. Mais tarde, em tese, o mesmo problema apareceu de novo.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("repeated"));
    }

    #[test]
    fn flags_pt_br_adjacent_hedge_stack() {
        let src = "Essa mudança pode potencialmente falhar sob carga sustentada.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("adjacent hedge stack"));
    }

    #[test]
    fn flags_merged_pt_br_and_en_hedges_in_one_tally() {
        // One Portuguese hedge + two English hedges in one short document: the union default
        // means both panels run, and they must merge into ONE total (3), not two separate
        // per-language tallies -- three hits clears the floor of 3.
        let src = "Vale ressaltar que o serviço caiu ontem. In conclusion, we rolled it back. It's worth noting that the rollback fixed it.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("3 occurrences"));
    }

    #[test]
    fn anchor_is_earliest_hedge_regardless_of_language() {
        // The English hedge is textually first; the anchor must land there even though the
        // Portuguese panel runs in a separate loop after the English one.
        let src = "It's worth noting that latency improved. Vale ressaltar que o time comemorou. Em tese, o problema está resolvido.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 1);
    }

    #[test]
    fn clean_pt_br_paragraph_without_hedges() {
        // "de fato" and "certamente" assert rather than hedge -- neither is in the panel.
        let src = "De fato, o novo índice reduziu o tempo de consulta pela metade. A equipe certamente vai monitorar o comportamento em produção pelas próximas semanas antes de expandir o rollout para os demais serviços.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// Covers every `HEDGE_PHRASES_PT_BR` alternative not already exercised above, split across
    /// the two boundary groups the panel is restructured into. Each phrase appears three times so
    /// the density floor (not the repeated-phrase branch) is what fires.
    #[test]
    fn flags_each_untested_pt_br_hedge_phrase() {
        let phrases = [
            "é importante destacar",
            "é possível que",
            "a princípio",
            "via de regra",
            "tende a ser",
            "um tanto quanto",
            "na maioria dos casos",
            "pode-se dizer que",
        ];
        for phrase in phrases {
            let src = format!(
                "O time notou que, {phrase}, o comportamento mudou um pouco depois do deploy.\n\nMais tarde, {phrase}, a mesma conclusão apareceu de novo no relatório da semana.\n\nNo fechamento, {phrase}, ninguém contestou o resultado observado em produção.\n"
            );
            let diags = diagnostics_for(&src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {phrase}");
            assert_eq!(diags[0].code, "SLOP015");
        }
    }

    #[test]
    fn natlang_gate_silences_the_other_languages_panel() {
        let pt_positive = "Essa mudança pode potencialmente falhar sob carga sustentada.\n";
        assert!(diagnostics_for_natlangs(pt_positive, &[NatLang::En]).is_empty());

        let en_positive = "This change might potentially fail under sustained load.\n";
        assert!(diagnostics_for_natlangs(en_positive, &[NatLang::PtBr]).is_empty());
    }

    /// Two DIFFERENT hedge phrases both repeat (>=2) -- exactly the shape that used to read a
    /// `HashMap`'s iteration order non-deterministically. Padded well past the density floor
    /// (which scales with word count) so this exercises the repeated-phrase branch, not the
    /// density branch: the message must always name "in conclusion" (textually first), never
    /// "it's worth noting" (textually later), regardless of `HashMap` iteration order.
    #[test]
    fn repeated_phrase_message_names_the_earliest_occurrence() {
        let filler =
            "The gardener watered the young trees every quiet morning before sunrise. ".repeat(160);
        let src = format!(
            "In conclusion, ship it now. {filler}It's worth noting that shipping went fine. {filler}In conclusion, ship it again. It's worth noting that we monitored it closely.\n"
        );
        let diags = diagnostics_for(&src);
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("repeated"),
            "{}",
            diags[0].message
        );
        assert!(
            diags[0].message.contains("\"in conclusion\""),
            "message was: {}",
            diags[0].message
        );
    }
}
