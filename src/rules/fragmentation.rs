use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, PROSE_LANGS};
use crate::prose::{CodeSpan, ProseDoc};
use crate::registry::RuleDef;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP030",
    name: "Dramatic fragmentation / robotic rhythm",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

static STOCK_FRAGMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:that'?s it|that'?s the whole thing|simple as that|full stop|period|end of story)[.!]?$")
        .unwrap()
});
static CONJ_OPENER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(and|but|or)(?-u:\b)").unwrap());

/// Sentences whose word count difference (max - min) is at most this are "robotically uniform".
const RHYTHM_SPREAD: usize = 2;

/// Determiners and prepositions excluded from `robotic_rhythm`'s opener check: on a pt-BR human
/// corpus (issue #30 phase 2), 1031 of this rule's messages were "three or more sentences open
/// with X" where X is one of these words (`o` 291, `a` 294, `em` 145, `no` 40, `os` 36, `na` 22
/// occurrences) -- three sentences opening on the same article or preposition is unremarkable in
/// Portuguese, not anaphora. The same fix reads on English text (`the`/`a`/`an` lead an ordinary
/// paragraph constantly): measured on this repo's own corpora with all three SLOP030 fixes
/// applied together (`--select SLOP030`, distinct-file counts), `ptbr-human` went from 226 to 163
/// of 323 files, `en-human` from 98 to 79 of 100 files, `ptbr-generated2` from 20 to 15 of 94, and
/// `ptbr-generated` from 5 to 0 of 60 -- see the crate's CHANGELOG for the full table. Pronouns
/// are deliberately NOT included (`it`, `this`, `ele`, `ela`, ...): three sentences opening on the
/// same pronoun is exactly the anaphora this signal exists to catch.
const OPENER_FUNCTION_WORDS: &[&str] = &[
    // English
    "the", "a", "an", "in", "on", "at", "of", "to", "for", "by", "with", "from", "as", "into",
    "over", "under", // Portuguese
    "o", "a", "os", "as", "um", "uma", "uns", "umas", "em", "no", "na", "nos", "nas", "de", "do",
    "da", "dos", "das", "ao", "à", "aos", "às", "com", "por", "para", "pelo", "pela", "pelos",
    "pelas", "sem", "sob", "sobre", "entre",
];

/// True when `s` (trimmed) is a lone initial -- one letter, optionally followed by a single
/// trailing period (`"A."`, `"M"`) -- the shape `split_sentences` produces when it cuts an
/// abbreviation like "A. M. Kuchling" into three "sentences". Measured on the pt-BR human corpus
/// (issue #30 phase 2): 89 of the short-run messages were this shape rather than genuine dramatic
/// fragmentation.
fn is_lone_initial(s: &str) -> bool {
    let t = s.trim();
    let core = t.strip_suffix('.').unwrap_or(t);
    let mut chars = core.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c.is_alphabetic())
}

/// True when `s` (trimmed) opens with an em dash or en dash -- a travessão dialogue turn, the
/// same convention SLOP018's own travessão exemption recognizes (see `emdash.rs`). A run of short
/// dialogue lines ("— Sim. — Não. — Talvez.") is conversation, not dramatic fragmentation.
fn is_dialogue_turn(s: &str) -> bool {
    s.trim_start().starts_with(['—', '–'])
}

/// Shared with recap.rs (SLOP029), which scans the document from the opposite end for the same
/// notion of "a blank line".
pub(crate) fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// Shared with recap.rs (SLOP029): a line consisting only of `-`/`*`/`_` repeated (optionally
/// spaced), e.g. `---`, `* * *`.
pub(crate) fn is_horizontal_rule(line: &str) -> bool {
    let stripped: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    stripped.len() >= 3
        && (stripped.bytes().all(|b| b == b'-')
            || stripped.bytes().all(|b| b == b'*')
            || stripped.bytes().all(|b| b == b'_'))
}

/// Shared with recap.rs (SLOP029).
pub(crate) static REF_DEF_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s{0,3}\[[^\]]+\]:\s*\S").unwrap());
/// Shared with recap.rs (SLOP029).
pub(crate) static COMMENT_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*<!--.*-->\s*$").unwrap());
/// Shared with recap.rs (SLOP029).
pub(crate) static HEADING_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s{0,3}#{1,6}\s+\S").unwrap());
static LIST_ITEM_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s{0,3}(?:[-*+]|\d{1,9}[.)])\s+\S").unwrap());

/// A piped table row, or a table separator row (`|---|:--:|`).
fn is_table_line(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') || t.matches('|').count() >= 2
}

