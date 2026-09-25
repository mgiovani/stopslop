use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::first_byte_per_line;
use crate::registry::RuleDef;
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP025",
    name: "Unsourced weasel attribution",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

/// Anonymous-authority attribution phrases: an appeal to an unnamed "expert"/"study"/"critic"
/// standing in for a real, checkable source.
static WEASEL_ATTRIBUTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(experts (?:agree|say)|studies (?:show|suggest)|research (?:shows|suggests|indicates)|industry reports suggest|many (?:argue|believe)|some (?:say|argue)|it is widely regarded as|widely considered|widely regarded as|it is believed that|critics argue|analysts predict|reports indicate|sources say|it is often said|observers have (?:cited|noted)|several sources|several publications|many have (?:argued|noted|suggested)|it has been (?:suggested|argued|noted)|commentators (?:say|note|argue)|proponents (?:argue|say)|detractors (?:argue|say))(?-u:\b)")
        .unwrap()
});

/// Brazilian-Portuguese twin of `WEASEL_ATTRIBUTION_RE`. `NOTABILITY_NAME_DROP_RE` has no pt-BR
/// twin -- not measured for phase 2, stays English-only. Measured against a 318-document,
/// 1.3-million-word human corpus and dropped above 2 hits: the impersonal `-se que` construction (`acredita-se que` 24
/// human hits, `estima-se que` 9, `sabe-se que` 5, `diz-se que` 4, `considera-se que` 5) is
/// standard encyclopedic Portuguese, not a weasel; likewise `estudos mostram` (5), `alguns dizem`
/// (2), `críticos argumentam` (1), `segundo especialistas` (3) -- every one has human hits,
/// exactly as issue #30 predicted for the impersonal `-se` form. `de acordo com especialistas` is
/// the one entry that ships (1 human hit).
///
/// Split into two groups instead of one `\b(?:...)\b`, same idiom as `hedging.rs`'s
/// `HEDGE_PHRASES_PT_BR`: the ASCII-initial alternatives sit under a leading+trailing
/// `(?-u:\b)`, while `[ée] amplamente ...` opens on the accented "é" -- a leading `(?-u:\b)`
/// right there never matches, since neither the preceding space byte nor `é`'s lead byte
/// (0xC3, not an ASCII word byte) is a word byte, so that assertion always fails at the one
/// position where this branch would start. Dropping the leading boundary and keeping only the
/// trailing one (every branch ends on the ASCII `[oa]s?`) fixes it.
static WEASEL_ATTRIBUTION_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:(?-u:\b)(?:especialistas (?:afirmam|concordam|apontam|dizem|recomendam)|pesquisas (?:mostram|indicam|sugerem|apontam|revelam)|a ci[êe]ncia (?:mostra|comprova)|muitos (?:acreditam|argumentam|afirmam|defendem)|analistas (?:preveem|apontam|afirmam)|relat[óo]rios (?:indicam|apontam)|fontes (?:afirmam|dizem|indicam|apontam)|costuma-se dizer|observadores (?:notaram|apontam|destacam)|diversas fontes|tem sido (?:sugerido|argumentado|apontado|dito)|comentaristas (?:dizem|apontam|argumentam)|defensores (?:argumentam|afirmam)|de acordo com especialistas)(?-u:\b)|[ée] amplamente (?:considerad|reconhecid|aceit)[oa]s?(?-u:\b))",
    )
    .unwrap()
});

/// Notability by name-dropping: three or more bare, comma-chained capitalized outlet/publication
/// names right after "cited/covered/featured in/by", with no per-citation context (no link, no
/// date, no quote -- just a list of names standing in for real sourcing). An optional "and "
/// before the final name covers the ordinary English list form ("X, Y, and Z"), not just a bare
/// comma splice.
static NOTABILITY_NAME_DROP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?-u:\b)(?:cited|covered|featured) (?:in|by) [A-Z][\w& ]+(?:, (?:and )?[A-Z][\w& ]+){2,}",
    )
    .unwrap()
});

