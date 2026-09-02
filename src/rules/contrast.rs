use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose::first_byte_per_line;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP023",
    name: "Binary contrast / negative listing",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: true,
    path_gated: false,
    check,
};

/// (a) Binary contrast: the "not X, but Y" shapes that `parallelism.rs`'s NEGATIVE_PARALLELISM
/// (SLOP017) does NOT already cover. SLOP017 requires the literal words "not only"/"not just" (or
/// "it's not just/only ... it's") within a SINGLE sentence (its gap class `[^.?!\n]` forbids a
/// sentence break). These four sub-shapes are either cross-sentence ("it's not X. it's Y.", "the
/// problem isn't X. the problem is Y.") or drop the just/only qualifier entirely ("this isn't X,
/// it's Y.", "the question isn't X, it's Y.", "it's not about X, it's about Y."), so none of them
/// can trip SLOP017's patterns.
static BINARY_CONTRAST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)\bit'?s not\b[^.!?\n]{0,60}[.!]\s+it'?s\b|\b(?:this|it|the question) isn'?t\b[^.!?\n]{0,60},\s*it'?s\b|\bit'?s not about\b[^.!?\n]{0,60},\s*it'?s about\b|\bthe problem isn'?t\b[^.!?\n]{0,60}[.!]\s+the problem is\b"#,
    )
    .unwrap()
});

/// (b) Negative listing: two or more consecutive "Not X." / "No Y." fragments, e.g. "Not a
/// framework. Not a library. A compiler." The first fragment is anchored at a sentence or line
/// start (optionally after a list marker, blockquote `>`, or bold/italic punctuation) so a normal
/// mid-sentence negation ("...service, not just the session store.") never anchors a match; the
/// second fragment's anchor comes for free from the `[.!]\s+` gap that closes the first one.
/// Capture group 1 is the whole two-fragment run, used as the diagnostic anchor.
static NEGATIVE_LISTING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?im)(?:^[ \t>*_#-]*|[.!?]["')\]]?[ \t]+)((?:not|no) [^.!?\n]{1,40}[.!]\s+(?:not|no) [^.!?\n]{1,40}[.!])"#,
    )
    .unwrap()
});

/// Scans the masked prose stream for binary-contrast and negative-listing shapes. Frontmatter and
/// URL spans are skipped; code is already blanked. Emits one diagnostic per matching line,
/// anchored at that line's first match.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    let bytes = BINARY_CONTRAST
        .find_iter(&doc.masked)
        .map(|m| m.start())
        .chain(
            NEGATIVE_LISTING
                .captures_iter(&doc.masked)
                .map(|c| c.get(1).unwrap().start()),
        )
        .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte));
    let by_line = first_byte_per_line(doc, bytes);
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "binary contrast / negative listing; rewrite it directly",
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
            index: None,
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
    fn flags_cross_sentence_its_not_contrast() {
        let diags =
            diagnostics_for("It's not a caching bug. It's a race in the invalidation path.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn flags_isnt_comma_contrast() {
        let diags =
            diagnostics_for("This isn't a performance tweak, it's a full rewrite of the path.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn flags_the_question_isnt_contrast() {
        let diags = diagnostics_for(
            "The question isn't whether to cache, it's how long to keep an entry.\n",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn flags_not_about_contrast() {
        let diags = diagnostics_for("It's not about speed, it's about getting the answer right.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn flags_the_problem_isnt_contrast() {
        let diags = diagnostics_for(
            "The problem isn't the query planner. The problem is a missing index.\n",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn clean_ordinary_two_sentence_paragraph() {
        let diags = diagnostics_for(
            "It's a small utility that wraps the retry logic. It's used by every client.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_negative_listing_run() {
        let diags =
            diagnostics_for("Not a framework. Not a library. A compiler for your test suite.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn clean_single_mid_sentence_negation() {
        // Only one negation, and it lands mid-sentence (after a comma) rather than opening one,
        // so this must not be mistaken for the two-fragment negative-listing shape.
        let diags = diagnostics_for(
            "That patch benefits every service that talks to the queue, not just the worker pool.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn does_not_duplicate_slop017_not_only_but_also() {
        // SLOP017 (parallelism.rs) owns this exact shape; SLOP023 must stay out of its way.
        let diags = diagnostics_for(
            "The new client is not only fast but also simple to integrate with existing code.\n",
        );
        assert!(diags.is_empty());
    }
}