pub(crate) struct Block {
    pub(crate) text: String,
    pub(crate) first_byte: usize,
    /// Exclusive end in the ORIGINAL document. `text` is joined and trimmed, so its length does
    /// not map back to source bytes; callers that need to test "is this byte inside prose?"
    /// (see rules::synonym_rotation) need the real span.
    pub(crate) end_byte: usize,
}

/// All paragraph blocks in the document: maximal contiguous non-blank line runs, excluding
/// anything that isn't ordinary prose -- code fences (already blanked to blank-looking lines in
/// `doc.masked`), frontmatter, headings, list items, table rows, horizontal rules, link-reference
/// definitions, and HTML-comment-only lines. A block containing even one such line is dropped
/// entirely rather than partially salvaged: bullet lists, tables, and headings must never be
/// treated as sentences (spec requirement), and a block that mixes prose with one of these is
/// rare enough that skipping it whole is the conservative, low-risk choice.
pub(crate) fn paragraph_blocks(doc: &ProseDoc) -> Vec<Block> {
    if let Some(ranges) = &doc.paragraphs {
        return html_blocks(doc, ranges);
    }
    let masked = &doc.masked;
    let spans = &doc.line_spans;
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
            doc.in_heading(spans[start + k].0)
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
        for (k, _) in lines.iter().enumerate() {
            let (ls, le) = spans[start + k];
            let line = with_code_placeholders(&doc.masked, ls, le, &doc.code_spans);
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(line);
        }
        blocks.push(Block {
            text,
            first_byte,
            end_byte: spans[end - 1].1,
        });
    }
    blocks
}

/// HTML paragraphs come pre-cut from the parse (`ProseDoc::paragraphs`, one leaf block element
/// each), so the blank-line walk above never runs on a masked HTML stream, where tags are blank
/// runs and every `<p>` in a section would glue into one block. A paragraph with no visible text
/// (`<p><img></p>`) is dropped.
fn html_blocks(doc: &ProseDoc, ranges: &[(usize, usize)]) -> Vec<Block> {
    ranges
        .iter()
        .filter_map(|&(s, e)| {
            let raw = with_code_placeholders(&doc.masked, s, e, &doc.code_spans);
            let text = raw.split_whitespace().collect::<Vec<_>>().join(" ");
            if text.is_empty() {
                return None;
            }
            let first_byte = doc.masked[s..e]
                .find(|c: char| !c.is_whitespace())
                .map(|i| s + i)
                .into_iter()
                .chain(
                    doc.code_spans
                        .iter()
                        .map(|c| c.start)
                        .filter(|c| (s..e).contains(c)),
                )
                .min()
                .unwrap_or(s);
            Some(Block {
                text,
                first_byte,
                end_byte: e,
            })
        })
        .collect()
}

/// The placeholder standing in for each token of a blanked inline `code` span. An ordinary word,
/// so a sentence built around code reads like prose: it can open a sentence, and the span
/// contributes the same number of words a reader would count in it.
const CODE_PLACEHOLDER: &str = "code";

/// Rebuilds one masked line with each inline-code span replaced by [`CODE_PLACEHOLDER`].
///
/// `doc.masked` blanks inline code to spaces, which erases the difference between "there was code
/// here" and "there was nothing here". Every check in this module keys on position or length --
/// which word opens a sentence, how many words a sentence has -- so reading the blanks as absence
/// misreports both. "Run `stopslop --format json .` now." masks to "Run    now." -- two words,
/// and a sentence that only appears to open with "Run" by luck of what got blanked. With
/// placeholders it reads "Run code code code code now.", matching the four tokens a reader sees
/// inside the backticks.
fn with_code_placeholders(masked: &str, ls: usize, le: usize, code_spans: &[CodeSpan]) -> String {
    let mut out = String::new();
    let mut cursor = ls;
    for span in code_spans {
        if span.end <= ls || span.start >= le {
            continue;
        }
        let (cs, ce) = (span.start.max(ls), span.end.min(le));
        out.push_str(&masked[cursor..cs]);
        for i in 0..span.words {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(CODE_PLACEHOLDER);
        }
        cursor = ce;
    }
    out.push_str(&masked[cursor..le]);
    out
}

