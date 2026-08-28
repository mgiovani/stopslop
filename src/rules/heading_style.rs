use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP021",
    name: "Heading & marker formatting affectations",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx],
    default_on: true,
    path_gated: false,
    check,
};

/// Emoji, counted and reported separately from [`SYMBOL`] below: an arrow is not an emoji, and
/// reporting one as "emoji used as ... marker" is simply wrong. Separate classes also mean
/// separate thresholds, so one emoji plus one arrow no longer sum to the ≥2 that fires.
///
/// `\p{Emoji_Presentation}` (supported by the `regex` crate's default unicode feature, verified
/// against 1.10) misses "text-default" emoji -- Emoji=Yes but needing a trailing U+FE0F to render
/// colored -- so those are allow-listed explicitly, verified against Unicode's `emoji-data.txt`.
/// Note 🛠 U+1F6E0 is astral AND text-default, so "high codepoint => Emoji_Presentation" is not a
/// safe shortcut. `\p{Extended_Pictographic}` is deliberately NOT used as the base instead: it is
/// a forward-compatibility superset for line-breaking/ZWJ and would match unassigned codepoints.
const EMOJI: &str = r"(?:\p{Emoji_Presentation}|[\u{27A1}\u{2714}\u{25AA}\u{26A0}\u{1F6E0}])";

/// Plain `Sm`/`So` symbols with no emoji properties at all, used decoratively as markers.
///
/// Excluded on purpose: `»` U+00BB (quotation punctuation far more often than a bullet);
/// U+2500-257F box drawing (structural ASCII-art glyphs -- a block/divider pattern, not a
/// heading prefix); U+2014 em dash (`rules::emdash` owns it); `•` U+2022 (a *fake* bullet, since
/// GFM won't parse it as a list marker at all -- a different tell needing a different message).
const SYMBOL: &str = r"[→⇒➔✓✗✘●▸★☆✦]";

/// A marker directly after heading hashes, opening a bullet, or leading a bare line. `(?m)` so
/// `^` is per-line. The gap between the marker-opener (`#`s / bullet char) and the marker is
/// `[ \t]+`, NOT `\s+`: `\s` matches `\n`, so an unconstrained gap can glue an empty
/// heading/bullet line to an unrelated marker-leading line on the NEXT line (a phantom cross-line
/// match) -- the same "`\s` matches `\n`" anchoring bug this codebase already guards against
/// elsewhere (see residue.rs's `[ \t]*` opener anchor).
fn marker_re(class: &str) -> Regex {
    Regex::new(&format!(
        r"(?m)^[ \t]{{0,3}}(?:#{{1,6}}[ \t]+{class}|[-*+][ \t]+{class}|{class}[ \t]+\S)"
    ))
    .unwrap()
}

static EMOJI_MARKER_RE: LazyLock<Regex> = LazyLock::new(|| marker_re(EMOJI));
static SYMBOL_MARKER_RE: LazyLock<Regex> = LazyLock::new(|| marker_re(SYMBOL));

/// Guard for (b): at least one pair of consecutive lowercase words somewhere outside heading
/// lines — evidence the document actually uses sentence case, so an all-caps/style doc (which
/// has nothing to contrast against) never fires.
static SENTENCE_CASE_GUARD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[a-z]{2,}\s+[a-z]{2,}\b").unwrap());

/// Markdown table separator row (e.g. `| --- | --- |`, `:-:|:-:`): evidence a body is a table,
/// not thin prose, for sub-check (c) below.
static TABLE_SEPARATOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^[ \t]*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)+\|?\s*$").unwrap()
});

/// HTML comments (incl. this harness's own `<!-- expect-line: ... -->` test markers): stripped
/// out of a section body before word/sentence counting in sub-check (c) -- a trailing comment
/// is not prose.
static HTML_COMMENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<!--[\s\S]*?-->").unwrap());

/// A sentence boundary, for the crude sentence count in sub-check (c): any run of `.`/`!`/`?`.
static SENTENCE_END_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[.!?]+").unwrap());

