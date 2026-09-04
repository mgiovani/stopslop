use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::first_byte_per_line;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP035",
    name: "Outline-shaped filler section",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

/// (a) Heading titles that are the outline-shaped placeholder itself, matched in full (not a
/// substring) and case-insensitively.
const FILLER_HEADINGS: &[&str] = &[
    "challenges",
    "challenges and legacy",
    "challenges and opportunities",
    "challenges and considerations",
    "challenges and future prospects",
    "future outlook",
    "future prospects",
    "future directions",
    "legacy and impact",
    "impact and legacy",
    "significance and impact",
    "conclusion and future outlook",
];

/// (b) Body phrase: a vague "faces/remains several/numerous/many challenges" hand-wave, plus
/// the bare "despite these challenges".
static FILLER_BODY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(despite these challenges|despite (?:its|these|the) [^.\n]{0,40}?(?:faces?|remains?) (?:several|numerous|many|a number of) (?:challenges|obstacles|hurdles|limitations))(?-u:\b)")
        .unwrap()
});

/// Brazilian-Portuguese twin of `FILLER_HEADINGS`, matched the same way (exact heading text,
/// lowercased). Listed as literal accented strings rather than regex classes, since headings are
/// matched by exact `[&str]` comparison, not by regex. Measured against a 318-document,
/// 1.3-million-word human corpus and dropped: the bare `Desafios` heading (6 human docs), `Impacto e legado` (2), `Legado e
/// impacto` (1). Also deliberately excluded despite 0 human hits: `Considerações finais` and
/// `Perspectivas futuras`, the mandated names of the conclusion section in Brazilian academic
/// writing (ABNT-style monographs) that the corpus does not contain -- flagging every human
/// monograph's conclusion heading would be a false positive by construction.
const FILLER_HEADINGS_PT_BR: &[&str] = &[
    "desafios e oportunidades",
    "desafios e perspectivas",
    "desafios e considerações",
    "desafios e perspectivas futuras",
    "direções futuras",
    "importância e impacto",
    "conclusão e perspectivas",
    "conclusão e perspectivas futuras",
];

/// Brazilian-Portuguese twin of `FILLER_BODY_RE`. `entraves` and `desafios` end on an ASCII
/// letter, `limitações` ends on the ASCII `s` after the class, so the trailing `(?-u:\b)` is
/// valid throughout.
static FILLER_BODY_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:apesar (?:desses|destes|dos|das) desafios|apesar d[eoa]s? [^.\n]{0,40}?(?:enfrenta|enfrentam|permanece|permanecem|ainda h[áa]) (?:v[áa]rios|diversos|in[úu]meros|muitos|uma s[ée]rie de) (?:desafios|obst[áa]culos|limita[çc][õo]es|entraves))(?-u:\b)")
        .unwrap()
});

/// Scans headings for exact-match outline-shaped titles, and the masked prose stream (headings
/// in scope, frontmatter and URLs skipped) for the vague-limitation body phrase. One diagnostic
/// per heading match, and one per matching line for the body phrase.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let en = ctx.natlangs.contains(&NatLang::En);
    let pt_br = ctx.natlangs.contains(&NatLang::PtBr);

    for h in &doc.headings {
        if doc.in_frontmatter(h.byte_start) {
            continue;
        }
        let title = h.text.trim().to_lowercase();
        if (en && FILLER_HEADINGS.contains(&title.as_str()))
            || (pt_br && FILLER_HEADINGS_PT_BR.contains(&title.as_str()))
        {
            out.push(Diagnostic::at_fix(
                rule,
                ctx,
                h.line,
                h.col,
                format!("outline-shaped filler heading: \"{}\"", h.text.trim()),
                "cut the section or replace it with specifics",
            ));
        }
    }

    let mut bytes: Vec<usize> = Vec::new();
    if en {
        bytes.extend(
            FILLER_BODY_RE
                .find_iter(&doc.masked)
                .map(|m| m.start())
                .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte)),
        );
    }
    if pt_br {
        bytes.extend(
            FILLER_BODY_PT_BR
                .find_iter(&doc.masked)
                .map(|m| m.start())
                .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte)),
        );
    }
    let by_line = first_byte_per_line(doc, bytes.into_iter());
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            "outline-shaped filler phrase waves at limitations without naming them",
            "name the specific limitation",
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
            natlangs,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn flags_filler_heading() {
        let src = "# Future Outlook\n\nThe team plans to keep iterating on the roadmap.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP035");
        assert_eq!(diags[0].line, 1);
    }

    #[test]
    fn flags_filler_heading_case_insensitively() {
        let src = "# challenges AND opportunities\n\nBody text follows here.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_despite_these_challenges() {
        let src = "The rollout shipped on time, despite these challenges.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP035");
    }

    #[test]
    fn flags_faces_several_challenges_shape() {
        let src = "Despite its early success, the project faces several challenges going into next year.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn clean_specific_heading_and_body() {
        let src = "# Known Limitations\n\nThe importer does not yet support incremental syncs; a full resync is required after schema changes.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_heading_that_only_partially_matches() {
        // Full-match required: a heading merely containing "challenges" as a substring of a
        // longer, specific title must not fire.
        let src = "# Scaling Challenges In The Payments Pipeline\n\nBody text follows here.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_each_pt_br_filler_heading() {
        for heading in FILLER_HEADINGS_PT_BR {
            let src = format!("# {heading}\n\nO time trabalha para resolver isso este mês.\n");
            let diags = diagnostics_for(&src);
            assert_eq!(
                diags.len(),
                1,
                "expected exactly one finding for: {heading}"
            );
            assert_eq!(diags[0].code, "SLOP035");
        }
    }

    /// One sample per `FILLER_BODY_PT_BR` top-level alternative.
    #[test]
    fn flags_each_pt_br_filler_body_alternative() {
        let cases = [
            "O lançamento ocorreu no prazo, apesar desses desafios.\n",
            "Apesar do rápido crescimento, a startup enfrenta vários desafios operacionais.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP035");
        }
    }

    /// The bare `Desafios` heading and the near-miss headings measured and dropped for the
    /// pt-BR corpus, plus the two ABNT-mandated conclusion headings excluded on purpose (see
    /// `FILLER_HEADINGS_PT_BR`'s doc comment).
    #[test]
    fn clean_pt_br_dropped_and_excluded_headings() {
        for src in [
            "# Desafios\n\nO time trabalha nisso continuamente.\n",
            "# Impacto e Legado\n\nO projeto mudou como o time trabalha.\n",
            "# Legado e Impacto\n\nO projeto mudou como o time trabalha.\n",
            "# Considerações Finais\n\nEsta seção resume o trabalho realizado no capítulo.\n",
            "# Perspectivas Futuras\n\nEsta seção aponta direções para trabalhos futuros.\n",
        ] {
            assert!(
                diagnostics_for(src).is_empty(),
                "unexpectedly flagged: {src:?}"
            );
        }
    }

    #[test]
    fn natlang_gate_silences_the_other_languages_panel() {
        let pt_positive = "# Desafios e Oportunidades\n\nO time trabalha para resolver isso.\n";
        assert!(diagnostics_for_natlangs(pt_positive, &[NatLang::En]).is_empty());

        let en_positive = "# Future Outlook\n\nThe team plans to keep iterating on the roadmap.\n";
        assert!(diagnostics_for_natlangs(en_positive, &[NatLang::PtBr]).is_empty());
    }
}
