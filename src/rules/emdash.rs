use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP018",
    name: "Mid-prose em dash",
    tier: Tier::A,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: true,
    path_gated: false,
    check,
};

/// Flags every U+2014 (em dash) in the masked prose stream that isn't line-initial -- both the
/// tight form (word--word) and the spaced form (word -- word). Frontmatter is skipped (metadata,
/// not prose); headings are in scope. Code is already blanked in `doc.masked`, so a dash inside a
/// fence or inline span never reaches this scan. `---` rules/frontmatter fences are plain hyphens
/// and can't match U+2014 regardless. The one allowed dash is the attribution/quote convention --
/// a dash that opens the line (after optional whitespace/blockquote `>` markers), as in
/// `-- Oscar Wilde` -- since that's a typographic convention, not mid-sentence punctuation. Each
/// qualifying occurrence gets its own diagnostic (not deduped per line): a line with two mid-prose
/// dashes is two problems to fix, not one.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    for (byte, ch) in doc.masked.char_indices() {
        if ch != '\u{2014}' {
            continue;
        }
        if doc.in_frontmatter(byte) || is_line_initial(&doc.masked, byte) {
            continue;
        }
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "em dash in prose; prefer a comma, colon, or parentheses",
        ));
    }
}

/// True if nothing but whitespace and blockquote `>` markers precede `byte` on its line -- i.e.
/// this em dash is the first real thing on the line (the attribution/quote convention).
fn is_line_initial(masked: &str, byte: usize) -> bool {
    let line_start = masked[..byte].rfind('\n').map_or(0, |i| i + 1);
    masked[line_start..byte]
        .trim_start_matches(|c: char| c.is_whitespace() || c == '>')
        .is_empty()
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
    fn flags_tight_mid_sentence_dash() {
        let diags = diagnostics_for("The build pipeline\u{2014}stays simple across releases.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP018");
    }

    #[test]
    fn flags_spaced_mid_sentence_dash() {
        let diags = diagnostics_for("Deploys went smoothly \u{2014} no surprises this week.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_dash_in_heading() {
        let diags = diagnostics_for("# Release Notes \u{2014} Now Faster\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_single_occurrence_no_density_floor() {
        // Old rule needed >= 3 dashes at a density above 1%; the new rule fires on one.
        let diags = diagnostics_for(
            "This document is otherwise ordinary and reasonably long, but one clause here\u{2014}right here\u{2014}breaks the flow.\n",
        );
        assert!(!diags.is_empty());
    }

    #[test]
    fn allows_line_initial_attribution() {
        let diags = diagnostics_for("\u{2014} Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_blockquote_attribution() {
        let diags = diagnostics_for("> \u{2014} Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_horizontal_rule_and_hyphens() {
        let src = "Body text.\n\n---\n\nMore body text, using a hyphen-range like 10-20 items.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn skips_frontmatter() {
        let src = "---\ntitle: Report \u{2014} Q3\n---\n\nBody text with no dash issue here.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn skips_dashes_inside_code_fence() {
        let src = "Body text.\n```\nword\u{2014}word \u{2014} word\n```\nMore body text here now today.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn skips_inline_code_dash() {
        let diags = diagnostics_for("Inline code stays silent: `a\u{2014}b`.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn each_occurrence_on_a_line_gets_its_own_diagnostic() {
        let diags = diagnostics_for("One\u{2014}two\u{2014}three mid-sentence dashes here.\n");
        assert_eq!(diags.len(), 2);
    }
}
