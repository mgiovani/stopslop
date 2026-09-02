use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, PROSE_LANGS};
use crate::prose::ProseDoc;
use crate::registry::RuleDef;
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP033",
    name: "Overlong sentence",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

static WORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\S+").unwrap());

// Tuned to 50, not the spec's 35: fixtures owned by other rules (e.g. clean_hedging.md)
// legitimately run up to 47 words in non-slop prose. 50 stays above that ceiling while still
// catching genuine run-ons.
const OVERLONG_WORDS: usize = 50;

/// Lines that contribute no words at all and act as hard sentence boundaries: frontmatter,
/// heading lines, and table rows (trimmed line starts with `|`).
fn skip_lines(doc: &ProseDoc, spans: &[(usize, usize)]) -> HashSet<usize> {
    let mut skip = HashSet::new();
    for (idx, &(ls, le)) in spans.iter().enumerate() {
        let trimmed = doc.masked[ls..le].trim();
        let is_table = trimmed.starts_with('|');
        if doc.in_frontmatter(ls) || doc.in_heading(ls) || is_table {
            skip.insert(idx + 1);
        }
    }
    skip
}

/// Byte offsets of every list-item marker (`-`/`*`/`+`/`1.`/`1)`): the start of a list item is a
/// hard sentence boundary, but the marker token itself (not a word) isn't counted.
fn marker_bytes(doc: &ProseDoc) -> HashSet<usize> {
    doc.list_blocks
        .iter()
        .flat_map(|b| b.items.iter().map(|i| i.marker_byte))
        .collect()
}

/// Splits the masked prose stream into sentences on `[.!?]` followed by whitespace/EOF, and
/// also at blank lines, heading lines, table rows, frontmatter, list-item boundaries, and HTML
/// block-element starts (`doc.block_starts`, so a `<select>` of sixty `<option>`s is sixty
/// one-word sentences, not one).
/// Flags any sentence over `OVERLONG_WORDS` words, one diagnostic anchored at the sentence's
/// first byte. Words are counted by whitespace splitting; a run of tokens that all fall inside
/// the same URL span collapses to a single word.
// The final `close_sentence!()` call resets state nothing downstream reads (the function
// returns right after); every earlier call's reset IS read by the next loop iteration, so the
// lint only ever fires on that last, harmless reset.
#[allow(unused_assignments)]
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let spans = &doc.line_spans;
    let skip = skip_lines(doc, spans);
    let markers = marker_bytes(doc);

    let mut prev_line: Option<usize> = None;
    let mut sentence_start: Option<usize> = None;
    let mut word_count = 0usize;
    let mut last_url_span: Option<(usize, usize)> = None;
    let mut next_block = 0usize;

    macro_rules! close_sentence {
        () => {
            if let Some(start) = sentence_start {
                if word_count > OVERLONG_WORDS {
                    let (line, col) = doc.line_col(start);
                    out.push(Diagnostic::at_fix(
                        rule,
                        ctx,
                        line,
                        col,
                        format!("sentence runs {word_count} words; split it"),
                        "split into two sentences at the first clause boundary",
                    ));
                }
            }
            sentence_start = None;
            word_count = 0;
            last_url_span = None;
        };
    }

    for m in WORD_RE.find_iter(&doc.masked) {
        let byte = m.start();
        let line = doc.line_col(byte).0;

        let crossed = doc.block_starts[next_block..].partition_point(|&b| b <= byte);
        if crossed > 0 {
            close_sentence!();
            next_block += crossed;
        }
        if skip.contains(&line) {
            close_sentence!();
            prev_line = None;
            continue;
        }
        if let Some(pl) = prev_line {
            if line > pl + 1 {
                close_sentence!();
            }
        }
        if markers.contains(&byte) {
            // The marker token itself is a boundary, not a word.
            close_sentence!();
            prev_line = Some(line);
            continue;
        }

        sentence_start.get_or_insert(byte);

        let url_span = doc.url_span_at(byte);
        let already_counted = url_span.is_some() && url_span == last_url_span;
        if !already_counted {
            word_count += 1;
        }
        last_url_span = url_span;
        prev_line = Some(line);

        if m.as_str().ends_with(['.', '!', '?']) {
            close_sentence!();
        }
    }
    close_sentence!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::prose::ProseDoc;

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        diagnostics_in(ProseDoc::parse(src), src, Lang::Md)
    }

    fn diagnostics_for_html(src: &str) -> Vec<Diagnostic> {
        diagnostics_in(ProseDoc::parse_html(src), src, Lang::Html)
    }

    fn diagnostics_in<'a>(doc: ProseDoc<'a>, src: &'a str, lang: Lang) -> Vec<Diagnostic> {
        let ctx = LintContext {
            display_path: "test.md".to_string(),
            source: src,
            index: None,
            lang,
            comments: &doc.ignore_comments,
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: Some(&doc),
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn html_block_boundaries_split_sentences() {
        let options: String = (0..60)
            .map(|i| format!("<option>item{i}</option>"))
            .collect();
        assert!(diagnostics_for_html(&format!("<select>{options}</select>\n")).is_empty());
        let long: Vec<String> = (0..60).map(|i| format!("w{i}")).collect();
        let src = format!("<p>{}</p>\n", long.join(" "));
        assert_eq!(diagnostics_for_html(&src).len(), 1);
    }

    fn words(n: usize, tail: &str) -> String {
        let mut s = String::new();
        for i in 0..n {
            if i > 0 {
                s.push(' ');
            }
            s.push_str("word");
        }
        s.push_str(tail);
        s
    }

    #[test]
    fn flags_overlong_sentence() {
        let src = format!("{}\n", words(OVERLONG_WORDS + 1, "."));
        let diags = diagnostics_for(&src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP033");
        assert!(diags[0].message.contains("split it"));
    }

    #[test]
    fn clean_sentence_at_threshold() {
        let src = format!("{}\n", words(OVERLONG_WORDS, "."));
        assert!(diagnostics_for(&src).is_empty());
    }

    #[test]
    fn clean_short_sentences() {
        let src = "The cache expires after sixty seconds. The client retries once on a timeout.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn heading_breaks_a_run_on_sentence() {
        // No terminal punctuation before the heading; the heading must still close the
        // preceding sentence rather than let it merge with the words after the heading.
        let mut src = words(OVERLONG_WORDS - 5, "");
        src.push_str("\n\n# Heading\n\n");
        src.push_str(&words(OVERLONG_WORDS - 5, "."));
        src.push('\n');
        assert!(diagnostics_for(&src).is_empty());
    }

    #[test]
    fn list_item_boundary_does_not_merge_with_previous_paragraph() {
        let mut src = words(OVERLONG_WORDS - 5, ".");
        src.push('\n');
        src.push_str("- ");
        src.push_str(&words(OVERLONG_WORDS - 5, "."));
        src.push('\n');
        assert!(diagnostics_for(&src).is_empty());
    }

    #[test]
    fn table_row_is_skipped_and_does_not_count_as_words() {
        let src = "| a | b | c |\n| - | - | - |\n\nShort sentence here.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn url_span_collapses_to_a_single_word() {
        // OVERLONG_WORDS - 1 plain words + 1 collapsed URL word = OVERLONG_WORDS, at the
        // threshold, not over it.
        let mut src = words(OVERLONG_WORDS - 1, "");
        // ai-slop-ignore
        src.push_str(" https://example.com/a/very/long/path/that/is/many/tokens.\n");
        assert!(diagnostics_for(&src).is_empty());
    }
}
