use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::PROSE_LANGS;
use crate::prose::first_byte_per_line;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP013",
    name: "Unfilled template placeholder text",
    tier: Tier::A,
    langs: PROSE_LANGS,
    default_on: true,
    path_gated: false,
    check,
};

/// (1) Bracketed instructional placeholders: just the opening `[` + keyword. The matching close
/// `]` is found by hand (see `find_bracket_close`) rather than with `[^\]]*\]`, which stops at
/// the FIRST `]` and is defeated by nested brackets in real link text (`[link to [our
/// repo]](url)`). The post-match rejection below (no lookaround in the `regex` crate) is what
/// keeps `[click here](url)` / `[link to docs](url)` from firing even though "link to" is itself
/// a listed keyword.
static RE_BRACKET_OPEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\[(insert|describe|add|replace|your name|company name|entertainer'?s name|link to|paste|tbd|todo|placeholder|xxx)\b")
        .unwrap()
});

/// Byte offset of the `]` that closes the `[` this match opened, tracking real nesting depth
/// from `start` (the position right after the opening `[`+keyword). Returns `None` if the
/// brackets never balance before EOF.
fn find_bracket_close(s: &str, start: usize) -> Option<usize> {
    let mut depth = 1i32;
    for (i, ch) in s[start..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + i);
                }
            }
            _ => {}
        }
    }
    None
}

/// (2) ALL-CAPS fill-in tokens. Case-sensitive: requires a fill-in verb/possessive prefix or a
/// `_HERE` suffix, so ordinary constants like `DATABASE_URL`/`MAX_RETRIES` never match.
static RE_ALLCAPS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(INSERT|PASTE|ADD|REPLACE|YOUR|SOURCE|EXAMPLE)_[A-Z0-9_]+\b|\b[A-Z0-9]+_HERE\b")
        .unwrap()
});

/// (3) Placeholder dates: literal XX stubs, or an `access-date=`/`date=` key still holding a
/// TBD/TODO/XXXX/stub value.
static RE_DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b20\d{2}-[Xx]{2}-[Xx]{2}\b|\b20\d{2}-\d{2}-[Xx]{2}\b|\b(access-?date|date)\s*=\s*(tbd|todo|xxxx|20\d{2}-[Xx]{2}-[Xx]{2})\b")
        .unwrap()
});

/// (4) HTML-comment fill instructions, e.g. `<!-- Add citation -->`.
static RE_HTML_COMMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<!--\s*(add|insert|todo|fill in|replace|describe)\b[^>]*-->").unwrap()
});

/// Scope: headings in scope, frontmatter IN SCOPE (placeholder dates commonly appear as
/// `date: 2025-XX-XX` in YAML frontmatter), URLs in scope. Only code (already blanked in
/// `doc.masked`) is excluded. One diagnostic per matching line, first byte wins.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let masked = doc.masked.as_str();
    let mut bytes: Vec<usize> = Vec::new();

    for m in RE_BRACKET_OPEN.find_iter(masked) {
        let Some(close) = find_bracket_close(masked, m.end()) else {
            continue;
        };
        // No lookaround in the `regex` crate: inspect the char right after the real closing `]`
        // by hand and reject it if this is really a markdown inline link `[text](url)`,
        // reference link `[text][ref]`, or reference/footnote definition `[text]:` rather than a
        // placeholder.
        let after = masked[close + 1..].chars().next();
        if matches!(after, Some('(') | Some('[') | Some(':')) {
            continue;
        }
        bytes.push(m.start());
    }
    for re in [&*RE_ALLCAPS, &*RE_DATE, &*RE_HTML_COMMENT] {
        bytes.extend(re.find_iter(masked).map(|m| m.start()));
    }

    let by_line = first_byte_per_line(doc, bytes.into_iter());
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "unfilled template placeholder",
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
    fn flags_bracketed_placeholder() {
        let diags = diagnostics_for("Written by [Your Name], a contributor to the blog.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP013");
    }

    #[test]
    fn flags_allcaps_insert_token() {
        let diags = diagnostics_for("Source link: INSERT_SOURCE_URL_30\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_allcaps_here_token() {
        let diags = diagnostics_for("Now playing: PASTE_SPOTIFY_TRACK_URL_HERE\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_placeholder_date_in_frontmatter() {
        let diags = diagnostics_for("---\ntitle: Draft\ndate: 2025-XX-XX\n---\nBody text here.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_html_comment_instruction() {
        let diags =
            diagnostics_for("Some intro text.\n<!-- Add citation -->\nMore text follows.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn ignores_markdown_link_even_with_keyword_text() {
        let diags = diagnostics_for(
            "Background is in the [link to our changelog](https://example.com/changelog) below.\n", // ai-slop-ignore
        );
        assert!(
            diags.is_empty(),
            "bracket-then-paren must be rejected as a real link"
        );
    }

    #[test]
    fn ignores_nested_bracket_markdown_link() {
        // The inner `[our repo]` must not fool the rejection check into inspecting the wrong
        // (inner) closing bracket's successor -- the real outer close is followed by `(`.
        let diags = diagnostics_for(
            "See the [link to [our repo]](https://github.com/example/repo) for details.\n",
        );
        assert!(
            diags.is_empty(),
            "nested-bracket link text must still be rejected as a real link"
        );
    }

    #[test]
    fn ignores_ordinary_link_and_ref_def() {
        let diags = diagnostics_for(
            "See [click here](https://example.com/docs) or [note]: https://example.com/ref\n", // ai-slop-ignore
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn ignores_ordinary_screaming_case_constants() {
        let diags =
            diagnostics_for("Runtime config reads DATABASE_URL and MAX_RETRIES at startup.\n");
        assert!(diags.is_empty());
    }
}
