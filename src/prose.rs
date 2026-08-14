//! Prose-lang (.md/.mdx/.txt/.rst) support: parses source into a byte-offset-preserving
//! "masked prose stream" (fenced code + inline code blanked to spaces) plus lightweight
//! structural metadata (headings, list blocks, URL spans, frontmatter). SLOP011-021 are thin
//! readers on top of `ProseDoc` — this is the one real per-file cost, computed once in
//! `engine::lint_prose`.

use crate::context::TextNode;
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::LazyLock;

pub struct Heading {
    pub level: usize,      // 1..=6 (count of leading '#')
    pub line: usize,       // 1-based
    pub col: usize,        // 1-based, col of first '#'
    pub text: String,      // heading text: leading '#'s + surrounding ws + trailing '#'s stripped
    pub byte_start: usize, // byte offset of the heading line's start
    pub byte_end: usize,   // byte offset of the heading line's end (exclusive of '\n')
}

pub struct ListItem {
    pub line: usize,        // 1-based line of the marker
    pub marker_byte: usize, // byte offset of the '-'/'*'/'+'/digit marker
    pub ordered: bool,      // true for `1.` / `1)`, false for -/*/+
}

pub struct ListBlock {
    pub items: Vec<ListItem>, // contiguous run (see masking rules for "contiguous")
}

// ponytail: the spec's ProseDoc has no lifetime parameter, but `ignore_comments: Vec<TextNode>`
// borrows `&str` slices of the original source (TextNode's `text` field is `&'a str`), and Rust
// struct definitions cannot elide a borrowed lifetime the way fn signatures can. `ProseDoc<'a>`
// tied to the same `'a` as `LintContext<'a>` is the smallest change that actually compiles.
pub struct ProseDoc<'a> {
    /// Same byte length as `source`. Fenced code blocks (``` / ~~~) and inline `code` spans are
    /// blanked to ASCII spaces; every '\n' is preserved, so byte offsets AND line/col map 1:1
    /// back onto the original source. This is the MASKED PROSE STREAM every rule scans.
    pub masked: String,
    /// Byte range [start, end) of leading YAML frontmatter incl. `---` fences, or None. NOT
    /// blanked in `masked` (rules opt in/out via `in_frontmatter`).
    pub frontmatter: Option<(usize, usize)>,
    pub headings: Vec<Heading>,
    pub list_blocks: Vec<ListBlock>,
    /// Byte ranges of URLs / inline-link targets / autolinks / reference defs. NOT blanked in
    /// `masked` (rules opt in/out via `in_url`).
    pub url_spans: Vec<(usize, usize)>,
    /// Word count of `masked` EXCLUDING the frontmatter span. Denominator for every density
    /// rule. (Code is already blanked -> contributes 0 words.)
    pub words: usize,
    /// `<!-- ai-slop-ignore -->` / `<!-- ai-slop-ignore-file -->` HTML comments, as TextNodes,
    /// for `suppress::apply`. (All HTML comments whose text contains "ai-slop-ignore".)
    pub ignore_comments: Vec<TextNode<'a>>,
    line_starts: Vec<usize>, // byte offset of each line start; for line_col
}

