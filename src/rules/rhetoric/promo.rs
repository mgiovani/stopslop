use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::ProseDoc;
use crate::registry::RuleDef;
use crate::rules::rhetoric::fragmentation::earliest_qualifying;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP031",
    name: "Promotional / advertisement language",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

// "boasts" and "game[-]changer" are omitted: VOCAB_TIER1 (SLOP016) and CLICHE_PHRASES (SLOP014)
// already match them, so including them here would double-flag. Only the untaken
// "game-changing" inflection stays.
static PROMO_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(nestled (?:in|among|between)|in the heart of|renowned for|world-renowned|breathtaking|must-visit|must-have|stunning|state-of-the-art|best-in-class|industry-leading|award-winning|unparalleled|unrivaled|second to none|a commitment to excellence|natural beauty|a hidden gem|one-stop shop|game-changing|next-generation|turnkey|rich (?:history|heritage|tradition))(?-u:\b)")
        .unwrap()
});

/// Brazilian-Portuguese twin of `PROMO_PHRASES`. Measured against a 318-document,
/// 1.3-million-word human corpus and dropped above 2 hits: `premiado` (5 human hits), `no coração
/// de` (3), `de última geração` (2), `incomparável` (2), `inigualável` (1, kept out with
/// `incomparável`), `imperdível` (1), `rica história/tradição/herança` (3), `aninhad[oa]
/// (?:em|entre|no|na)` (3 human hits -- "aninhado em" is "nested in", a technical term in the
/// translated Python docs) -- every one is ordinary Portuguese description, not brochure
/// register. Same threshold and merged tally as English.
static PROMO_PHRASES_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:renomad[oa] p(?:or|elo|ela)|mundialmente (?:renomad|conhecid|famos)[oa]s?|de tirar o f[ôo]lego|deslumbrante|o melhor da categoria|l[íi]der (?:do|no|de) (?:setor|mercado|segmento)|sem igual|compromisso com a excel[êe]ncia|beleza natural|uma j[óo]ia escondida|solu[çc][ãa]o completa|pr[óo]xima gera[çc][ãa]o|pronto para uso|experi[êe]ncia [úu]nica|transforme (?:sua|seu|a sua|o seu)|para o pr[óo]ximo n[íi]vel|n[ãa]o perca|garanta (?:sua|seu|o seu|a sua)|vagas limitadas|resultados (?:reais|comprovados|garantidos))(?-u:\b)")
        .unwrap()
});

/// `garanta já` ends on the accented `á`, where the trailing `(?-u:\b)` never matches (the next
/// char is whatever follows in prose, e.g. a comma, and `á`'s own trailing byte is not an ASCII
/// word byte either -- see AGENTS.md's boundary trap); require the literal space or end-of-string
/// that follows instead of a boundary. Kept as its own static rather than folded into
/// `PROMO_PHRASES_PT_BR`'s trailing-`(?-u:\b)` group, whose other alternatives all end on plain
/// ASCII letters.
static PROMO_GARANTA_JA_PT_BR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?-u:\b)garanta j[áa](?:\s|$)").unwrap());

/// Density rule over the masked prose stream (headings in scope, frontmatter and URLs
/// skipped), same shape as `hedging.rs` (SLOP015): fires once, document-level, anchored at the
/// first in-scope occurrence, when the total N meets the density floor
/// (`N >= max(3, ceil(3 * words / 1000))`) OR any single phrase repeats `>= 2` times.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let mut total = 0usize;
    let mut first_byte: Option<usize> = None;
    let mut per_phrase: HashMap<String, (usize, usize)> = HashMap::new();

    if ctx.natlangs.contains(&NatLang::En) {
        count_promo(
            doc,
            &PROMO_PHRASES,
            &mut total,
            &mut first_byte,
            &mut per_phrase,
        );
    }
    if ctx.natlangs.contains(&NatLang::PtBr) {
        count_promo(
            doc,
            &PROMO_PHRASES_PT_BR,
            &mut total,
            &mut first_byte,
            &mut per_phrase,
        );
        count_promo(
            doc,
            &PROMO_GARANTA_JA_PT_BR,
            &mut total,
            &mut first_byte,
            &mut per_phrase,
        );
    }

    let threshold = (3 * doc.words).div_ceil(1000).max(3);
    let repeated_phrase = earliest_qualifying(&per_phrase, 2);
    if total >= threshold || repeated_phrase.is_some() {
        let (line, col) = doc.line_col(first_byte.unwrap());
        let message = if total >= threshold {
            format!(
                "high density of promotional/advertisement phrases ({total} occurrences vs threshold {threshold})"
            )
        } else {
            let phrase = repeated_phrase.unwrap();
            format!("promotional phrase repeated: \"{phrase}\" appears multiple times")
        };
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            message,
            "state the measurable property instead",
        ));
    }
}

