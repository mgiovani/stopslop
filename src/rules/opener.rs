use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::PROSE_LANGS;
use crate::prose::first_byte_per_line;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP022",
    name: "Formulaic opener / rhetorical setup",
    tier: Tier::B,
    langs: PROSE_LANGS,
    default_on: true,
    path_gated: false,
    check,
};

/// Sentence/line-initial anchor, shared by both patterns below: either the true start of a line
/// (optionally preceded by a list marker, blockquote `>`, bold/italic `*`/`_`, an ATX `#`, or an
/// ordered-list digit/`.`/`)`), or the tail of the previous sentence (`[.!?]` + optional closing
/// quote/paren + whitespace). Anchoring here (rather than a bare `\b`) is what keeps a phrase like
/// "the part everyone misses" from firing when it shows up mid-clause instead of opening one.
const PREFIX: &str = r#"(?:^[ \t>*_#0-9.)-]*|[.!?]["')\]]?[ \t]+)"#;

/// Families (a) throat-clearing, (b) faux-insight setups, and (c) rhetorical setups (minus the
/// self-answered question/answer shape, handled separately below since its shape isn't a fixed
/// phrase), plus three later additions: (d) signposting ("let's dive into", "buckle up", ...),
/// (e) authority tropes ("in reality", "what really matters", ...), and (f) conversational
/// openers ("real talk", "the thing is,"). Capture group 1 is the phrase itself, so the diagnostic
/// column points at the phrase, not the anchor. Apostrophes are optional (`'?`) to also catch the
/// typo'd unapostrophized form, matching this codebase's existing phrase-panel convention.
///
/// Several signposting/authority candidates were dropped for overlap with an existing panel
/// rather than added here (OVERLAP INVARIANT -- every phrase panel in this codebase stays
/// disjoint, so no span is ever flagged by two rules):
/// - "let's dive in" and "let's take a look" are already in `prose_words::FILLER_PHRASES`
///   (SLOP027, `rules::filler`) verbatim.
/// - "at its core" is already in `prose_words::FILLER_PHRASES` (SLOP027) verbatim.
/// - "the real question is" was dropped: `rules::recap`'s KICKER_PHRASE (SLOP029) already matches
///   the bare substring "the real question" with no word-boundary anchor, so a sentence-initial
///   "The real question is ..." inside the document's final block could be claimed by both rules.
static OPENER_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"(?im){PREFIX}(here'?s the thing|here'?s what i mean|let me be clear|i'?ll be honest|let'?s be honest|the uncomfortable truth is|here'?s the deal|make no mistake|let me explain|but here'?s the kicker|this is the part most people skip|what most people get wrong|here'?s what nobody tells you|the part everyone misses|what nobody talks about|most people don'?t realize|here'?s what they don'?t tell you|the part nobody mentions|what if i told you|think about it:|plot twist:|let that sink in|spoiler alert:|here'?s a thought:|let'?s dive into|let'?s explore|let'?s break this down|let'?s break it down|let'?s get started|here'?s what you need to know|now let'?s look at|without further ado|buckle up|in reality|what really matters|the deeper issue|the heart of the matter|real talk|the thing is,)"#
    ))
    .unwrap()
});

/// Self-answered question/answer pairs: a short question (<=10 words, ending in "?") immediately
/// followed on the SAME line by its own short answer clause (<=8 words, ending in "."/"!"), e.g.
/// "The answer? Yes." / "Why does this matter? Because latency drops." Group 1 is the question
/// (the diagnostic anchor); group 2 is the answer clause.
static QUESTION_ANSWER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"(?im){PREFIX}((?:[A-Za-z][\w']*[ \t]+){{0,9}}[A-Za-z][\w']*\?)[ \t]+((?:[A-Za-z][\w']*[ \t]+){{0,7}}[A-Za-z][\w']*[.!])"#
    ))
    .unwrap()
});