impl<'a> ProseDoc<'a> {
    pub fn parse(source: &'a str) -> ProseDoc<'a> {
        let line_starts = compute_line_starts(source);
        let line_spans = compute_line_spans(source, &line_starts);

        let frontmatter = detect_frontmatter(source, &line_spans);

        let mut masked_bytes = source.as_bytes().to_vec();
        let fence_ranges = blank_fences(source, &line_spans, &mut masked_bytes);

        let is_fenced_line: Vec<bool> = line_spans
            .iter()
            .map(|&(ls, _)| fence_ranges.iter().any(|&(s, e)| ls >= s && ls < e))
            .collect();
        let is_fm_line: Vec<bool> = line_spans
            .iter()
            .map(|&(ls, _)| frontmatter.is_some_and(|(s, e)| ls >= s && ls < e))
            .collect();

        let is_indented_code_line = blank_indented_code(
            source,
            &line_spans,
            &is_fenced_line,
            &is_fm_line,
            &mut masked_bytes,
        );
        let is_code_line: Vec<bool> = is_fenced_line
            .iter()
            .zip(&is_indented_code_line)
            .map(|(&f, &i)| f || i)
            .collect();

        blank_inline_code(&line_spans, &mut masked_bytes);
        let masked =
            String::from_utf8(masked_bytes).expect("masking only overwrites char-boundary spans");

        let headings = scan_headings(&masked, &line_spans, &is_code_line, &is_fm_line);
        let list_blocks = scan_list_blocks(&masked, &line_spans, &is_code_line, &is_fm_line);
        let url_spans = scan_url_spans(&masked);

        let fm_end = frontmatter.map(|(_, e)| e).unwrap_or(0);
        let words = masked[fm_end..].split_whitespace().count();

        let ignore_comments = scan_ignore_comments(source, &masked, &line_starts);

        ProseDoc {
            masked,
            frontmatter,
            headings,
            list_blocks,
            url_spans,
            words,
            ignore_comments,
            line_starts,
        }
    }

    /// 1-based (line, col) for a byte offset into source/masked. Binary-search `line_starts`;
    /// col = 1 + chars from line start to byte (count chars, not bytes).
    pub fn line_col(&self, byte: usize) -> (usize, usize) {
        compute_line_col(&self.line_starts, &self.masked, byte)
    }

    /// Byte range of `byte`'s own line, end exclusive of the trailing '\n'. Binary search rather
    /// than the obvious `rfind('\n')`/`find('\n')` pair: rules call this once per regex match, and
    /// on a single-line document each scan runs the length of the file -- 900 KB of one-line prose
    /// took 24s in SLOP028 before this.
    pub fn line_span(&self, byte: usize) -> (usize, usize) {
        let idx = match self.line_starts.binary_search(&byte) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        };
        let end = self
            .line_starts
            .get(idx + 1)
            .map(|&next| next - 1)
            .unwrap_or(self.masked.len());
        (self.line_starts[idx], end)
    }

    /// True if `byte` is inside the frontmatter span.
    pub fn in_frontmatter(&self, byte: usize) -> bool {
        self.frontmatter.is_some_and(|(s, e)| byte >= s && byte < e)
    }

    /// True if `byte` falls on any heading line (byte in [h.byte_start, h.byte_end]).
    pub fn in_heading(&self, byte: usize) -> bool {
        self.headings
            .iter()
            .any(|h| byte >= h.byte_start && byte <= h.byte_end)
    }

    /// True if `byte` falls inside any URL / link-target span.
    pub fn in_url(&self, byte: usize) -> bool {
        self.url_spans.iter().any(|&(s, e)| byte >= s && byte < e)
    }
}

/// Dedupes an iterator of byte offsets down to the first (leftmost) byte per line — the "one
/// diagnostic per matching line" pattern shared by SLOP011-014's rules. Callers filter/map their
/// regex matches down to in-scope byte offsets first, then feed them here.
pub fn first_byte_per_line(
    doc: &ProseDoc,
    bytes: impl Iterator<Item = usize>,
) -> BTreeMap<usize, usize> {
    let mut by_line: BTreeMap<usize, usize> = BTreeMap::new();
    for byte in bytes {
        let line = doc.line_col(byte).0;
        let entry = by_line.entry(line).or_insert(byte);
        if byte < *entry {
            *entry = byte;
        }
    }
    by_line
}

fn compute_line_col(line_starts: &[usize], text: &str, byte: usize) -> (usize, usize) {
    let idx = match line_starts.binary_search(&byte) {
        Ok(i) => i,
        Err(0) => 0,
        Err(i) => i - 1,
    };
    let line_start = line_starts[idx];
    let col = 1 + text[line_start..byte.min(text.len())].chars().count();
    (idx + 1, col)
}

