use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose::ProseDoc;
use crate::registry::RuleDef;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP030",
    name: "Dramatic fragmentation / robotic rhythm",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: false,
    path_gated: false,
    check,
};

static STOCK_FRAGMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:that'?s it|that'?s the whole thing|simple as that|full stop|period|end of story)[.!]?$")
        .unwrap()
});
static CONJ_OPENER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(and|but|or)\b").unwrap());

/// Sentences whose word count difference (max - min) is at most this are "robotically uniform".
const RHYTHM_SPREAD: usize = 2;

fn line_spans(masked: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = 0usize;
    for (i, b) in masked.bytes().enumerate() {
        if b == b'\n' {
            spans.push((start, i));
            start = i + 1;
        }
    }
    spans.push((start, masked.len()));
    spans
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn is_horizontal_rule(line: &str) -> bool {
    let stripped: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    stripped.len() >= 3
        && (stripped.bytes().all(|b| b == b'-')
            || stripped.bytes().all(|b| b == b'*')
            || stripped.bytes().all(|b| b == b'_'))
}

static REF_DEF_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s{0,3}\[[^\]]+\]:\s*\S").unwrap());
static COMMENT_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*<!--.*-->\s*$").unwrap());
static HEADING_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s{0,3}#{1,6}\s+\S").unwrap());
static LIST_ITEM_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s{0,3}(?:[-*+]|\d{1,9}[.)])\s+\S").unwrap());

/// A piped table row, or a table separator row (`|---|:--:|`).
fn is_table_line(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') || t.matches('|').count() >= 2
}

struct Block {
    text: String,
    first_byte: usize,
}

/// All paragraph blocks in the document: maximal contiguous non-blank line runs, excluding
/// anything that isn't ordinary prose -- code fences (already blanked to blank-looking lines in
/// `doc.masked`), frontmatter, headings, list items, table rows, horizontal rules, link-reference
/// definitions, and HTML-comment-only lines. A block containing even one such line is dropped
/// entirely rather than partially salvaged: bullet lists, tables, and headings must never be
/// treated as sentences (spec requirement), and a block that mixes prose with one of these is
/// rare enough that skipping it whole is the conservative, low-risk choice.
fn paragraph_blocks(doc: &ProseDoc) -> Vec<Block> {
    let masked = &doc.masked;
    let spans = line_spans(masked);
    let mut blocks = Vec::new();
    let mut i = 0usize;
    while i < spans.len() {
        if is_blank(&masked[spans[i].0..spans[i].1]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < spans.len() && !is_blank(&masked[spans[i].0..spans[i].1]) {
            i += 1;
        }
        let end = i;
        let lines: Vec<&str> = (start..end)
            .map(|j| &masked[spans[j].0..spans[j].1])
            .collect();
        let disqualified = lines.iter().enumerate().any(|(k, l)| {
            HEADING_LINE.is_match(l)
                || LIST_ITEM_LINE.is_match(l)
                || is_table_line(l)
                || is_horizontal_rule(l)
                || REF_DEF_LINE.is_match(l)
                || COMMENT_LINE.is_match(l)
                || doc.in_frontmatter(spans[start + k].0)
        });
        if disqualified {
            continue;
        }
        let mut text = String::new();
        let first_byte = spans[start].0 + (lines[0].len() - lines[0].trim_start().len());
        for line in &lines {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(line.trim());
        }
        blocks.push(Block { text, first_byte });
    }
    blocks
}

/// Splits paragraph text into sentences on runs of `.`/`!`/`?` followed by whitespace or
/// end-of-string (so "3.14" and other mid-token punctuation, never followed by whitespace right
/// there, is not a boundary). Each returned slice keeps its own trailing punctuation.
/// ponytail: no abbreviation list (e.g. "e.g."); a real fixture would need it to upgrade.
fn split_sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if matches!(bytes[i], b'.' | b'!' | b'?') {
            let mut j = i + 1;
            while j < bytes.len() && matches!(bytes[j], b'.' | b'!' | b'?') {
                j += 1;
            }
            if j >= bytes.len() || bytes[j] == b' ' || bytes[j] == b'\t' {
                let sentence = text[start..j].trim();
                if !sentence.is_empty() {
                    out.push(sentence);
                }
                start = j;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        out.push(tail);
    }
    out
}

fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// (a) dramatic fragmentation: a stock mic-drop fragment anywhere, OR 3+ consecutive very short
/// (<=5 word) sentences, OR 2+ consecutive sentences opening with And/But/Or.
fn dramatic_fragmentation(sentences: &[&str]) -> Option<&'static str> {
    for s in sentences {
        if STOCK_FRAGMENT.is_match(s.trim()) {
            return Some("dramatic fragmentation: stock mic-drop fragment");
        }
    }
    let mut run = 0usize;
    for s in sentences {
        if word_count(s) <= 5 {
            run += 1;
            if run >= 3 {
                return Some(
                    "dramatic fragmentation: three or more consecutive very short sentences",
                );
            }
        } else {
            run = 0;
        }
    }
    let mut conj_run = 0usize;
    for s in sentences {
        if CONJ_OPENER.is_match(s.trim()) {
            conj_run += 1;
            if conj_run >= 2 {
                return Some(
                    "dramatic fragmentation: consecutive sentences opening with And/But/Or",
                );
            }
        } else {
            conj_run = 0;
        }
    }
    None
}

