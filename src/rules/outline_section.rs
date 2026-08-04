use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose::first_byte_per_line;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP035",
    name: "Outline-shaped filler section",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: false,
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
    Regex::new(r"(?i)\b(despite these challenges|despite (?:its|these|the) [^.\n]{0,40}?(?:faces?|remains?) (?:several|numerous|many|a number of) (?:challenges|obstacles|hurdles|limitations))\b")
        .unwrap()
});

/// Scans headings for exact-match outline-shaped titles, and the masked prose stream (headings
/// in scope, frontmatter and URLs skipped) for the vague-limitation body phrase. One diagnostic
/// per heading match, and one per matching line for the body phrase.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    for h in &doc.headings {
        if doc.in_frontmatter(h.byte_start) {
            continue;
        }
        let title = h.text.trim().to_lowercase();
        if FILLER_HEADINGS.contains(&title.as_str()) {
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

    let bytes = FILLER_BODY_RE
        .find_iter(&doc.masked)
        .map(|m| m.start())
        .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte));
    let by_line = first_byte_per_line(doc, bytes);
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
    use crate::prose::ProseDoc;

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        let doc = ProseDoc::parse(src);
        let ctx = LintContext {
            display_path: "test.md".to_string(),
            source: src,
            tree: None,
            lang: Lang::Md,
            comments: &doc.ignore_comments,
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: Some(&doc),
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
}