fn compute_line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

/// (start, end) per line, end exclusive of the line's own trailing '\n' (or source.len() for the
/// last line).
fn compute_line_spans(source: &str, line_starts: &[usize]) -> Vec<(usize, usize)> {
    line_starts
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let e = line_starts
                .get(i + 1)
                .map(|&next| next - 1)
                .unwrap_or(source.len());
            (s, e)
        })
        .collect()
}

/// Line 1 exactly "---" (after trimming trailing ws) opens frontmatter; the next line that is
/// exactly "---" or "..." closes it. Only recognized at byte 0.
fn detect_frontmatter(source: &str, line_spans: &[(usize, usize)]) -> Option<(usize, usize)> {
    let &(s0, e0) = line_spans.first()?;
    if source[s0..e0].trim_end() != "---" {
        return None;
    }
    for &(s, e) in &line_spans[1..] {
        let line = source[s..e].trim_end();
        if line == "---" || line == "..." {
            return Some((0, e));
        }
    }
    None
}

/// A line matching `^\s*(`{3,}|~{3,})` opens a fence: returns (marker char, run length).
fn fence_marker(line: &str) -> Option<(u8, usize)> {
    let trimmed = line.trim_start();
    let ch = *trimmed.as_bytes().first()?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let run = trimmed.bytes().take_while(|&b| b == ch).count();
    (run >= 3).then_some((ch, run))
}

/// A line `^\s*<ch>{>=min_len}\s*$` closes a fence opened with `ch`/`min_len`.
fn is_fence_close(line: &str, ch: u8, min_len: usize) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let run = trimmed.bytes().take_while(|&b| b == ch).count();
    run == trimmed.len() && run >= min_len
}

/// Finds fenced code block byte ranges (aligned to line boundaries: opening fence line's start
/// through closing fence line's end, or EOF if unterminated) and blanks them in `masked_bytes`
/// (preserving '\n'). Returns the ranges so callers can classify lines as "inside a fence".
fn blank_fences(
    source: &str,
    line_spans: &[(usize, usize)],
    masked_bytes: &mut [u8],
) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut i = 0;
    while i < line_spans.len() {
        let (ls, le) = line_spans[i];
        if let Some((ch, len)) = fence_marker(&source[ls..le]) {
            let mut j = i + 1;
            let mut close_end = None;
            while j < line_spans.len() {
                let (js, je) = line_spans[j];
                if is_fence_close(&source[js..je], ch, len) {
                    close_end = Some(je);
                    break;
                }
                j += 1;
            }
            let end = close_end.unwrap_or(source.len());
            ranges.push((ls, end));
            i = if close_end.is_some() {
                j + 1
            } else {
                line_spans.len()
            };
        } else {
            i += 1;
        }
    }
    for &(s, e) in &ranges {
        for b in &mut masked_bytes[s..e] {
            if *b != b'\n' {
                *b = b' ';
            }
        }
    }
    ranges
}

// The double-backtick alternative's content is `[^\n]+?` (non-greedy, not `[^`\n]*`): the
// standard CommonMark literal-backtick idiom (`` `` `code with a ` backtick` `` ``) wraps
// content that itself contains single backticks, so excluding backticks from the content class
// makes the double-backtick alternative never match that idiom at all — it'd fall through to
// the single-backtick alternative, which then grabs a bogus 1-backtick-wide submatch and leaves
// the real code (and any embedded backticks) unblanked. Non-greedy `[^\n]+?` just scans forward
// for the nearest real `` `` `` closer, backticks-inside included, same trick fence-matching
// already relies on elsewhere in this file.
/// True if `line` opens with >=4 spaces or a leading tab (CommonMark's indented-code-block
/// threshold), and isn't itself all whitespace.
fn is_indented_code_candidate(line: &str) -> bool {
    if line.starts_with('\t') {
        return true;
    }
    let leading_spaces = line.bytes().take_while(|&b| b == b' ').count();
    leading_spaces >= 4 && leading_spaces < line.len()
}

