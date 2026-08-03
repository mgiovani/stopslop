use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose::ProseDoc;
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

/// Flags every U+2014 (em dash) in the masked prose stream that isn't block-initial -- both the
/// tight form (word--word) and the spaced form (word -- word). Frontmatter is skipped (metadata,
/// not prose); headings are in scope. Code is already blanked in `doc.masked`, so a dash inside a
/// fence or inline span never reaches this scan. `---` rules/frontmatter fences are plain hyphens
/// and can't match U+2014 regardless. The one allowed dash is the attribution/quote convention --
/// a dash that opens a *block* (after optional whitespace/blockquote `>` markers), as in
/// `-- Oscar Wilde` -- since that's a typographic convention, not mid-sentence punctuation. A dash
/// opening a line that merely continues the preceding one (wrapped prose, list-item continuation)
/// is ordinary punctuation that happened to land at a wrap point, so it is flagged. Each
/// qualifying occurrence gets its own diagnostic (not deduped per line): a line with two mid-prose
/// dashes is two problems to fix, not one.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    for (byte, ch) in doc.masked.char_indices() {
        if ch != '\u{2014}' {
            continue;
        }
        if doc.in_frontmatter(byte) || is_block_initial(doc, byte) {
            continue;
        }
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "em dash in prose; rewrite the sentence",
        ));
    }
}

/// True if this em dash opens a block: nothing but whitespace and blockquote `>` markers precede
/// it on its line, *and* the line itself starts a block: it is the document's first line (or the
/// first after frontmatter), it carries a `>` marker, or the line above is blank or itself a
/// blockquote line. Opening a line is not enough -- prose wraps, so a dash that merely landed at a
/// wrap point or on a list-item continuation line still needs flagging. One deliberate exception:
/// a `>`-marked line is always treated as block-initial, so a wrapped continuation *inside* a
/// blockquote stays exempt -- `> -- Author` under a quote is indistinguishable from attribution,
/// and attribution is the likelier reading.
fn is_block_initial(doc: &ProseDoc, byte: usize) -> bool {
    let masked = &doc.masked;
    let line_start = masked[..byte].rfind('\n').map_or(0, |i| i + 1);
    let prefix = &masked[line_start..byte];
    if !prefix
        .trim_start_matches(|c: char| c.is_whitespace() || c == '>')
        .is_empty()
    {
        return false;
    }
    if prefix.contains('>') {
        return true; // attribution inside a blockquote
    }
    let Some(before) = masked[..line_start].strip_suffix('\n') else {
        return true; // first line of the document
    };
    let prev_start = before.rfind('\n').map_or(0, |i| i + 1);
    if doc.in_frontmatter(prev_start) {
        return true; // first prose line after frontmatter -- the body's real first line
    }
    let prev = before[prev_start..].trim();
    // Blank line above => this dash opens a block. `>` line above => this is the lazy continuation
    // of a blockquote, where an unmarked `-- Author` line is the same attribution convention.
    prev.is_empty() || prev.starts_with('>')
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
    fn allows_attribution_after_blank_line() {
        let diags = diagnostics_for("> A quoted line of prose.\n\n\u{2014} Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_attribution_on_blockquote_lazy_continuation_line() {
        // CommonMark lazy continuation: the unmarked line still belongs to the blockquote, and
        // `-- Author` directly under a quote is the attribution convention, not mid-prose dash.
        let diags = diagnostics_for("> A quoted line of prose.\n\u{2014} Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_attribution_on_first_body_line_after_frontmatter() {
        // The closing `---` is not a blank line, but the line after it still opens the body.
        let diags = diagnostics_for("---\ntitle: Report\n---\n\u{2014} Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_attribution_after_whitespace_only_blank_line() {
        let diags = diagnostics_for("Body text sits above the quote.\n   \n\u{2014} Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_dash_starting_a_wrapped_continuation_line() {
        // The original false negative: prose wraps, and a dash landing at the wrap point escaped.
        let diags = diagnostics_for("a \u{2014} b\n      \u{2014} c\n\u{2014} d\n");
        assert_eq!(diags.len(), 3);
        assert_eq!((diags[0].line, diags[0].col), (1, 3));
        assert_eq!((diags[1].line, diags[1].col), (2, 7));
        assert_eq!((diags[2].line, diags[2].col), (3, 1));
    }

    #[test]
    fn flags_dash_at_column_one_of_a_continuation_line() {
        let diags = diagnostics_for(
            "The release went out on schedule and the rollout was uneventful\n\u{2014} apart from one flaky test.\n",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!((diags[0].line, diags[0].col), (2, 1));
    }

    #[test]
    fn flags_dash_on_list_item_continuation_line() {
        let diags = diagnostics_for(
            "- The first bullet runs long enough to wrap onto a second line\n  \u{2014} which continues it.\n",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!((diags[0].line, diags[0].col), (2, 3));
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