/// One language panel's contribution to the document-level tally: every match outside
/// frontmatter/URLs bumps `total`, tracks the earliest byte in `first_byte` by min (not "first
/// encountered" -- `check` can merge more than one pass, so the earliest match overall may come
/// from any of them), and tallies its lowercased text in `per_phrase`.
fn count_promo(
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
            natlangs,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn flags_three_distinct_promo_phrases() {
        let src = "The venue is nestled among rolling hills.\n\nOur platform is state-of-the-art and built for scale.\n\nThe new release is truly a hidden gem for power users.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP031");
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("state the measurable property instead")
        );
    }

    #[test]
    fn flags_repeated_single_phrase() {
        let src = "Our tool is state-of-the-art. Later, the API is also state-of-the-art.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("repeated"));
    }

    #[test]
    fn clean_two_promo_hits_below_floor() {
        let src = "The dashboard is stunning at a glance. It is also turnkey for new teams.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_duplicate_vocabulary_boast_span() {
        // "boasts an?" is dropped from this panel: VOCAB_TIER1 (SLOP016) already matches bare
        // "boast(s|ed|ing)?", so this rule must stay silent even on a promotional-sounding
        // "boasts a" construction.
        let src =
            "The service boasts a fast API. The service boasts a fast API. The service boasts a fast API.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_duplicate_cliche_game_changer_span() {
        // "game changer"/"game-changer" is CLICHE_PHRASES (SLOP014) territory; only the
        // "-changing" inflection belongs to this panel.
        let src = "This release is a real game-changer for the team.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_each_pt_br_promo_phrase_alternative() {
        let cases = [
            "O restaurante é renomado por sua cozinha tradicional.",
            "O chef é mundialmente conhecido por seus pratos autorais.",
            "A vista do topo é de tirar o fôlego ao amanhecer.",
            "A paisagem é deslumbrante durante o pôr do sol.",
            "Nosso atendimento é o melhor da categoria na região.",
            "A empresa é líder do setor de logística no país.",
            "O serviço oferecido é sem igual em toda a cidade.",
            "Temos um compromisso com a excelência em cada entrega.",
            "A reserva preserva a beleza natural da mata local.",
            "Esse bairro é uma jóia escondida no centro da cidade.",
            "Oferecemos uma solução completa para pequenas empresas.",
            "O produto representa a próxima geração de sensores industriais.",
            "O painel vem pronto para uso assim que chega à loja.",
            "A trilha oferece uma experiência única para os visitantes.",
            "Transforme sua rotina com o novo aplicativo de finanças.",
            "Leve o seu negócio para o próximo nível ainda este ano.",
            "Não perca a promoção de lançamento nesta semana.",
            "Garanta sua vaga na turma de outubro antes que esgote.",
            "As vagas limitadas para o curso terminam nesta sexta-feira.",
            "Os resultados comprovados do tratamento atraem novos clientes.",
        ];
        for s in cases {
            assert!(PROMO_PHRASES_PT_BR.is_match(s), "{s}");
        }
    }

    #[test]
    fn flags_pt_br_garanta_ja() {
        assert!(PROMO_GARANTA_JA_PT_BR.is_match("Garanta já o seu ingresso para o show."));
    }

    #[test]
    fn flags_three_distinct_pt_br_promo_hits_at_threshold() {
        // 3 distinct pt-BR promo phrases, matching the density floor of 3 exactly.
        let src = "O restaurante é renomado por sua culinária local.\n\nA vista é de tirar o fôlego ao amanhecer.\n\nA decoração é deslumbrante em todos os detalhes.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP031");
        assert!(diags[0].message.contains("vs threshold"));
    }

    /// Every one of these has human hits in the pt-BR corpus (see `PROMO_PHRASES_PT_BR`'s doc
    /// comment) and stays out of the panel.
    #[test]
    fn clean_pt_br_dropped_promo_shapes() {
        let dropped = [
            "O projeto foi premiado na conferência do ano passado.",
            "O escritório fica no coração de São Paulo.",
            "O sensor usa tecnologia de última geração no chip.",
            "A vista do mirante é incomparável ao entardecer.",
            "O sabor do prato é inigualável entre os visitantes.",
            "O show de sexta-feira é imperdível para os fãs.",
            "A cidade tem uma rica história cultural desde o século XIX.",
            "A região mantém uma rica tradição de festas populares.",
            "O resort está aninhado entre as montanhas da região serrana.",
        ];
        for s in dropped {
            assert!(!PROMO_PHRASES_PT_BR.is_match(s), "{s}");
        }
    }

    #[test]
    fn natlang_gate_silences_the_other_languages_panel() {
        let pt_positive = "Nosso atendimento é o melhor da categoria. Nosso atendimento é o melhor da categoria. Nosso atendimento é o melhor da categoria.\n";
        assert!(diagnostics_for_natlangs(pt_positive, &[NatLang::En]).is_empty());

        let en_positive =
            "Our tool is state-of-the-art. Later, the API is also state-of-the-art.\n";
        assert!(diagnostics_for_natlangs(en_positive, &[NatLang::PtBr]).is_empty());
    }

    /// Two DIFFERENT promo phrases both repeat (>=2) -- exactly the shape that used to read a
    /// `HashMap`'s iteration order non-deterministically. Padded well past the density floor so
    /// this exercises the repeated-phrase branch: the message must always name
    /// "state-of-the-art" (textually first), never "a hidden gem" (textually later).
    #[test]
    fn repeated_phrase_message_names_the_earliest_occurrence() {
        let filler =
            "The gardener watered the young trees every quiet morning before sunrise. ".repeat(160);
        let src = format!(
            "Our platform is state-of-the-art and built for scale. {filler}The new release is truly a hidden gem for power users. {filler}Our platform stays state-of-the-art after the update. The new release remains a hidden gem for power users.\n"
        );
        let diags = diagnostics_for(&src);
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("repeated"),
            "{}",
            diags[0].message
        );
        assert!(
            diags[0].message.contains("\"state-of-the-art\""),
            "message was: {}",
            diags[0].message
        );
    }
}