/// Scans the masked prose stream for formulaic openers and self-answered question/answer pairs.
/// Frontmatter and URL spans are skipped; code is already blanked. Headings are in scope (a
/// formulaic heading like "## Here's the Thing About Caching" is a prime target, same rationale
/// as cliche.rs). Emits one diagnostic per matching line, anchored at that line's first match.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    let bytes = OPENER_PHRASES
        .captures_iter(&doc.masked)
        .map(|c| c.get(1).unwrap().start())
        .chain(
            QUESTION_ANSWER
                .captures_iter(&doc.masked)
                .map(|c| c.get(1).unwrap().start()),
        )
        .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte));
    let by_line = first_byte_per_line(doc, bytes);
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            "formulaic opener / rhetorical setup; get to the point instead",
            "delete the opener and start with the fact",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
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
    fn flags_throat_clearing_opener() {
        let diags = diagnostics_for("Here's the thing: the cache never expires on its own.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP022");
    }

    #[test]
    fn clean_ordinary_sentence_about_the_cache() {
        let diags =
            diagnostics_for("The cache has an interesting expiration behavior worth knowing.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_faux_insight_setup() {
        let diags =
            diagnostics_for("This is the part most people skip when reviewing a pull request.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP022");
    }

    #[test]
    fn clean_ordinary_review_sentence() {
        let diags =
            diagnostics_for("Most reviewers check the diff before approving a pull request.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_rhetorical_setup_phrase() {
        let diags = diagnostics_for("Plot twist: the deploy rolled back automatically.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP022");
    }

    #[test]
    fn clean_sentence_mentioning_plot_and_twist_mid_clause() {
        let diags = diagnostics_for(
            "The twist in the plot was that the deploy rolled back automatically.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_self_answered_question() {
        let diags = diagnostics_for("Why does this matter? Because latency drops.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP022");
    }

    #[test]
    fn clean_long_answer_after_question() {
        // The question is short, but the answer clause runs well past the 8-word cap, so this
        // reads as an ordinary FAQ explanation rather than the rhetorical gotcha shape.
        let diags = diagnostics_for(
            "Why does this matter? The reasoning spans several paragraphs of historical context that this document covers in detail.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn dedupes_to_one_diagnostic_per_line() {
        // Two independently anchored openers on one line (line-initial, then sentence-initial
        // after the period) must still collapse to a single diagnostic for that line.
        let diags = diagnostics_for("Make no mistake. Here's the thing: it works.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn skips_phrase_inside_frontmatter() {
        let src =
            "---\nHere's the thing: draft notes\n---\n\nOrdinary text closes out the document.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn diagnostic_carries_a_fix_hint() {
        let diags = diagnostics_for("Plot twist: the deploy rolled back automatically.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("delete the opener and start with the fact")
        );
    }

    #[test]
    fn flags_signposting_openers() {
        let diags = diagnostics_for(
            "Let's dive into the config file.\n\nLet's explore the retry logic next.\n\nLet's break this down into three steps.\n\nBuckle up, this gets technical fast.\n",
        );
        assert_eq!(diags.len(), 4);
        assert!(diags.iter().all(|d| d.code == "SLOP022"));
    }

    #[test]
    fn clean_ordinary_sentence_with_dive_in_and_take_a_look() {
        // "let's dive in" / "let's take a look" are SLOP027's FILLER_PHRASES territory already;
        // opener.rs must not also claim them.
        let diags = diagnostics_for(
            "Let's dive in and look at the config.\n\nLet's take a look at the retry logic.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_authority_trope_openers() {
        let diags = diagnostics_for(
            "In reality, the fix took an afternoon.\n\nWhat really matters is uptime.\n\nThe deeper issue is a missing index.\n\nThe heart of the matter is latency.\n",
        );
        assert_eq!(diags.len(), 4);
        assert!(diags.iter().all(|d| d.code == "SLOP022"));
    }

    #[test]
    fn clean_sentence_using_at_its_core() {
        // "at its core" is SLOP027's FILLER_PHRASES territory already; opener.rs must not also
        // claim it, even though the catalog groups it under "authority trope" too.
        let diags = diagnostics_for("At its core, the service is a thin proxy.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn does_not_duplicate_recap_real_question_kicker() {
        // "the real question" is SLOP029's KICKER_PHRASE territory; opener.rs must stay out of
        // its way rather than also claiming "the real question is".
        let diags = diagnostics_for("The real question is whether caching helps.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_conversational_openers() {
        let diags =
            diagnostics_for("Real talk, the rollout was rough.\n\nThe thing is, it works now.\n");
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.code == "SLOP022"));
    }

    #[test]
    fn skips_phrase_inside_url() {
        // The phrase regex requires literal spaces ("here's the thing"); a hyphenated URL slug
        // never matches on text alone, and is additionally masked via in_url as a
        // belt-and-suspenders check (same rationale as cliche.rs).
        let src = "Read more at https://example.com/heres-the-thing-guide today.\n"; // ai-slop-ignore
        assert!(diagnostics_for(src).is_empty());
    }
}
