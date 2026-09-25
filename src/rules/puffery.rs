use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::first_byte_per_line;
use crate::registry::RuleDef;
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP024",
    name: "Importance puffery / fake-strong verb",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

/// (a) Importance puffery: an inflated significance claim that states no fact ("marks a pivotal
/// moment", "solidifies its position", ...). "stands as a testament" and "plays a
/// vital/crucial role" are deliberately OMITTED from this panel even though the catalog groups
/// them under the same shape: `cliche.rs`'s CLICHE_PHRASES already matches "stands/serves as a
/// testament to" and `hedging.rs`'s HEDGE_PHRASES already matches "plays/play an
/// (vital|crucial|...) role in" -- including them here would double-flag the exact same span
/// under two rules. "reflects a broader shift" is likewise NOT re-added below: it was already
/// present in this exact panel from the start (`reflects? a broader shift|reflected a broader
/// shift`), so the catalog entry is a same-panel duplicate, not a new phrase.
static IMPORTANCE_PUFFERY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(marks? a pivotal moment|marked a pivotal moment|marks? a turning point|marked a turning point|solidif(?:ies|ied) its position|cements? its place|cemented its place|underscores? its significance|underscored its significance|represents? a significant milestone|represented a significant milestone|highlights? the importance of|highlighted the importance of|reflects? a broader shift|reflected a broader shift|is poised to revolutionize|are poised to revolutionize|sets? a new standard for|set a new standard for|is a reminder of|is a reminder that|symboliz(?:es|ing) its (?:ongoing|enduring|lasting)|setting the stage for|a key turning point|an indelible mark|is deeply rooted in|remains deeply rooted in|represents? a shift|marks? a shift|the focal point)(?-u:\b)")
        .unwrap()
});

/// Brazilian-Portuguese twin of `IMPORTANCE_PUFFERY_RE` only -- the (b)/(c) sub-patterns
/// (`FAKE_STRONG_VERB_RE`, `FAUX_SCALE_RANGE_RE`) stay English-only, see the corpus note below.
/// Measured against a 318-document, 1.3-million-word human corpus (Wikipedia + public-domain
/// fiction + translated Python docs) and dropped above 2 hits: `serve/atua/funciona como um` (14 human hits, 11 docs)
/// and `desde X até Y` (10 human hits) are ordinary Portuguese, not puffery -- only this
/// importance panel ships. `prepara o terreno para` is the measured signal (0 human hits, 6
/// generated hits across 5 documents). `é um lembrete de que` is written only as `serve como um
/// lembrete de que`: the bare `é` form starts on the accented letter, where the leading
/// `(?-u:\b)` can never sit. Trailing boundary: every alternative ends on an ASCII letter
/// (`posição` ends in `o`, `enraizada` in `a`), so the trailing `(?-u:\b)` is valid throughout.
static IMPORTANCE_PUFFERY_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:marc(?:a|ou|am) um (?:momento|ponto) (?:decisivo|crucial|de virada|de inflex[ãa]o)|representa(?:m)? um marco (?:significativo|importante|hist[óo]rico)|consolid(?:a|ou|am) (?:sua|a sua|a) posi[çc][ãa]o|(?:destaca|ressalta|refor[çc]a|evidencia)(?:m)? a import[âa]ncia d[eoa]|reflete(?:m)? uma mudan[çc]a (?:mais ampla|maior)|est[áa] prestes a revolucionar|estabelece(?:m)? um novo padr[ãa]o|serve como um lembrete de que|abre(?:m)? caminho para|prepar(?:a|ou|am) o terreno para|(?:est[áa]|permanece) profundamente enraizad[oa]|representa(?:m)? uma mudan[çc]a de paradigma|o ponto focal)(?-u:\b)")
        .unwrap()
});

/// (b) Fake-strong linking verb: "serves/acts/functions/stands/boasts as a/an <noun phrase>",
/// where a plain "is"/"has" would be clearer. Gated tightly on the article ("as a/an") so
/// idiomatic non-nominal uses ("serves as expected", "acts as designed", "functions as intended")
/// never match -- those have no article after "as" at all. Up to two leading modifier words
/// before the head noun covers "serves as a fully centralized hub"-style phrasing.
static FAKE_STRONG_VERB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?-u:\b)(?:serves?|acts?|functions?|stands?|boasts?) as an? (?:\w+ ){0,2}\w+(?-u:\b)",
    )
    .unwrap()
});