/// Finds CommonMark-style indented code blocks (a run of lines each indented >=4 spaces/a tab,
/// not interrupting a paragraph -- the line before the run must be blank, a fence/frontmatter
/// boundary, or the start of the document; blank lines inside the run don't end it as long as
/// another indented line follows) and blanks them in `masked_bytes` (preserving '\n'), mirroring
/// `blank_fences`. Returns a per-line bool so callers can fold it into "is this a code line"
/// alongside `is_fenced_line`.
/// ponytail: doesn't special-case list-item continuation indent (nested list content indented
/// 4+ columns can get swept in too) -- narrow enough not to matter until a real fixture proves
/// otherwise.
fn blank_indented_code(
    source: &str,
    line_spans: &[(usize, usize)],
    is_fenced_line: &[bool],
    is_fm_line: &[bool],
    masked_bytes: &mut [u8],
) -> Vec<bool> {
    let mut is_indented = vec![false; line_spans.len()];
    let mut prev_blank_or_boundary = true; // start of document counts as a boundary
    let mut i = 0;
    while i < line_spans.len() {
        if is_fenced_line[i] || is_fm_line[i] {
            prev_blank_or_boundary = true;
            i += 1;
            continue;
        }
        let (ls, le) = line_spans[i];
        let line = &source[ls..le];
        if line.trim().is_empty() {
            prev_blank_or_boundary = true;
            i += 1;
            continue;
        }
        if prev_blank_or_boundary && is_indented_code_candidate(line) {
            let start = i;
            let mut last_code_line = i;
            let mut j = i;
            while j < line_spans.len() && !is_fenced_line[j] && !is_fm_line[j] {
                let (js, je) = line_spans[j];
                let jline = &source[js..je];
                if jline.trim().is_empty() {
                    j += 1;
                    continue;
                }
                if !is_indented_code_candidate(jline) {
                    break;
                }
                last_code_line = j;
                j += 1;
            }
            for line in is_indented.iter_mut().take(last_code_line + 1).skip(start) {
                *line = true;
            }
            i = last_code_line + 1;
            prev_blank_or_boundary = false;
            continue;
        }
        prev_blank_or_boundary = false;
        i += 1;
    }

    for (idx, &(ls, le)) in line_spans.iter().enumerate() {
        if is_indented[idx] {
            for b in &mut masked_bytes[ls..le] {
                if *b != b'\n' {
                    *b = b' ';
                }
            }
        }
    }
    is_indented
}

static INLINE_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"``([^\n]+?)``|`([^`\n]+)`").unwrap());

/// Blanks line-local backtick spans (`` `code` `` / ``` ``code`` ```) in the already
/// fence-blanked `masked_bytes`. Multi-line inline spans are not supported.
/// ponytail: inline code assumed single-line; multi-line inline spans are rare — upgrade to a
/// CommonMark inline scan only if a fixture demands it.
fn blank_inline_code(line_spans: &[(usize, usize)], masked_bytes: &mut [u8]) {
    for &(ls, le) in line_spans {
        let ranges: Vec<(usize, usize)> = {
            let line_str = std::str::from_utf8(&masked_bytes[ls..le]).unwrap();
            INLINE_CODE_RE
                .find_iter(line_str)
                .map(|m| (ls + m.start(), ls + m.end()))
                .collect()
        };
        for (s, e) in ranges {
            for b in &mut masked_bytes[s..e] {
                if *b != b'\n' {
                    *b = b' ';
                }
            }
        }
    }
}