/// Tokens whose trailing period never ends a sentence, matched case-insensitively against the
/// token immediately before the period `split_sentences` is about to treat as a boundary
/// ("Inc.", "vs.", "Dr.", ...). Without this, "We compared Acme Inc. vs. Beta Corp. and found
/// ..." split into four "sentences" and could trip SLOP030's short-sentence-run check on
/// ordinary prose.
///
/// Language-neutral rather than natlang-gated: an abbreviation's period is a word-internal mark
/// no matter which language surrounds it, both lists together are two dozen short tokens, and a
/// missed abbreviation only produces the SAME over-splitting this const exists to fix -- there is
/// no false-negative cost to checking both unconditionally. `e.g`/`i.e` cover what this module's
/// own scan actually extracts: the FIRST period of "e.g." is never a boundary candidate (nothing
/// follows it but "g", not whitespace), so the token in front of the real candidate -- the
/// SECOND period -- is "e.g" with its own first period already inside it. `e.g.`/`i.e.` are kept
/// too so the check also matches if a caller ever compares the token as written in running prose,
/// trailing period included.
const ABBREVIATIONS: &[&str] = &[
    // English
    // "no" (as in "No. 5") stays out: it ends ordinary sentences ("The answer was no.").
    "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "etc", "approx", "inc", "corp", "ltd",
    "co", "fig", "e.g", "e.g.", "i.e", "i.e.", "cf", "al",
    // Portuguese additions not already covered above ("dr", "sr", "prof" are shared with English)
    "sra", "srta", "dra", "profa", "eng", "séc", "sec", "a.c", "d.c", "p.ex", "pág", "ed", "ex",
    "nº",
];

/// The whitespace-delimited token immediately before byte offset `i` in `text`, with any leading
/// non-alphanumeric characters (an opening quote or paren) trimmed off. `is_abbreviation`'s only
/// caller.
fn preceding_token(text: &str, i: usize) -> &str {
    let start = text[..i].rfind(char::is_whitespace).map_or(0, |p| {
        p + text[p..].chars().next().map_or(1, char::len_utf8)
    });
    text[start..i].trim_start_matches(|c: char| !c.is_alphanumeric())
}

fn is_abbreviation(token: &str) -> bool {
    !token.is_empty() && ABBREVIATIONS.contains(&token.to_lowercase().as_str())
}