/// (b) robotic rhythm: 3+ sentences (anywhere in the paragraph, not necessarily consecutive)
/// sharing the same opening word, OR 4+ sentences whose word counts are all within
/// `RHYTHM_SPREAD` of each other. Evaluated over the WHOLE paragraph's sentence set (not a
/// subset search): a single long outlier sentence is enough to keep an otherwise-uniform
/// paragraph silent, which is the conservative choice that also matches ordinary prose (a real
/// paragraph is rarely uniform end to end).
fn robotic_rhythm(sentences: &[&str]) -> Option<String> {
    let mut openers: HashMap<String, usize> = HashMap::new();
    for s in sentences {
        if let Some(w) = s.split_whitespace().next() {
            let key = w
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            if !key.is_empty() {
                *openers.entry(key).or_insert(0) += 1;
            }
        }
    }
    if let Some(word) = openers.iter().find(|&(_, &n)| n >= 3).map(|(w, _)| w) {
        return Some(format!(
            "robotic rhythm: three or more sentences open with \"{word}\""
        ));
    }

    if sentences.len() >= 4 {
        let counts: Vec<usize> = sentences.iter().map(|s| word_count(s)).collect();
        let max = *counts.iter().max().unwrap();
        let min = *counts.iter().min().unwrap();
        if max - min <= RHYTHM_SPREAD {
            return Some(
                "robotic rhythm: sentence lengths in this paragraph barely vary".to_string(),
            );
        }
    }
    None
}

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    for block in paragraph_blocks(doc) {
        let sentences = split_sentences(&block.text);
        if sentences.is_empty() {
            continue;
        }
        if let Some(msg) = dramatic_fragmentation(&sentences) {
            let (line, col) = doc.line_col(block.first_byte);
            out.push(Diagnostic::at(rule, ctx, line, col, msg));
            continue;
        }
        if let Some(msg) = robotic_rhythm(&sentences) {
            let (line, col) = doc.line_col(block.first_byte);
            out.push(Diagnostic::at(rule, ctx, line, col, msg));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn flags_three_consecutive_short_sentences() {
        let src = "It works. It scales. It ships. The rest of the rollout needs no further changes from anyone on the team this quarter.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP030");
    }

    #[test]
    fn clean_varied_sentence_lengths() {
        let src = "The team shipped the update on Friday afternoon after a short review cycle. Only one small bug turned up during testing, and it was fixed within the hour. Rollback was never needed because the new version behaved exactly as expected in staging.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_stock_mic_drop_fragment() {
        let src = "The rollout took months of careful planning and testing across every region. That's it.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn clean_no_stock_fragment() {
        let src = "The rollout took months of careful planning and testing across every region.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_consecutive_and_but_openers() {
        let src = "We tried the change on staging first. And it worked well there. But nothing changed for the production dashboard afterward, oddly enough.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn clean_conjunction_mid_sentence_not_opener() {
        let src = "We tried the change on staging and it worked well there, but nothing changed for the production dashboard afterward.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_same_opening_word_three_times() {
        let src = "The service starts quickly. The service scales well. The service costs little to run every month for most teams.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP030");
    }

    #[test]
    fn clean_varied_opening_words() {
        let src = "The service starts quickly. Most teams find it easy to run. Costs stay low for typical workloads across the board.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_four_sentences_with_uniform_length() {
        // Each sentence is >5 words (so the short-sentence-run sub-check can't preempt this),
        // no repeated opening word, but all four word counts (9,9,8,8) sit within the
        // robotic-rhythm spread -- isolates the length-uniformity signal specifically.
        let src = "The cache module holds the hottest keys in memory. A disk layer holds every remaining key on disk. One background timer moves stale entries out nightly. Another small job promotes fresh entries back in.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP030");
    }

    #[test]
    fn clean_four_sentences_with_varied_length() {
        let src = "The cache holds hot keys. The disk-backed layer behind it holds the much larger set of cold keys that rarely get touched. A background timer moves stale entries out. Fresh writes go straight to the fast tier first.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn list_items_are_not_treated_as_sentences() {
        // Three very short "sentences" in a row, but they are list items, not paragraph
        // prose, and must not be scanned as consecutive short sentences.
        let src = "- Ship it.\n- Test it.\n- Tag it.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn table_rows_are_not_treated_as_sentences() {
        let src = "| Name | Value |\n| ---- | ----- |\n| a | 1 |\n| b | 2 |\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn headings_are_not_treated_as_sentences() {
        let src = "# Ship it. Test it. Tag it.\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