/// Heading text that opens a reference/appendix-style section: conventionally short by nature,
/// not a slop signal. Matched as a case-insensitive prefix of the (trimmed) heading text.
const REFERENCE_HEADINGS: &[&str] = &[
    "references",
    "appendix",
    "footnotes",
    "footnote",
    "further reading",
    "see also",
    "acknowledgments",
    "acknowledgements",
    "license",
    "licence",
    "changelog",
    "citations",
    "bibliography",
];

fn is_reference_heading(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    REFERENCE_HEADINGS.iter().any(|kw| lower.starts_with(kw))
}

const STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "nor", "for", "of", "to", "in", "on", "at", "by", "with",
    "as", "from", "into", "over", "per", "via", "vs", "is", "are",
];

fn is_stopword(word: &str) -> bool {
    let stripped = word
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    STOPWORDS.contains(&stripped.as_str())
}

fn starts_capitalized(word: &str) -> bool {
    word.chars()
        .find(|c| c.is_alphabetic())
        .is_some_and(|c| c.is_uppercase())
}

/// A heading (already known to have >=3 total words) is title-cased when >=80% of its
/// non-stopword tokens start with an uppercase letter.
fn is_title_case_heading(text: &str) -> bool {
    let non_stop: Vec<&str> = text
        .split_whitespace()
        .filter(|w| !is_stopword(w))
        .collect();
    if non_stop.is_empty() {
        return false;
    }
    let capitalized = non_stop.iter().filter(|w| starts_capitalized(w)).count();
    capitalized * 100 >= 80 * non_stop.len()
}