/// Splits paragraph text into sentences on runs of `.`/`!`/`?` followed by whitespace or
/// end-of-string (so "3.14" and other mid-token punctuation, never followed by whitespace right
/// there, is not a boundary), except a single `.` directly after an `ABBREVIATIONS` token, which
/// never ends a sentence regardless of what follows. Each returned slice keeps its own trailing
/// punctuation.
pub(crate) fn split_sentences(text: &str) -> Vec<&str> {
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
            let at_boundary = j >= bytes.len() || bytes[j] == b' ' || bytes[j] == b'\t';
            let after_abbreviation =
                bytes[i] == b'.' && j == i + 1 && is_abbreviation(preceding_token(text, i));
            if at_boundary && !after_abbreviation {
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

pub(crate) fn word_count(s: &str) -> usize {
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
        // A lone initial (abbreviation splitting) or a travessão dialogue turn neither counts
        // toward the run nor resets it: it's not a sentence in its own right, so it can't be
        // dramatic fragmentation and can't break up a genuine run either.
        if is_lone_initial(s) || is_dialogue_turn(s) {
            continue;
        }
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

/// Deterministic replacement for `tally.iter().find(|&(_, &n)| n >= min_count)`: a `HashMap`'s
/// iteration order is unspecified, so scanning it directly for "any entry past the threshold"
/// made the reported phrase change between runs on identical input, which breaks `--baseline`
/// fingerprints. Instead, among the entries meeting `min_count`, picks the one whose FIRST
/// occurrence (`tally`'s `usize` value: a byte offset or, for `robotic_rhythm`, a sentence index
/// -- any monotonically increasing position marker works) comes earliest, matching this crate's
/// own "anchored at the first occurrence" framing for these density diagnostics. Shared by
/// fragmentation.rs (SLOP030), hedging.rs (SLOP015), filler.rs (SLOP027), weak_verb.rs (SLOP028),
/// promo.rs (SLOP031), and hyphen.rs (SLOP032), which all pick a repeated phrase to name in their
/// message out of a `HashMap<String, usize>` tally the same way.
pub(crate) fn earliest_qualifying(
    tally: &HashMap<String, (usize, usize)>,
    min_count: usize,
) -> Option<&str> {
    tally
        .iter()
        .filter(|&(_, &(count, _))| count >= min_count)
        .min_by_key(|&(_, &(_, first))| first)
        .map(|(phrase, _)| phrase.as_str())
}

/// (b) robotic rhythm: 3+ sentences (anywhere in the paragraph, not necessarily consecutive)
/// sharing the same opening word, OR 4+ sentences whose word counts are all within
/// `RHYTHM_SPREAD` of each other. Evaluated over the WHOLE paragraph's sentence set (not a
/// subset search): a single long outlier sentence is enough to keep an otherwise-uniform
/// paragraph silent, which is the conservative choice that also matches ordinary prose (a real
/// paragraph is rarely uniform end to end).
fn robotic_rhythm(sentences: &[&str]) -> Option<String> {
    let mut openers: HashMap<String, (usize, usize)> = HashMap::new();
    for (idx, s) in sentences.iter().enumerate() {
        if let Some(w) = s.split_whitespace().next() {
            let key = w
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            // A determiner or preposition opener is unremarkable repetition, not anaphora (see
            // OPENER_FUNCTION_WORDS's doc comment); pronouns are deliberately not excluded.
            if !key.is_empty() && !OPENER_FUNCTION_WORDS.contains(&key.as_str()) {
                openers.entry(key).or_insert((0, idx)).0 += 1;
            }
        }
    }
    if let Some(word) = earliest_qualifying(&openers, 3) {
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
    use crate::lang::Lang;

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
    fn html_paragraphs_are_one_element_each() {
        assert!(diagnostics_for_html("<p>Fast.</p>\n<p>Simple.</p>\n<p>Free.</p>\n").is_empty());
        let diags = diagnostics_for_html("<h2>Why</h2>\n<p>Fast. Simple. Free.</p>\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 2);
    }

    #[test]
    fn html_code_only_paragraph_anchors_on_the_code_span() {
        let src = "<p><code>a b c</code></p>\n";
        let blocks = paragraph_blocks(&ProseDoc::parse_html(src));
        assert_eq!(blocks[0].first_byte, src.find("<code>").unwrap());
    }

    #[test]
    fn html_inline_code_counts_as_words_in_its_sentence() {
        let blocks = paragraph_blocks(&ProseDoc::parse_html(
            "<p>Run <code>stopslop --fix .</code> now. Then <b>read</b> the output.</p>\n",
        ));
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].text,
            "Run code code code now. Then read the output."
        );
    }

    #[test]
    fn flags_three_consecutive_short_sentences() {
        let src = "It works. It scales. It ships. The rest of the rollout needs no further changes from anyone on the team this quarter.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP030");
    }

    /// `doc.masked` blanks inline code to spaces, so both sub-checks here used to read the blanks
    /// as absence. Found on this repo's own README.
    #[test]
    fn inline_code_is_not_read_as_absent() {
        // Opener check: masking left "and" as the first visible token of both sentences.
        let openers = "`select` and `ignore` follow Ruff's composition rules here. `extend-select` and `extend-ignore` never replace anything at all.\n";
        assert!(diagnostics_for(openers).is_empty());

        // Word-count check: three ordinary sentences counted as "very short" because the code
        // spans contributed zero words.
        let counts = "Run `stopslop --format json .` now. Then read `target/release/output.json` carefully. Finally pipe `jq '.runs[0].results'` through less.\n";
        assert!(diagnostics_for(counts).is_empty());
    }

    /// The placeholder stands in per token, not per span: a reader counts `--format json` as two
    /// words, so collapsing it to one would resurrect the very-short-sentence false positive.
    #[test]
    fn code_span_contributes_its_own_token_count() {
        let one_word_each = "Run `a` now. Then `b` too. Also `c` here. The rollout needs no further changes from anyone on the team this quarter.\n";
        let diags = diagnostics_for(one_word_each);
        assert_eq!(diags.len(), 1, "genuinely short sentences must still fire");
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
        // A pronoun, not a determiner: OPENER_FUNCTION_WORDS deliberately excludes pronouns, so
        // this is exactly the anaphora the signal exists to catch.
        let src = "It starts quickly. It scales well. It costs little to run every month for most teams.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP030");
    }

    #[test]
    fn clean_varied_opening_words() {
        let src = "The service starts quickly. Most teams find it easy to run. Costs stay low for typical workloads across the board.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// Three sentences opening on the same determiner ("The") is unremarkable repetition, not
    /// anaphora -- OPENER_FUNCTION_WORDS excludes it. Contrast with the pronoun case in
    /// `flags_same_opening_word_three_times`, which still fires.
    #[test]
    fn clean_repeated_determiner_opener_is_not_robotic_rhythm() {
        let src = "The service starts quickly. The service scales well. The service costs little to run every month for most teams.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// Portuguese twin of the determiner-exclusion test above: `o`/`a`/`em`/`no`/`os`/`na` are
    /// the top hitters from the pt-BR corpus measurement (see OPENER_FUNCTION_WORDS's doc
    /// comment) and must not fire the opener-repeat signal.
    #[test]
    fn clean_repeated_pt_br_preposition_opener_is_not_robotic_rhythm() {
        let src = "Em Roma, o clima era ameno. Em Atenas, o debate seguia forte. Em Cartago, o comércio prosperava havia décadas inteiras.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn lone_initials_do_not_count_toward_or_break_a_short_sentence_run() {
        // "A." and "M." are abbreviation splits (see `is_lone_initial`'s doc comment), not real
        // sentences: they must neither count toward the very-short-sentence run nor reset it, so
        // the three genuine short sentences around them still trigger the run.
        let src = "It works. A. M. It scales. It ships. The rest of the rollout needs no further changes from anyone on the team this quarter.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP030");
    }

    #[test]
    fn dialogue_turns_do_not_count_toward_or_break_a_short_sentence_run() {
        // Travessão dialogue lines are conversation, not dramatic fragmentation, and must not
        // interrupt a genuine short-sentence run either.
        let src = "It works. — Sim. It scales. It ships. The rest of the rollout needs no further changes from anyone on the team this quarter.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP030");
    }

    #[test]
    fn a_lone_initial_alone_does_not_manufacture_a_run() {
        // Two short sentences plus two lone initials: still only two REAL short sentences, so
        // this must not manufacture a fake run of three.
        let src = "It works. A. M. It ships. The rest of the rollout needs no further changes from anyone on the team this quarter.\n";
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

    #[test]
    fn split_sentences_keeps_abbreviation_periods_inside_the_sentence() {
        let text = "We compared Acme Inc. vs. Beta Corp. and found the results close.";
        assert_eq!(split_sentences(text), vec![text]);
    }

    #[test]
    fn split_sentences_splits_after_a_sentence_ending_in_no() {
        let text = "The answer was no. We moved on.";
        assert_eq!(
            split_sentences(text),
            vec!["The answer was no.", "We moved on."]
        );
    }

    #[test]
    fn split_sentences_still_splits_after_a_sentence_that_contains_an_abbreviation() {
        let text =
            "We compared Acme Inc. and Beta Corp. before choosing a vendor. It works. It scales. It ships.";
        assert_eq!(
            split_sentences(text),
            vec![
                "We compared Acme Inc. and Beta Corp. before choosing a vendor.",
                "It works.",
                "It scales.",
                "It ships.",
            ]
        );
    }

    /// "Firstly" opens the paragraph first (sentence index 0) and repeats 3x; "Meanwhile" also
    /// repeats 3x but first opens a sentence later (index 1). Before the fix,
    /// `openers.iter().find(|&(_, &n)| n >= 3)` scanned a `HashMap` in unspecified order, so this
    /// exact input could name either opener depending on the map's hash seed for that call.
    #[test]
    fn robotic_rhythm_names_the_earliest_qualifying_opener() {
        let sentences = [
            "Firstly the team reviewed the design document together.",
            "Meanwhile the client tested the staging build overnight.",
            "Firstly the team scheduled a follow-up review for Monday.",
            "Meanwhile the client reported no new issues since then.",
            "Firstly the team merged the change after final approval.",
            "Meanwhile the client confirmed the rollout plan for release.",
        ];
        let msg = robotic_rhythm(&sentences).unwrap();
        assert!(msg.contains("\"firstly\""), "message was: {msg}");
    }

    /// Same input as above, called 50 times: pre-fix, a fresh `HashMap` (a new, independently
    /// randomized hash state per call) made the reported opener flip between runs on identical
    /// input, which breaks `--baseline` fingerprints. This test must fail against the unfixed
    /// `openers.iter().find(...)` selection (confirmed by running it against that code before
    /// applying `earliest_qualifying`) and pass once the selection is deterministic.
    #[test]
    fn robotic_rhythm_opener_selection_is_deterministic_across_runs() {
        let sentences = [
            "Firstly the team reviewed the design document together.",
            "Meanwhile the client tested the staging build overnight.",
            "Firstly the team scheduled a follow-up review for Monday.",
            "Meanwhile the client reported no new issues since then.",
            "Firstly the team merged the change after final approval.",
            "Meanwhile the client confirmed the rollout plan for release.",
        ];
        let first = robotic_rhythm(&sentences);
        for _ in 0..50 {
            assert_eq!(robotic_rhythm(&sentences), first);
        }
    }
}