/// ATX heading: `^\s{0,3}#{1,6}\s+.*`. `text` = the line with leading #'s/ws and trailing #'s/ws
/// stripped. ponytail: Setext (=== / --- underlines) skipped; add if a fixture needs it.
fn heading_match(line: &str) -> Option<(usize, String)> {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    let mut ws = 0usize;
    while ws < 3 && i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
        ws += 1;
    }
    let mut level = 0usize;
    while level < 6 && i < bytes.len() && bytes[i] == b'#' {
        i += 1;
        level += 1;
    }
    if level == 0 || i >= bytes.len() || !(bytes[i] == b' ' || bytes[i] == b'\t') {
        return None;
    }
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    let rest = line[i..].trim_end();
    let trailing_stripped = rest.trim_end_matches('#').trim_end();
    Some((level, trailing_stripped.to_string()))
}

/// List-item marker: `^\s{0,3}([-*+]|\d{1,9}[.)])\s+\S`. Returns (local marker byte, ordered).
fn list_item_marker(line: &str) -> Option<(usize, bool)> {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    let mut ws = 0usize;
    while ws < 3 && i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
        ws += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    if matches!(bytes[i], b'-' | b'*' | b'+') {
        let marker = i;
        return has_marker_tail(bytes, i + 1).then_some((marker, false));
    }
    let digit_start = i;
    let mut j = i;
    while j < bytes.len() && bytes[j].is_ascii_digit() && j - digit_start < 9 {
        j += 1;
    }
    if j > digit_start && j < bytes.len() && matches!(bytes[j], b'.' | b')') {
        return has_marker_tail(bytes, j + 1).then_some((digit_start, true));
    }
    None
}

/// After the marker char: requires `\s+\S` (at least one ws char, then a non-ws char).
fn has_marker_tail(bytes: &[u8], after: usize) -> bool {
    if after >= bytes.len() || !(bytes[after] == b' ' || bytes[after] == b'\t') {
        return false;
    }
    let mut k = after;
    while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
        k += 1;
    }
    k < bytes.len() && bytes[k] != b' ' && bytes[k] != b'\t'
}

fn scan_headings(
    masked: &str,
    line_spans: &[(usize, usize)],
    is_fenced_line: &[bool],
    is_fm_line: &[bool],
) -> Vec<Heading> {
    let mut out = Vec::new();
    for (i, &(ls, le)) in line_spans.iter().enumerate() {
        if is_fenced_line[i] || is_fm_line[i] {
            continue;
        }
        let line = &masked[ls..le];
        if let Some((level, text)) = heading_match(line) {
            let col = 1 + line[..line.len() - line.trim_start().len()].chars().count();
            out.push(Heading {
                level,
                line: i + 1,
                col,
                text,
                byte_start: ls,
                byte_end: le,
            });
        }
    }
    out
}

#[derive(PartialEq)]
enum LineKind {
    Boundary, // inside a fence or frontmatter: hard-breaks any open list run
    Blank,
    Item(usize, bool), // (local marker byte, ordered)
    Other,
}

fn classify_line(line: &str, fenced: bool, frontmatter: bool) -> LineKind {
    if fenced || frontmatter {
        return LineKind::Boundary;
    }
    if line.trim().is_empty() {
        return LineKind::Blank;
    }
    match list_item_marker(line) {
        Some((marker, ordered)) => LineKind::Item(marker, ordered),
        None => LineKind::Other,
    }
}

fn scan_list_blocks(
    masked: &str,
    line_spans: &[(usize, usize)],
    is_fenced_line: &[bool],
    is_fm_line: &[bool],
) -> Vec<ListBlock> {
    let mut blocks = Vec::new();
    let mut current: Vec<ListItem> = Vec::new();
    let mut streak = 0usize; // consecutive non-blank, non-list "Other" lines

    for (i, &(ls, le)) in line_spans.iter().enumerate() {
        let line = &masked[ls..le];
        match classify_line(line, is_fenced_line[i], is_fm_line[i]) {
            LineKind::Boundary => {
                if !current.is_empty() {
                    blocks.push(ListBlock {
                        items: std::mem::take(&mut current),
                    });
                }
                streak = 0;
            }
            LineKind::Blank => streak = 0,
            LineKind::Item(marker, ordered) => {
                current.push(ListItem {
                    line: i + 1,
                    marker_byte: ls + marker,
                    ordered,
                });
                streak = 0;
            }
            LineKind::Other => {
                streak += 1;
                if streak >= 2 && !current.is_empty() {
                    blocks.push(ListBlock {
                        items: std::mem::take(&mut current),
                    });
                }
            }
        }
    }
    if !current.is_empty() {
        blocks.push(ListBlock { items: current });
    }
    blocks
}

static AUTOLINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<(https?://[^>\s]+)>").unwrap());
static LINK_TARGET_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\]\(([^)\s]+)").unwrap());
static REF_DEF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*\[[^\]]+\]:\s*(\S+)").unwrap());
static BARE_URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"https?://[^\s)>\]]+").unwrap());

/// Byte spans of autolinks, inline-link targets, reference-definition targets, and bare URLs.
/// Overlap is fine; `in_url` just checks membership.
fn scan_url_spans(masked: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    for re in [&AUTOLINK_RE, &LINK_TARGET_RE, &REF_DEF_RE] {
        for caps in re.captures_iter(masked) {
            let m = caps.get(1).unwrap();
            spans.push((m.start(), m.end()));
        }
    }
    for m in BARE_URL_RE.find_iter(masked) {
        spans.push((m.start(), m.end()));
    }
    spans
}

static IGNORE_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<!--([\s\S]*?)-->").unwrap());

/// All HTML comments in `source` whose text contains "ai-slop-ignore", excluding ones that live
/// inside a fenced/inline code span (documenting the suppression syntax with a literal
/// `` `<!-- ai-slop-ignore -->` `` code example must not self-suppress the whole file). A byte
/// range that code-masking touched reads back differently in `masked` than in `source`
/// (blanked to spaces); unchanged means it's real prose, not a code example.
fn scan_ignore_comments<'a>(
    source: &'a str,
    masked: &str,
    line_starts: &[usize],
) -> Vec<TextNode<'a>> {
    let mut out = Vec::new();
    for caps in IGNORE_COMMENT_RE.captures_iter(source) {
        let whole = caps.get(0).unwrap();
        if caps[1].contains("ai-slop-ignore") && masked[whole.range()] == source[whole.range()] {
            let (line, col) = compute_line_col(line_starts, source, whole.start());
            out.push(TextNode {
                text: whole.as_str(),
                start_byte: whole.start(),
                end_byte: whole.end(),
                line,
                col,
                is_doc: false,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fenced_block_fully_blanked_and_length_preserved() {
        let src = "before\n```\ncode here\nmore code\n```\nafter\n";
        let doc = ProseDoc::parse(src);
        assert_eq!(doc.masked.len(), src.len());
        // Every line strictly inside the fence (incl. delimiters) is blank apart from '\n'.
        for line in doc.masked.lines().skip(1).take(4) {
            assert!(line.chars().all(|c| c == ' '), "line not blanked: {line:?}");
        }
        assert!(doc.masked.starts_with("before"));
        assert!(doc.masked.trim_end().ends_with("after"));
    }

    #[test]
    fn unterminated_fence_blanks_through_eof() {
        let src = "text\n```\nunterminated\nstill inside\n";
        let doc = ProseDoc::parse(src);
        assert_eq!(doc.masked.len(), src.len());
        for line in doc.masked.lines().skip(1) {
            assert!(line.chars().all(|c| c == ' '), "line not blanked: {line:?}");
        }
    }

    #[test]
    fn line_col_maps_known_byte() {
        let src = "one\ntwo\nthree\n";
        let doc = ProseDoc::parse(src);
        // byte offset of 't' in "two" (line 2, col 1)
        let byte = src.find("two").unwrap();
        assert_eq!(doc.line_col(byte), (2, 1));
        // byte offset of 'r' in "three" (line 3, col 3)
        let byte = src.find("ree").unwrap();
        assert_eq!(doc.line_col(byte), (3, 3));
    }

    #[test]
    fn inline_code_span_is_blanked() {
        let src = "text with `some code` inline\n";
        let doc = ProseDoc::parse(src);
        assert_eq!(doc.masked.len(), src.len());
        assert!(!doc.masked.contains('`'));
        assert!(!doc.masked.contains("some code"));
        assert!(doc.masked.contains("text with"));
        assert!(doc.masked.contains("inline"));
    }

    #[test]
    fn frontmatter_detected_and_excluded_from_words() {
        let src = "---\ntitle: Test\ndate: 2025-01-01\n---\nActual body text here.\n";
        let doc = ProseDoc::parse(src);
        let (s, e) = doc.frontmatter.expect("frontmatter should be detected");
        assert_eq!(s, 0);
        assert_eq!(&src[..e], "---\ntitle: Test\ndate: 2025-01-01\n---");
        assert!(doc.in_frontmatter(5)); // inside "title: Test"
        assert!(!doc.in_frontmatter(e + 1)); // past the closing fence
                                             // "Actual body text here." = 4 words; frontmatter words must not be counted.
        assert_eq!(doc.words, 4);
    }

    #[test]
    fn no_frontmatter_when_doc_does_not_open_with_dashes() {
        let doc = ProseDoc::parse("# Heading\n\nBody text.\n");
        assert!(doc.frontmatter.is_none());
    }

    #[test]
    fn unterminated_frontmatter_falls_through_as_ordinary_prose() {
        // Unlike an unterminated fence (blanked through EOF), an opened-but-never-closed
        // frontmatter block isn't recognized at all: the literal "---" line and the
        // YAML-looking lines fall through untouched, scanned as ordinary prose.
        let src = "---\ntitle: Test\ndate: 2025-01-01\nBody text goes here without a close.\n";
        let doc = ProseDoc::parse(src);
        assert!(doc.frontmatter.is_none());
        assert!(!doc.in_frontmatter(5));
        assert_eq!(doc.masked, src);
    }

    #[test]
    fn indented_code_block_is_blanked_and_prose_around_it_survives() {
        let src = "Here is how to use it:\n\n    function example() {\n      return leverage(robust);\n    }\n\nThat's it.\n";
        let doc = ProseDoc::parse(src);
        assert_eq!(doc.masked.len(), src.len());
        assert!(!doc.masked.contains("leverage"));
        assert!(!doc.masked.contains("function example"));
        assert!(doc.masked.contains("Here is how to use it"));
        assert!(doc.masked.contains("That's it"));
    }

    #[test]
    fn indented_continuation_of_a_paragraph_is_not_treated_as_code() {
        // Indented code cannot interrupt a paragraph: a lazily-indented continuation line right
        // after non-blank prose (no blank line separating them) stays live prose, not code.
        let src = "This paragraph continues\n    onto an indented line with no blank before it.\n";
        let doc = ProseDoc::parse(src);
        assert_eq!(doc.masked, src);
    }

    #[test]
    fn ignore_comment_in_inline_code_is_not_a_real_suppression() {
        // Documenting the suppression syntax with a literal code example must not
        // self-suppress the whole file (a real regression: this exact pattern appears in
        // stopslop's own README).
        let doc = ProseDoc::parse(
            "Use `<!-- ai-slop-ignore-file -->` to suppress everything.\n\nReal body text.\n",
        );
        assert!(doc.ignore_comments.is_empty());
    }

    #[test]
    fn real_ignore_comment_outside_code_is_still_recognized() {
        let doc = ProseDoc::parse("Body text. <!-- ai-slop-ignore -->\nMore text.\n");
        assert_eq!(doc.ignore_comments.len(), 1);
    }
}