/// (a) emoji-as-marker lines, (b) title-cased headings, and (c) thin sections, each an
/// independent sub-check that emits at most one diagnostic, anchored at its first offender.
/// Byte of the first marker line when at least two exist, else None.
///
/// Matching runs over `doc.masked` so fenced/inline code is excluded, but the match is then
/// re-checked against `ctx.source`: masking blanks inline code to spaces, which MANUFACTURES the
/// marker position this rule keys on. `` - `feat` → **Features** `` masks to `-        → ...`,
/// where the arrow now looks like it directly follows the bullet even though real content sits
/// between them in the source. Two of the rule's four hits across a 130-document corpus were this
/// artifact, and it affects every character in both classes, not just arrows. `ctx.source` is
/// already consulted elsewhere in this file (see the fence check in sub-check (c)) for the same
/// "the mask hides the truth" reason.
fn marker_run(doc: &crate::prose::ProseDoc, ctx: &LintContext, re: &Regex) -> Option<usize> {
    let mut lines = 0usize;
    let mut first = None;
    for m in re.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || ctx.source[byte..m.end()] != doc.masked[byte..m.end()] {
            continue;
        }
        lines += 1;
        first.get_or_insert(byte);
    }
    (lines >= 2).then(|| first.unwrap())
}

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    for (re, message) in [
        (&*EMOJI_MARKER_RE, "emoji used as heading / list marker"),
        (
            &*SYMBOL_MARKER_RE,
            "technical symbol used as heading / list marker",
        ),
    ] {
        if let Some(byte) = marker_run(doc, ctx, re) {
            let (line, col) = doc.line_col(byte);
            out.push(Diagnostic::at(rule, ctx, line, col, message.to_string()));
        }
    }

    let eligible: Vec<_> = doc
        .headings
        .iter()
        .filter(|h| h.text.split_whitespace().count() >= 3)
        .collect();
    let h = eligible.len();
    let title_cased = eligible
        .iter()
        .filter(|h| is_title_case_heading(&h.text))
        .count();
    let guard = SENTENCE_CASE_GUARD_RE
        .find_iter(&doc.masked)
        .any(|m| !doc.in_heading(m.start()) && !doc.in_frontmatter(m.start()));

    if h >= 3 && title_cased * 100 >= 75 * h && guard {
        if let Some(first) = eligible.iter().find(|h| is_title_case_heading(&h.text)) {
            let (line, col) = doc.line_col(first.byte_start);
            out.push(Diagnostic::at(
                rule,
                ctx,
                line,
                col,
                format!("title-case headings ({title_cased} of {h} headings)"),
            ));
        }
    }

    // (c) thin sections: a heading whose body (the text up to the next heading, or EOF) is
    // under ~25 words AND fewer than 2 sentences is a candidate. Excluded: the document's first
    // H1 (title), a heading immediately followed by a deeper-level heading (legitimate nesting --
    // the parent isn't expected to carry its own body), a body that's a code fence or table
    // (checked against ctx.source: `masked` blanks fence markers to spaces, which would hide
    // them), and reference/appendix-style sections. Fires once the document has 2+ candidates --
    // a single short section is completely normal.
    let first_h1 = doc.headings.iter().position(|hd| hd.level == 1);
    let mut thin_count = 0usize;
    let mut first_thin_byte = None;
    for (i, h) in doc.headings.iter().enumerate() {
        if Some(i) == first_h1 {
            continue;
        }
        if doc
            .headings
            .get(i + 1)
            .is_some_and(|next| next.level > h.level)
        {
            continue;
        }
        if is_reference_heading(&h.text) {
            continue;
        }
        let body_start = (h.byte_end + 1).min(doc.masked.len());
        let body_end = doc
            .headings
            .get(i + 1)
            .map_or(doc.masked.len(), |next| next.byte_start)
            .max(body_start);
        let raw_body = &ctx.source[body_start..body_end];
        if raw_body.contains("```")
            || raw_body.contains("~~~")
            || TABLE_SEPARATOR_RE.is_match(raw_body)
        {
            continue;
        }
        let body = HTML_COMMENT_RE.replace_all(&doc.masked[body_start..body_end], "");
        let words = body.split_whitespace().count();
        let sentences = SENTENCE_END_RE.find_iter(&body).count();
        if words < 25 && sentences < 2 {
            thin_count += 1;
            first_thin_byte.get_or_insert(h.byte_start);
        }
    }
    if thin_count >= 2 {
        let (line, col) = doc.line_col(first_thin_byte.unwrap());
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            format!(
                "{thin_count} headings guard sections too thin to need them (under ~25 words each)"
            ),
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
    fn flags_emoji_marker_lines() {
        let src = "- \u{1F680} Ship it fast.\n- \u{1F4CC} Pin the release notes.\n- Plain bullet without any emoji here.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP021");
    }

    /// Inline code blanks to spaces in `doc.masked`, so `` - `feat` → **Features** `` used to
    /// look like a bullet immediately followed by an arrow. Real content sits between them.
    #[test]
    fn blanked_inline_code_does_not_manufacture_a_marker() {
        let src = "- `feat` \u{2192} **Features**\n- `fix` \u{2192} **Bug Fixes**\n- `docs` \u{2192} **Documentation**\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_symbol_markers_separately_from_emoji() {
        let src = "\u{2192} Report the file path.\n\u{2192} Then delete the scratch file.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP021");
        assert_eq!(
            diags[0].message,
            "technical symbol used as heading / list marker"
        );
    }

    /// Separate counters: one emoji and one symbol are one of each, not two of anything.
    #[test]
    fn one_emoji_plus_one_symbol_does_not_reach_either_threshold() {
        let src = "- \u{1F680} Ship it fast.\n\u{2192} Then report the path.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// `\p{Emoji_Presentation}` misses text-default emoji (Emoji=Yes, needs U+FE0F to render
    /// colored); they are allow-listed explicitly.
    #[test]
    fn flags_text_default_emoji_missed_by_the_property_class() {
        let src = "- \u{26A0} Check the quota first.\n- \u{2714} Then confirm the rollout.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "emoji used as heading / list marker");
    }

    #[test]
    fn mid_sentence_arrows_and_excluded_symbols_are_not_markers() {
        for src in [
            // Technical notation, not a marker: the arrow is not in marker position.
            "The pipeline maps input \u{2192} output.\nIt then maps output \u{2192} report.\nFinally report \u{2192} archive.\n",
            // U+00BB is quotation punctuation.
            "\u{00BB} First quoted line.\n\u{00BB} Second quoted line.\n\u{00BB} Third quoted line.\n",
            // Box drawing is structural ASCII art, a different pattern entirely.
            "\u{250C} root\n\u{251C} child\n\u{2514} leaf\n",
        ] {
            assert!(
                diagnostics_for(src)
                    .iter()
                    .all(|d| !d.message.contains("marker")),
                "flagged as a marker: {src:?}"
            );
        }
    }

    #[test]
    fn flags_title_case_headings_with_sentence_case_guard() {
        // Each heading has a real (2-sentence) body so this stays a pure title-case test: with
        // no body at all, back-to-back empty H2s would also legitimately trip the (c) thin-
        // section sub-check below, which is a different concern than this test targets.
        let src = "# Getting Started With Setup\n\nThis is a normal sentence written in plain lowercase words.\n\n## Configuring Your Local Environment\n\nThis section walks through the environment variables read on startup. Override them locally when testing changes.\n\n## Reviewing The Final Report Now\n\nThis section explains where the report is generated. Reviewers check it before it ships to stakeholders.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP021");
    }

    #[test]
    fn empty_heading_does_not_glue_to_next_lines_emoji() {
        // An empty ATX heading line followed by an unrelated emoji-leading line must NOT count
        // as an emoji-after-heading match: the gap can't cross the newline. Only one real
        // emoji-marker line exists here (the bullet), so this must stay under the >=2 threshold.
        let src = "##\n\u{1F680}Ship it now.\n\n- \u{1F4CC} Pin the release notes.\n- Plain bullet without any emoji here.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_checkmark_and_arrow_markers() {
        let src = "\u{2713} Done with setup.\n\u{2192} Next steps follow below.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP021");
    }

    #[test]
    fn clean_single_title_cased_heading_and_single_emoji_bullet() {
        let src = "# Getting started with the linter\n\nThis document walks through everyday setup steps in plain sentence case.\n\n## Configuring Your Workspace Rules\n\n## Running the checks locally\n\n- Run the command from the project root.\n- \u{2728} A single decorative bullet marker.\n- Review the output for warnings.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_thin_stacked_sections() {
        // "Overview" has a real body (30 words); "Setup" and "Usage" are both near-empty --
        // 2 thin candidates meets the >=2 floor, anchored at the first one ("Setup").
        let src = "# Guide\n\n## Overview\n\nThis section gives real explanatory detail about the guide so that it is clearly not a thin section by either word count or sentence count in this particular paragraph today.\n\n## Setup\n\nDone.\n\n## Usage\n\nRun it.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP021");
        assert!(diags[0].message.contains("thin"));
    }

    #[test]
    fn clean_single_thin_section_below_floor() {
        // Only "Setup" is thin here; a single thin section under an otherwise normal document
        // is completely ordinary and must not fire.
        let src = "# Guide\n\n## Overview\n\nThis section gives real explanatory detail about the guide so that it is clearly not a thin section by either word count or sentence count in this particular paragraph today.\n\n## Setup\n\nDone.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_nested_and_reference_heading_thin_bodies_excluded() {
        // "Configuration" has an empty body of its own but is immediately followed by a deeper
        // heading (legitimate nesting); "Appendix" is a reference-style section. Neither counts
        // toward the thin-section total, so this must stay clean even though both headings would
        // otherwise look near-empty.
        let src = "# Guide\n\n## Configuration\n\n### Basic Setup\n\nThis subsection has plenty of real detail about configuration options so that it is clearly not a thin section under any reasonable word or sentence count here today.\n\n## Appendix\n\nSee the reference table listed above for details.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_code_and_table_bodies_excluded() {
        // Both sections are near-empty by word count, but a body that's a code fence or a table
        // is a legitimate structural pattern, not a thin section.
        let src = "# Guide\n\n## Example\n\n```\nfoo()\n```\n\n## Data\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