/// (c) Faux-scale range: "from the singularity of the Big Bang to the enigmatic dance of dark
/// matter"-style constructions. The detectable signal is the NESTED DOUBLE `of` -- one bare "from
/// X to Y" is ordinary English (a date range, a version range, a distance), but "from <phrase> of
/// <phrase> to <phrase> of <phrase>" is the specific poetic-pairing shape that stands in for an
/// actual magnitude claim. Each `<phrase>` slot allows an optional leading "the" plus up to 3
/// filler words before its head noun, matching real examples like "the Big Bang" or "the
/// enigmatic dance" rather than only single-word nouns.
static FAUX_SCALE_RANGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)from (?:the )?(?:\w+ ){0,3}\w+ of (?:the )?(?:\w+ ){0,3}\w+ to (?:the )?(?:\w+ ){0,3}\w+ of (?:the )?(?:\w+ ){0,3}\w+(?-u:\b)")
        .unwrap()
});

/// Scans all three sub-patterns over the masked prose stream (headings in scope, frontmatter and
/// URLs skipped). A (b) match whose noun phrase is "testament" is skipped: that exact shape
/// ("stands/serves as a testament to ...") is SLOP014's CLICHE_PHRASES territory already. One
/// diagnostic per matching line, anchored at the first (leftmost) match; the fix hint differs by
/// which sub-pattern produced that leftmost match (importance puffery states no fact -> "state
/// the fact instead"; a fake-strong linking verb has a direct one-word replacement -> "use `is` or
/// `has`"; a faux-scale range names no actual magnitude -> "name the actual range instead"), so
/// the winning byte's origin is tracked alongside the byte itself.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    let en = ctx.natlangs.contains(&NatLang::En);
    let mut importance: HashSet<usize> = HashSet::new();
    if en {
        importance.extend(
            IMPORTANCE_PUFFERY_RE
                .find_iter(&doc.masked)
                .map(|m| m.start())
                .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte)),
        );
    }
    if ctx.natlangs.contains(&NatLang::PtBr) {
        importance.extend(
            IMPORTANCE_PUFFERY_PT_BR
                .find_iter(&doc.masked)
                .map(|m| m.start())
                .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte)),
        );
    }
    // (b) and (c) stay English-only: the pt-BR corpus check found their literal shapes
    // (`serve/atua/funciona como um`, `desde X até Y`) are ordinary Portuguese, not puffery, so
    // no pt-BR twin ships for either sub-pattern (see IMPORTANCE_PUFFERY_PT_BR's doc comment).
    let mut fake_strong: HashSet<usize> = HashSet::new();
    let mut faux_scale: HashSet<usize> = HashSet::new();
    if en {
        fake_strong.extend(
            FAKE_STRONG_VERB_RE
                .find_iter(&doc.masked)
                .filter(|m| !m.as_str().to_lowercase().contains("testament"))
                .map(|m| m.start())
                .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte)),
        );
        faux_scale.extend(
            FAUX_SCALE_RANGE_RE
                .find_iter(&doc.masked)
                .map(|m| m.start())
                .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte)),
        );
    }

    let bytes = importance
        .iter()
        .chain(fake_strong.iter())
        .chain(faux_scale.iter())
        .copied();
    let by_line = first_byte_per_line(doc, bytes);
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        let fix = if importance.contains(&byte) {
            "state the fact instead"
        } else if fake_strong.contains(&byte) {
            "use `is` or `has`"
        } else {
            "name the actual range instead"
        };
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            "importance puffery, inflated linking verb, or faux-scale range; state the fact plainly instead",
            fix,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::prose::ProseDoc;

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
    fn flags_importance_puffery() {
        let src = "The launch marks a pivotal moment for the product line.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP024");
        assert_eq!(diags[0].fix.as_deref(), Some("state the fact instead"));
    }

    #[test]
    fn flags_fake_strong_verb() {
        let src = "The app serves as a centralized hub for sponsor management.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP024");
        assert_eq!(diags[0].fix.as_deref(), Some("use `is` or `has`"));
    }

    #[test]
    fn flags_new_importance_puffery_markers() {
        let cases = [
            "The anniversary is a reminder of how far the platform has come.\n",
            "The mural symbolizes its ongoing role in the neighborhood.\n",
            "This release is setting the stage for the next major version.\n",
            "The merger was a key turning point for the company.\n",
            "The founder's decision left an indelible mark on the culture.\n",
            "The policy remains deeply rooted in decades of precedent.\n",
            "The update represents a shift toward async workflows.\n",
            "The redesign marks a shift in how users navigate the site.\n",
            "Performance has become the focal point of this quarter's work.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP024");
        }
    }

    #[test]
    fn flags_boasts_as_fake_strong_verb() {
        let src = "The dashboard boasts as a fully centralized hub for every metric.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP024");
        assert_eq!(diags[0].fix.as_deref(), Some("use `is` or `has`"));
    }

    #[test]
    fn clean_ordinary_use_of_reminder_and_shift_words() {
        let src = "Set a reminder for the on-call handoff, then update the schedule.\n\nThe on-call schedule shifts every Monday at nine.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_ordinary_prose() {
        let src = "The release ships next week with the fixes from this sprint.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_faux_scale_range() {
        let src = "The talk ranged from the singularity of the Big Bang to the enigmatic dance of dark matter.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP024");
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("name the actual range instead")
        );
    }

    #[test]
    fn clean_bare_from_to_range_is_ordinary_english() {
        // A single "from X to Y" with no nested "of ... of" is a normal range (a date range
        // here), not the poetic-pairing shape this sub-pattern targets.
        let src = "The migration window runs from Monday to Friday this week.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_single_of_is_not_a_faux_scale_range() {
        // Only one "of" in the whole range: still an ordinary sentence, not the NESTED double
        // "of" this sub-pattern requires.
        let src = "Coverage runs from the start of the meeting to the closing remarks.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_idiomatic_serves_as_without_article() {
        // "serves as expected"/"acts as designed" have no article after "as": ordinary
        // technical phrasing, not the inflated-hub shape this rule targets.
        let src = "The retry wrapper serves as expected under sustained load.\n\nThe fallback acts as designed when the primary region is down.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_duplicate_cliche_testament_span() {
        // "stands as a testament to" is SLOP014's CLICHE_PHRASES territory; SLOP024 must stay
        // silent on that exact span so the two Tier-A rules never both fire on one line.
        use crate::rules::cliche;
        let src = "The bridge stands as a testament to careful engineering.\n";
        let puffery_diags = diagnostics_for(src);
        assert!(puffery_diags.is_empty());

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
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut cliche_out = Vec::new();
        (cliche::RULE.check)(&cliche::RULE, &ctx, &mut cliche_out);
        assert_eq!(cliche_out.len(), 1);
        assert_eq!(cliche_out[0].code, "SLOP014");
    }

    #[test]
    fn does_not_duplicate_hedging_role_span() {
        // "plays a vital role" is SLOP015's HEDGE_PHRASES territory; SLOP024 must stay silent on
        // that phrase even though the catalog groups it under "importance puffery" too.
        use crate::rules::hedging;
        let src = "Caching plays a vital role in keeping latency low.\n";
        assert!(diagnostics_for(src).is_empty());

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
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut hedging_out = Vec::new();
        (hedging::RULE.check)(&hedging::RULE, &ctx, &mut hedging_out);
        // hedging.rs is a density rule (needs 3+ hits or a repeat) so a single occurrence alone
        // won't fire either -- the point of this test is only that puffery.rs never claims it.
        assert!(hedging_out.is_empty());
    }

    /// One sample per `IMPORTANCE_PUFFERY_PT_BR` top-level alternative.
    #[test]
    fn flags_each_pt_br_importance_puffery_alternative() {
        let cases = [
            "O lançamento marca um momento decisivo para a empresa este ano.\n",
            "A crise marcou um ponto de virada na história da fábrica.\n",
            "O acordo representa um marco significativo para o setor de tecnologia.\n",
            "A nova lei consolida a posição do país na região.\n",
            "O relatório destaca a importância do investimento em pesquisa.\n",
            "A decisão reflete uma mudança mais ampla na política da empresa.\n",
            "A tecnologia está prestes a revolucionar a forma como trabalhamos.\n",
            "O produto estabelece um novo padrão para a indústria.\n",
            "Isso serve como um lembrete de que a segurança importa.\n",
            "A parceria abre caminho para novos investimentos na região.\n",
            "A reforma prepara o terreno para mudanças futuras na empresa.\n",
            "A tradição está profundamente enraizada na cultura local.\n",
            "A atualização representa uma mudança de paradigma para o setor.\n",
            "A qualidade é o ponto focal deste projeto este ano.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP024");
            assert_eq!(diags[0].fix.as_deref(), Some("state the fact instead"));
        }
    }

    /// The two shapes measured and dropped for the pt-BR corpus: ordinary Portuguese, not
    /// puffery (see `IMPORTANCE_PUFFERY_PT_BR`'s doc comment).
    #[test]
    fn clean_pt_br_dropped_shapes() {
        for src in [
            "O motor serve como um sistema de resfriamento auxiliar no veículo.\n",
            "A viagem foi planejada desde o início até o fim pela mesma equipe.\n",
        ] {
            assert!(
                diagnostics_for(src).is_empty(),
                "unexpectedly flagged: {src:?}"
            );
        }
    }

    #[test]
    fn natlang_gate_silences_the_other_languages_panel() {
        let pt_positive = "O lançamento marca um momento decisivo para a empresa este ano.\n";
        assert!(diagnostics_for_natlangs(pt_positive, &[NatLang::En]).is_empty());

        let en_positive = "The launch marks a pivotal moment for the product line.\n";
        assert!(diagnostics_for_natlangs(en_positive, &[NatLang::PtBr]).is_empty());
    }
}
