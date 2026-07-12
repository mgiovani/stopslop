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
    default_on: false,
    path_gated: false,
    check,
};

/// The marker char class: `\p{Emoji_Presentation}` (supported by the `regex` crate's default
/// unicode feature, verified against 1.10) plus a small allow-list of common text-presentation
/// symbols the catalog explicitly cites alongside emoji (✓/→ are Emoji=Yes but
/// Emoji_Presentation=No in Unicode, so the property class alone misses them).
const MARKER: &str = r"(?:\p{Emoji_Presentation}|[✓→])";

/// Emoji directly after heading hashes, opening a bullet, or leading a bare line (e.g. a
/// decorative checkmark/arrow/pin marker). `(?m)` so `^` is per-line. The gap between the
/// marker-opener (`#`s / bullet char) and the emoji is `[ \t]+`, NOT `\s+`: `\s` matches `\n`,
/// so an unconstrained gap can glue an empty heading/bullet line to an unrelated emoji-leading
/// line on the NEXT line (a phantom cross-line match) -- the same "`\s` matches `\n`" anchoring
/// bug this codebase already guards against elsewhere (see residue.rs's `[ \t]*` opener anchor).
static EMOJI_MARKER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?m)^[ \t]{{0,3}}(?:#{{1,6}}[ \t]+{MARKER}|[-*+][ \t]+{MARKER}|{MARKER}[ \t]+\S)"
    ))
    .unwrap()
});

/// Guard for (b): at least one pair of consecutive lowercase words somewhere outside heading
/// lines — evidence the document actually uses sentence case, so an all-caps/style doc (which
/// has nothing to contrast against) never fires.
static SENTENCE_CASE_GUARD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[a-z]{2,}\s+[a-z]{2,}\b").unwrap());

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

/// (a) emoji-as-marker lines and (b) title-cased headings, each an independent sub-check that
/// emits at most one diagnostic, anchored at its first offender.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    let mut emoji_lines = 0usize;
    let mut first_emoji_byte = None;
    for m in EMOJI_MARKER_RE.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) {
            continue;
        }
        emoji_lines += 1;
        first_emoji_byte.get_or_insert(byte);
    }
    if emoji_lines >= 2 {
        let (line, col) = doc.line_col(first_emoji_byte.unwrap());
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "emoji used as heading / list marker".to_string(),
        ));
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

    #[test]
    fn flags_title_case_headings_with_sentence_case_guard() {
        let src = "# Getting Started With Setup\n\nThis is a normal sentence written in plain lowercase words.\n\n## Configuring Your Local Environment\n\n## Reviewing The Final Report Now\n";
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
}