/// A citation on the SAME LINE as a weasel phrase means the claim is actually sourced, not
/// unsourced: a markdown link `[text](target)`, a bare URL, a footnote reference (`[^1]`), a
/// reST-style citation/footnote reference (`[label]_`), or a parenthetical with a year
/// ("(Smith 2023)").
static LINE_CITATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\[[^\]\n]*\]\([^)\n]*\)|https?://\S+|\[\^[^\]\n]+\]|\[[^\]\n]+\]_|\([^()\n]*(?-u:\b)(?:19|20)\d{2}(?-u:\b)[^()\n]*\)",
    )
    .unwrap()
});

/// Scans the masked prose stream for unsourced weasel attribution and notability name-dropping
/// (headings in scope, frontmatter and URL spans skipped). Any match whose LINE also carries a
/// citation signal is suppressed -- the rule targets bare appeals to unnamed/uncontextualized
/// authority, not attributed claims. One diagnostic per matching line, anchored at the first
/// (leftmost) match; the message and fix differ by which sub-pattern produced that match.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    let cited_lines: HashSet<usize> = LINE_CITATION_RE
        .find_iter(&doc.masked)
        .map(|m| doc.line_col(m.start()).0)
        .collect();

    let mut attribution: HashSet<usize> = HashSet::new();
    let mut name_drop: HashSet<usize> = HashSet::new();
    if ctx.natlangs.contains(&NatLang::En) {
        attribution.extend(
            WEASEL_ATTRIBUTION_RE
                .find_iter(&doc.masked)
                .map(|m| m.start())
                .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte)),
        );
        name_drop.extend(
            NOTABILITY_NAME_DROP_RE
                .find_iter(&doc.masked)
                .map(|m| m.start())
                .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte)),
        );
    }
    if ctx.natlangs.contains(&NatLang::PtBr) {
        attribution.extend(
            WEASEL_ATTRIBUTION_PT_BR
                .find_iter(&doc.masked)
                .map(|m| m.start())
                .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte)),
        );
    }

    let bytes = attribution
        .iter()
        .chain(name_drop.iter())
        .copied()
        .filter(|&byte| !cited_lines.contains(&doc.line_col(byte).0));
    let by_line = first_byte_per_line(doc, bytes);
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        let (message, fix) = if attribution.contains(&byte) {
            (
                "unsourced weasel attribution; name the source or cite it",
                "name the source, or cut the claim",
            )
        } else {
            (
                "notability by name-dropping outlets with no per-citation context",
                "cite each claim individually, or cut the list",
            )
        };
        out.push(Diagnostic::at_fix(rule, ctx, line, col, message, fix));
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
    fn flags_experts_agree() {
        let src = "Experts agree that the migration reduced downtime.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP025");
    }

    #[test]
    fn flags_critics_argue_and_reports_indicate() {
        let src =
            "Critics argue the pricing change will hurt small teams.\n\nReports indicate a slowdown in adoption.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.code == "SLOP025"));
    }

    #[test]
    fn diagnostic_carries_a_fix_hint() {
        let diags = diagnostics_for("Experts agree that the migration reduced downtime.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("name the source, or cut the claim")
        );
    }

    #[test]
    fn flags_new_weasel_attribution_markers() {
        let cases = [
            "Observers have noted a slowdown in release cadence.\n",
            "Several sources describe the outage as preventable.\n",
            "Several publications covered the pricing backlash.\n",
            "Many have argued the migration was rushed.\n",
            "It has been suggested that the rollback was unnecessary.\n",
            "Commentators say the redesign missed the mark.\n",
            "Proponents argue the change reduces on-call load.\n",
            "Detractors argue the change adds needless complexity.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP025");
        }
    }

    #[test]
    fn clean_ordinary_prose() {
        let src = "Users reported a crash on startup, and the team shipped a fix the same day.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn silent_when_markdown_link_present() {
        let src = "Studies show a 40% drop in latency, per [the benchmark report](https://example.com/report).\n"; // ai-slop-ignore
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn silent_when_bare_url_present() {
        let src = "Research indicates a measurable improvement; see https://example.com/data for the full set.\n"; // ai-slop-ignore
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn silent_when_parenthetical_year_present() {
        let src = "Analysts predict slower growth next quarter (Chen 2024).\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn silent_when_footnote_reference_present() {
        let src = "Sources say the rollout was delayed[^1].\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_notability_name_drop() {
        let src = "The project was featured in The New York Times, The Guardian, and Wired.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP025");
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("cite each claim individually, or cut the list")
        );
    }

    #[test]
    fn flags_notability_name_drop_with_bare_comma_splice() {
        let src = "The tool was cited in TechCrunch, VentureBeat, Ars Technica.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP025");
    }

    #[test]
    fn clean_single_outlet_mention() {
        // One outlet, no chain of names -- an ordinary, checkable attribution.
        let src = "The report was featured in The New York Times.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// One sample per `WEASEL_ATTRIBUTION_PT_BR` top-level alternative.
    #[test]
    fn flags_each_pt_br_weasel_attribution_alternative() {
        let cases = [
            "Especialistas afirmam que a mudança reduziu o tempo de resposta.\n",
            "Pesquisas mostram uma queda significativa na taxa de erro.\n",
            "A ciência mostra que o método funciona na maioria dos casos.\n",
            "Muitos acreditam que a migração foi precipitada demais.\n",
            "A prática é amplamente considerada segura pela comunidade.\n",
            "Analistas preveem uma queda na adoção da ferramenta.\n",
            "Relatórios indicam uma lentidão crescente no sistema.\n",
            "Fontes afirmam que o lançamento foi adiado sem aviso.\n",
            "Costuma-se dizer que a simplicidade vence a complexidade.\n",
            "Observadores notaram um atraso incomum na resposta do serviço.\n",
            "Diversas fontes descrevem a falha como evitável.\n",
            "Tem sido sugerido que o rollback foi desnecessário.\n",
            "Comentaristas dizem que o redesenho perdeu o ponto principal.\n",
            "Defensores argumentam que a mudança reduz a carga operacional.\n",
            "De acordo com especialistas, o problema tende a se repetir.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP025");
        }
    }

    /// The impersonal `-se que` construction and the other shapes measured and dropped for the
    /// pt-BR corpus: standard encyclopedic Portuguese, not a weasel (see the panel's doc comment).
    #[test]
    fn clean_pt_br_dropped_shapes() {
        for src in [
            "Acredita-se que o novo modelo reduz o tempo de resposta.\n",
            "Estima-se que o custo total aumente até o fim do ano.\n",
            "Sabe-se que o sistema falha sob carga extrema.\n",
            "Diz-se que a nova versão corrige o problema antigo.\n",
            "Considera-se que o projeto está pronto para produção.\n",
            "Estudos mostram uma melhora consistente na taxa de acerto.\n",
            "Alguns dizem que a mudança foi precipitada demais.\n",
            "Críticos argumentam que o preço ficou alto demais.\n",
            "Segundo especialistas, o mercado deve se recuperar em breve.\n",
        ] {
            assert!(
                diagnostics_for(src).is_empty(),
                "unexpectedly flagged: {src:?}"
            );
        }
    }

    #[test]
    fn natlang_gate_silences_the_other_languages_panel() {
        let pt_positive = "Especialistas afirmam que a mudança reduziu o tempo de resposta.\n";
        assert!(diagnostics_for_natlangs(pt_positive, &[NatLang::En]).is_empty());

        let en_positive = "Experts agree that the migration reduced downtime.\n";
        assert!(diagnostics_for_natlangs(en_positive, &[NatLang::PtBr]).is_empty());
    }
}
