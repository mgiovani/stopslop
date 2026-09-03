//! Prose-lang (.md/.mdx/.txt/.rst/.html) support: parses source into a byte-offset-preserving
//! "masked prose stream" (fenced code + inline code blanked to spaces) plus lightweight
//! structural metadata (headings, list blocks, URL spans, frontmatter). SLOP011-021 are thin
//! readers on top of `ProseDoc` — this is the one real per-file cost, computed once in
//! `engine::lint_prose`. HTML takes the inverse route in `parse_html`: everything is blanked and
//! only visible text, comments, and link targets are restored from a tree-sitter-html parse.

use crate::context::TextNode;
use crate::lang::{Lang, NatLang};
use regex::Regex;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::LazyLock;
use tree_sitter::Node;

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

// ponytail: the spec's ProseDoc has no lifetime, but `ignore_comments: Vec<TextNode>` borrows
// `&str` slices of the source, and structs can't elide a borrowed lifetime like fn signatures
// can. `ProseDoc<'a>` tied to `LintContext<'a>` is the smallest change that compiles.
/// One inline `code` span: its byte range in the source, and how many whitespace-separated
/// tokens it held before being blanked.
#[derive(Debug, Clone, Copy)]
pub struct CodeSpan {
    pub start: usize,
    pub end: usize,
    pub words: usize,
}

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
    /// Inline `code` spans, which ARE blanked in `masked`. Kept so rules that key on position or
    /// length can substitute a placeholder instead of reading the blanks as absence (see
    /// `blank_inline_code`).
    pub code_spans: Vec<CodeSpan>,
    /// Word count of `masked` EXCLUDING the frontmatter span. Denominator for every density
    /// rule. (Code is already blanked -> contributes 0 words.)
    pub words: usize,
    /// `<!-- ai-slop-ignore -->` / `<!-- ai-slop-ignore-file -->` HTML comments, as TextNodes,
    /// for `suppress::apply`. (All HTML comments whose text contains "ai-slop-ignore".)
    pub ignore_comments: Vec<TextNode<'a>>,
    /// (start, end) byte span per line of `masked`, end exclusive of the line's own trailing
    /// '\n'. Computed once here so rules that need to walk the document line by line (e.g.
    /// SLOP029/030/033) don't each rebuild their own copy with a private newline scan.
    pub line_spans: Vec<(usize, usize)>,
    /// Start byte of every block-level HTML element, ascending. A hard sentence boundary for
    /// SLOP033 and the anchor for `block_initial`: a `<select>` of 60 `<option>`s carries no
    /// terminal punctuation and would otherwise read as one 60-word sentence. Empty for the
    /// Markdown family, whose paragraphs are blank-line delimited.
    pub block_starts: Vec<usize>,
    /// Every HTML attribute that carries a value, as one `name="value"` TextNode each, so the
    /// string-scanning rules see them the way they see a code lang's string literals through
    /// `ctx.strings` (SLOP009 reads `alt="image"` and a placeholder-image host here). Empty for the
    /// Markdown family.
    pub attr_values: Vec<TextNode<'a>>,
    /// HTML paragraphs as element byte ranges: every block element that holds no other block
    /// and is not a list item, table cell, form control, or heading, so `<p>Fast.</p>
    /// <p>Simple.</p>` is two paragraphs and a nav of `<li>`s is none. `None` for the Markdown
    /// family, whose paragraphs `fragmentation::paragraph_blocks` derives from blank lines.
    pub paragraphs: Option<Vec<(usize, usize)>>,
    /// Decoded HTML entities the punctuation rules care about, as (byte of the `&`, char):
    /// `&mdash;` and `&#8212;` are dashes to SLOP018 and `&ldquo;` a smart quote to SLOP020,
    /// but decoding them into `masked` would change byte offsets. Empty for Markdown.
    pub entities: Vec<(usize, char)>,
    /// Start byte of every `<strong>`/`<b>` element, for SLOP019; Markdown bold is a `**` pair
    /// the rule finds in the masked stream itself, and HTML tags are blank there.
    pub bold_spans: Vec<usize>,
    /// `<footer>`, `<aside>`, and `<nav>` element ranges. Their paragraphs are prose to the
    /// density rules but never the document's ending, which SLOP029 reads. Empty for Markdown.
    pub footers: Vec<(usize, usize)>,
    /// The document's declared natural language, read from a `lang` attribute on the root
    /// `<html>` start tag only -- a `lang` on an inner element marks a quotation or an embedded
    /// snippet in a different language, not the document itself, and this crate has no per-span
    /// natlang concept to express that. `None` for a missing/absent `lang`, an unrecognized tag
    /// (`NatLang::from_tag`), or the whole Markdown family (no root tag to read): a file is not
    /// config, so a declaration this crate doesn't understand is silently ignored rather than
    /// treated as an error. `engine::lint_prose` reads this to narrow which language panels run
    /// on this one file (see its doc comment for the narrowing rule).
    pub html_lang: Option<NatLang>,
    line_starts: Vec<usize>, // byte offset of each line start; for line_col
    /// (byte, col) of the last `line_col` answer. Rules ask in byte order, so the next answer on
    /// the same line counts chars from here rather than from the line start -- a 1.8 MB
    /// single-line file with 200k em dashes took 18s before this.
    col_memo: Cell<(usize, usize)>,
    /// `line_col` counts columns here, not in `masked`: a blanked multibyte char is several
    /// spaces in `masked` but one column on the page.
    source: &'a str,
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
            .map(|&(ls, _)| span_contains(&fence_ranges, ls))
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

        let code_spans = blank_inline_code(&line_spans, &mut masked_bytes);
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
            code_spans,
            words,
            ignore_comments,
            line_spans,
            block_starts: Vec::new(),
            attr_values: Vec::new(),
            paragraphs: None,
            entities: Vec::new(),
            bold_spans: Vec::new(),
            footers: Vec::new(),
            html_lang: None,
            line_starts,
            col_memo: Cell::new((0, 1)),
            source,
        }
    }

    /// HTML sibling of `parse`. The masked stream starts as every byte blanked to a space except
    /// '\n'; a tree-sitter-html parse then restores the bytes of `text` nodes, comments, and
    /// `href`/`src` attribute values, so the prose rules see what a reader sees. Tags, entities,
    /// `<script>`/`<style>` bodies, and the subtrees of `SKIP_TAGS` never reach the stream.
    /// `<h1>`..`<h6>` map onto `headings` by the element's line span so `in_heading` keeps
    /// working; `frontmatter`, `list_blocks`, and `code_spans` stay empty (issue #29 names the
    /// upgrade path for each). Named entities are left blank rather than decoded: decoding would
    /// change byte offsets, and a blank `&mdash;` costs a miss, never a false positive.
    pub fn parse_html(source: &'a str) -> ProseDoc<'a> {
        let line_starts = compute_line_starts(source);
        let line_spans = compute_line_spans(source, &line_starts);

        let prepared = blank_template_syntax(source);
        let mut masked_bytes: Vec<u8> = source
            .bytes()
            .map(|b| if b == b'\n' { b } else { b' ' })
            .collect();
        let mut scan = HtmlScan::default();
        let mut parser = tree_sitter::Parser::new();
        let tree = match parser.set_language(&crate::lang::ts_language(Lang::Html)) {
            Ok(()) => parser.parse(&prepared, None),
            Err(_) => None,
        };
        if let Some(tree) = &tree {
            restore_html(tree.root_node(), &prepared, &mut masked_bytes, &mut scan);
        }
        let masked = String::from_utf8(masked_bytes)
            .expect("restored spans are whole nodes, hence char-aligned");

        let headings = html_headings(&scan.headings, &masked, source, &line_starts, &line_spans);
        let mut url_spans = scan.url_spans;
        url_spans.extend(scan_url_spans(&masked));
        url_spans.sort_unstable_by_key(|&(s, _)| s);
        let url_spans = merge_overlapping(url_spans);
        let words = masked.split_whitespace().count();
        let ignore_comments = scan_ignore_comments(source, &masked, &line_starts);
        let attr_values = scan
            .attrs
            .iter()
            .map(|&(s, e)| {
                let (line, col) = compute_line_col(&line_starts, source, s);
                TextNode {
                    text: &source[s..e],
                    start_byte: s,
                    end_byte: e,
                    line,
                    col,
                    is_doc: false,
                }
            })
            .collect();

        // A leaf block is one paragraph. A block holding another block keeps the text before
        // that child, because tree-sitter-html leaves an unclosed `<p>` open across the list
        // that follows it.
        let paragraphs = scan
            .blocks
            .iter()
            .enumerate()
            .filter(|&(_, &(_, _, is_para))| is_para)
            .filter_map(|(i, &(start, end, _))| match scan.blocks.get(i + 1) {
                Some(&(child, _, _)) if child < end => {
                    (!masked[start..child].trim().is_empty()).then_some((start, child))
                }
                _ => Some((start, end)),
            })
            .collect();

        ProseDoc {
            masked,
            frontmatter: None,
            headings,
            list_blocks: Vec::new(),
            url_spans,
            code_spans: scan.code_spans,
            words,
            ignore_comments,
            line_spans,
            block_starts: scan.blocks.iter().map(|b| b.0).collect(),
            attr_values,
            paragraphs: Some(paragraphs),
            entities: scan.entities,
            bold_spans: scan.bold_spans,
            footers: scan.footers,
            html_lang: tree
                .as_ref()
                .and_then(|t| detect_html_lang(t.root_node(), &prepared)),
            line_starts,
            col_memo: Cell::new((0, 1)),
            source,
        }
    }

    /// True when only whitespace separates `byte` from the start of its enclosing HTML block
    /// element: the byte opens what a reader sees as a paragraph, list item, cell, or heading.
    /// The Markdown family has no `block_starts` and always answers false; its block structure
    /// lives in blank lines and markers, which the rules read directly.
    pub fn block_initial(&self, byte: usize) -> bool {
        let idx = self.block_starts.partition_point(|&b| b <= byte);
        idx > 0
            && self.masked[self.block_starts[idx - 1]..byte]
                .trim()
                .is_empty()
    }

    /// 1-based (line, col) for a byte offset into source/masked. Binary-search `line_starts`;
    /// col = 1 + chars from line start to byte (count chars, not bytes).
    pub fn line_col(&self, byte: usize) -> (usize, usize) {
        let idx = line_index(&self.line_starts, byte);
        let line_start = self.line_starts[idx];
        let byte = byte.min(self.masked.len());
        let (from, base) = match self.col_memo.get() {
            (b, c) if b >= line_start && b <= byte => (b, c),
            _ => (line_start, 1),
        };
        // Rules hand over match starts, which sit on char boundaries; `get` keeps a stray
        // mid-char byte from panicking the parallel walk and counts the masked spaces instead.
        let col = base
            + self.source[..byte.max(from)].get(from..byte).map_or_else(
                || self.masked[from..byte].chars().count(),
                |s| s.chars().count(),
            );
        self.col_memo.set((byte, col));
        (idx + 1, col)
    }

    /// Byte range of `byte`'s own line, end exclusive of the trailing '\n'. Binary search rather
    /// than the obvious `rfind('\n')`/`find('\n')` pair: rules call this once per regex match, and
    /// on a single-line document each scan runs the length of the file -- 900 KB of one-line prose
    /// took 24s in SLOP028 before this.
    pub fn line_span(&self, byte: usize) -> (usize, usize) {
        let idx = line_index(&self.line_starts, byte);
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
    /// `headings` is built in line order, one per line, so it is sorted and disjoint; SLOP033
    /// asks once per line, and a linear scan here made it O(lines x headings) (issue #21).
    pub fn in_heading(&self, byte: usize) -> bool {
        let idx = self.headings.partition_point(|h| h.byte_start <= byte);
        idx > 0 && byte <= self.headings[idx - 1].byte_end
    }

    /// True if `byte` falls inside any URL / link-target span.
    pub fn in_footer(&self, byte: usize) -> bool {
        self.footers.iter().any(|&(s, e)| s <= byte && byte < e)
    }

    pub fn in_url(&self, byte: usize) -> bool {
        self.url_span_at(byte).is_some()
    }

    /// The URL/link-target span containing `byte`, if any. Binary search: `url_spans` is sorted
    /// by start and merged disjoint at construction time (`scan_url_spans`), so at most one span
    /// can contain a given byte -- same `partition_point` idiom as `in_heading`.
    pub fn url_span_at(&self, byte: usize) -> Option<(usize, usize)> {
        let idx = self.url_spans.partition_point(|&(s, _)| s <= byte);
        (idx > 0)
            .then(|| self.url_spans[idx - 1])
            .filter(|&(_, e)| byte < e)
    }
}

/// True if `byte` falls in `[s, e)` for some span in `spans`, which must be sorted by start and
/// non-overlapping (so at most one candidate exists: the last span whose start is <= byte).
fn span_contains(spans: &[(usize, usize)], byte: usize) -> bool {
    let idx = spans.partition_point(|&(s, _)| s <= byte);
    idx > 0 && byte < spans[idx - 1].1
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

fn line_index(line_starts: &[usize], byte: usize) -> usize {
    match line_starts.binary_search(&byte) {
        Ok(i) => i,
        Err(0) => 0,
        Err(i) => i - 1,
    }
}

fn compute_line_col(line_starts: &[usize], text: &str, byte: usize) -> (usize, usize) {
    let idx = line_index(line_starts, byte);
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

// Content is `[^\n]+?` (non-greedy), not `[^`\n]*`: CommonMark's literal-backtick idiom wraps
// content with single backticks, so excluding backticks makes double-backtick never match — it
// falls through to single-backtick with a bogus submatch, leaving real code unblanked.
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
/// Returns the blanked spans, so rules that care about POSITION or LENGTH can tell "there was
/// code here" from "there was nothing here". Blanking to spaces alone loses that distinction and
/// has produced false positives three separate times: an arrow after a blanked span looked like a
/// bullet's marker (SLOP021), and a sentence opening with code read as opening with its next word
/// while counting zero words for the code (SLOP030, both sub-checks).
fn blank_inline_code(line_spans: &[(usize, usize)], masked_bytes: &mut [u8]) -> Vec<CodeSpan> {
    let mut spans = Vec::new();
    for &(ls, le) in line_spans {
        // Read BEFORE blanking: this is the only point where the span's real token count is
        // still available, and a reader counts `--format json` as two words, not one.
        let ranges: Vec<CodeSpan> = {
            let line_str = std::str::from_utf8(&masked_bytes[ls..le]).unwrap();
            INLINE_CODE_RE
                .find_iter(line_str)
                .map(|m| CodeSpan {
                    start: ls + m.start(),
                    end: ls + m.end(),
                    words: m
                        .as_str()
                        .trim_matches('`')
                        .split_whitespace()
                        .count()
                        .max(1),
                })
                .collect()
        };
        for span in ranges {
            for b in &mut masked_bytes[span.start..span.end] {
                if *b != b'\n' {
                    *b = b' ';
                }
            }
            spans.push(span);
        }
    }
    spans
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

/// Byte spans of autolinks, inline-link targets, reference-definition targets, and bare URLs,
/// sorted and merged so `url_span_at` can binary-search: the four regexes overlap on a link
/// target that is itself a bare URL.
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
    spans.sort_unstable_by_key(|&(s, _)| s);
    merge_overlapping(spans)
}

/// Merges a start-sorted list of possibly-overlapping spans into a disjoint, start-sorted list.
fn merge_overlapping(spans: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::with_capacity(spans.len());
    for (s, e) in spans {
        match out.last_mut() {
            Some((_, last_e)) if s <= *last_e => *last_e = (*last_e).max(e),
            _ => out.push((s, e)),
        }
    }
    out
}

/// Elements whose subtree never reaches the prose stream: code and preformatted text (the
/// fenced-block analogue), form input echoes, inert templates, and vector or math markup.
const SKIP_TAGS: &[&str] = &[
    "pre", "code", "textarea", "template", "svg", "math", "noscript", "script", "style",
];

/// Phrasing-content tags. Every other element starts a block (see `ProseDoc::block_starts`).
/// The allowlist runs this way round so an unknown or custom element becomes a boundary (at
/// worst a missed long sentence) rather than glue (a false one).
const INLINE_TAGS: &[&str] = &[
    "a", "abbr", "b", "bdi", "bdo", "br", "cite", "code", "data", "del", "dfn", "em", "i", "img",
    "ins", "kbd", "mark", "picture", "q", "rp", "rt", "ruby", "s", "samp", "small", "source",
    "span", "strong", "sub", "sup", "time", "u", "var", "wbr",
];

static TEMPLATE_RE: LazyLock<regex::bytes::Regex> = LazyLock::new(|| {
    regex::bytes::Regex::new(r"\{\{[\s\S]*?\}\}|\{%[\s\S]*?%\}|\{#[\s\S]*?#\}").unwrap()
});

/// Blanks Django/Jinja `{{ … }}`, `{% … %}`, and `{# … #}` to spaces, '\n' preserved, BEFORE the
/// parse: `{% if a < b %}` otherwise makes the scanner read `<b` as a start tag and swallow
/// everything up to the next `>`. Non-greedy, so an unbalanced opener matches nothing and the
/// rest of the document stays visible. Templates ship as `.html` in every Python web repo.
fn blank_template_syntax(source: &str) -> String {
    let mut bytes = source.as_bytes().to_vec();
    for m in TEMPLATE_RE.find_iter(source.as_bytes()) {
        for b in &mut bytes[m.range()] {
            if *b != b'\n' {
                *b = b' ';
            }
        }
    }
    String::from_utf8(bytes).expect("matches start and end on ASCII delimiters")
}

/// Block elements that are never a paragraph: list and table structure, form controls, and
/// headings, mirroring what `fragmentation::paragraph_blocks` drops in Markdown.
const NON_PARAGRAPH_TAGS: &[&str] = &[
    "li", "ul", "ol", "dl", "dt", "menu", "table", "thead", "tbody", "tfoot", "tr", "td", "th",
    "caption", "option", "optgroup", "select", "button", "label", "legend", "nav", "title", "h1",
    "h2", "h3", "h4", "h5", "h6",
];

/// Page furniture around the content. Its text is linted, but the last paragraph inside it is
/// never the piece's ending.
const FOOTER_TAGS: &[&str] = &["footer", "aside", "nav"];

#[derive(Default)]
struct HtmlScan {
    headings: Vec<(usize, usize, usize)>, // (level, element start, element end)
    url_spans: Vec<(usize, usize)>,
    blocks: Vec<(usize, usize, bool)>, // (start, end, may be a paragraph), pre-order
    attrs: Vec<(usize, usize)>,        // whole `name="value"` spans
    code_spans: Vec<CodeSpan>,
    entities: Vec<(usize, char)>,
    bold_spans: Vec<usize>,
    footers: Vec<(usize, usize)>,
}

/// The document's declared language from its root `<html lang="...">` start tag, read off the
/// already-parsed tree rather than a second text scan over the source: a text scan for
/// `<html ...lang=...>` matches whichever occurrence of that shape comes first in the file,
/// comment included -- `<!-- fallback: <html lang="en"> -->` above the real
/// `<html lang="pt-BR">` used to win, since the scan had no way to know it was reading inside a
/// comment. Reading the tree's actual root sidesteps that: a `comment` node is never an
/// `element`, so it can never stand in for the page's own root tag. `None` for a missing
/// root/tag/attribute or a tag `NatLang::from_tag` doesn't recognize.
fn detect_html_lang(root: Node, src: &str) -> Option<NatLang> {
    let mut cursor = root.walk();
    let html = root
        .children(&mut cursor)
        .find(|c| c.kind() == "element" && tag_name(*c, src).eq_ignore_ascii_case("html"))?;
    let mut cursor = html.walk();
    let start_tag = html
        .children(&mut cursor)
        .find(|c| matches!(c.kind(), "start_tag" | "self_closing_tag"))?;
    let mut cursor = start_tag.walk();
    let (_, (s, e)) = start_tag
        .named_children(&mut cursor)
        .filter(|c| c.kind() == "attribute")
        .find_map(|attr| {
            attribute_parts(attr, src).filter(|(name, _)| name.eq_ignore_ascii_case("lang"))
        })?;
    NatLang::from_tag(&src[s..e])
}

/// The chars the punctuation rules read: the em dash and curly quotes, named or numeric.
/// Everything else stays blank; `&amp;` and `&nbsp;` carry no tell, and `&ndash;` is left out
/// because SLOP018 exempts an en dash between digits by looking at its neighbors in the masked
/// stream, which an entity does not have.
fn decode_entity(entity: &str) -> Option<char> {
    let body = entity.strip_prefix('&')?.trim_end_matches(';');
    if let Some(num) = body.strip_prefix('#') {
        let (digits, radix) = match num.strip_prefix(['x', 'X']) {
            Some(hex) => (hex, 16),
            None => (num, 10),
        };
        return u32::from_str_radix(digits, radix)
            .ok()
            .and_then(char::from_u32)
            .filter(|c| matches!(c, '\u{2014}' | '\u{2018}'..='\u{201D}'));
    }
    Some(match body {
        "mdash" => '\u{2014}',
        "lsquo" => '\u{2018}',
        "rsquo" => '\u{2019}',
        "ldquo" => '\u{201C}',
        "rdquo" => '\u{201D}',
        _ => return None,
    })
}

/// Word count of an inline `<code>` element's content, so `with_code_placeholders` can stand a
/// word in for each token the reader sees, the way Markdown's inline code spans do.
fn code_words(element: Node, src: &str) -> usize {
    let mut cursor = element.walk();
    let kids: Vec<Node> = element.children(&mut cursor).collect();
    match (kids.first(), kids.last()) {
        (Some(open), Some(close)) if kids.len() >= 2 && close.kind() == "end_tag" => src
            [open.end_byte()..close.start_byte()]
            .split_whitespace()
            .count(),
        _ => 0,
    }
}

/// One pre-order pass restoring the visible bytes of `prepared` into `masked` and collecting
/// heading, link-target, and block spans. Keys on node kind only: `comment` is a grammar extra
/// and may sit inside a start tag. A `text` node under an ERROR parent is error recovery
/// re-lexing markup (an unterminated `<!--`, a stray quote) and stays blank.
fn restore_html(root: Node, prepared: &str, masked: &mut [u8], scan: &mut HtmlScan) {
    let src = prepared.as_bytes();
    let mut cursor = root.walk();
    crate::context::walk_tree(&mut cursor, &mut |node| match node.kind() {
        "element" | "script_element" | "style_element" => {
            let tag = tag_name(node, prepared);
            if let Some(level) = heading_level(tag) {
                scan.headings
                    .push((level, node.start_byte(), node.end_byte()));
            }
            if !has_tag(INLINE_TAGS, tag) {
                scan.blocks.push((
                    node.start_byte(),
                    node.end_byte(),
                    !has_tag(NON_PARAGRAPH_TAGS, tag),
                ));
            }
            if tag.eq_ignore_ascii_case("strong") || tag.eq_ignore_ascii_case("b") {
                scan.bold_spans.push(node.start_byte());
            }
            if has_tag(FOOTER_TAGS, tag) {
                scan.footers.push((node.start_byte(), node.end_byte()));
            }
            if tag.eq_ignore_ascii_case("code") {
                scan.code_spans.push(CodeSpan {
                    start: node.start_byte(),
                    end: node.end_byte(),
                    words: code_words(node, prepared),
                });
            }
            !has_tag(SKIP_TAGS, tag)
        }
        "entity" => {
            if let Some(ch) = decode_entity(&prepared[node.byte_range()]) {
                scan.entities.push((node.start_byte(), ch));
            }
            true
        }
        "text" => {
            if !node.parent().is_some_and(|p| p.is_error()) {
                let r = node.byte_range();
                masked[r.clone()].copy_from_slice(&src[r]);
            }
            true
        }
        "comment" => {
            let r = node.byte_range();
            masked[r.clone()].copy_from_slice(&src[r]);
            true
        }
        "attribute" => {
            if let Some((name, (s, e))) = attribute_parts(node, prepared) {
                if name.eq_ignore_ascii_case("href") || name.eq_ignore_ascii_case("src") {
                    masked[s..e].copy_from_slice(&src[s..e]);
                    scan.url_spans.push((s, e));
                }
                scan.attrs.push((node.start_byte(), node.end_byte()));
            }
            true
        }
        _ => true,
    });
}

fn tag_name<'s>(element: Node, src: &'s str) -> &'s str {
    let mut cursor = element.walk();
    let tag = element
        .children(&mut cursor)
        .find(|c| matches!(c.kind(), "start_tag" | "self_closing_tag"));
    tag.and_then(|tag| tag.named_child(0))
        .filter(|n| n.kind() == "tag_name")
        .map(|n| &src[n.byte_range()])
        .unwrap_or_default()
}

fn has_tag(set: &[&str], tag: &str) -> bool {
    set.iter().any(|t| t.eq_ignore_ascii_case(tag))
}

fn heading_level(tag: &str) -> Option<usize> {
    match tag.as_bytes() {
        [b'h' | b'H', d @ b'1'..=b'6'] => Some(usize::from(d - b'0')),
        _ => None,
    }
}

/// The attribute's name and the byte range of its value, or None for a valueless attribute. The
/// value is either bare or the aliased `attribute_value` inside `quoted_attribute_value`;
/// `href=""` has neither.
fn attribute_parts<'s>(attr: Node, src: &'s str) -> Option<(&'s str, (usize, usize))> {
    let mut cursor = attr.walk();
    let mut name = "";
    let mut value = None;
    for child in attr.named_children(&mut cursor) {
        match child.kind() {
            "attribute_name" => name = &src[child.byte_range()],
            "attribute_value" => value = Some(child.byte_range()),
            "quoted_attribute_value" => {
                value = child
                    .named_child(0)
                    .filter(|v| v.kind() == "attribute_value")
                    .map(|v| v.byte_range());
            }
            _ => {}
        }
    }
    value.map(|r| (name, (r.start, r.end)))
}

/// `Heading`s from `<h1>`..`<h6>` spans, keyed to the element's whole line span so the
/// Markdown-shaped `in_heading` still answers per line. `text` is the element's visible text with
/// tags collapsed, so an entity or a restored `href` inside the heading alters it (`Challenges
/// &amp; Opportunities` reads `Challenges Opportunities`): an accepted miss for the exact-match
/// title lists in SLOP035/036. A heading whose line overlaps the previous one is dropped to keep
/// the sorted-disjoint invariant `in_heading` binary-searches on.
fn html_headings(
    raw: &[(usize, usize, usize)],
    masked: &str,
    source: &str,
    line_starts: &[usize],
    line_spans: &[(usize, usize)],
) -> Vec<Heading> {
    let mut out: Vec<Heading> = Vec::new();
    for &(level, start, end) in raw {
        let byte_start = line_spans[line_index(line_starts, start)].0;
        let last = end.saturating_sub(1).max(start);
        let byte_end = line_spans[line_index(line_starts, last)].1;
        if out.last().is_some_and(|h| byte_start <= h.byte_end) {
            continue;
        }
        let (line, col) = compute_line_col(line_starts, source, start);
        let text = masked[start..end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        out.push(Heading {
            level,
            line,
            col,
            text,
            byte_start,
            byte_end,
        });
    }
    out
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

    fn tokens<'d>(doc: &'d ProseDoc) -> Vec<&'d str> {
        doc.masked.split_whitespace().collect()
    }

    #[test]
    fn html_masks_tags_and_restores_text() {
        let src = "<!doctype html>\n<p class=\"lead\">Hello <em>world</em>.</p>\n";
        let doc = ProseDoc::parse_html(src);
        assert_eq!(doc.masked.len(), src.len());
        assert_eq!(tokens(&doc), ["Hello", "world", "."]);
        for (i, b) in src.bytes().enumerate() {
            assert_eq!(
                b == b'\n',
                doc.masked.as_bytes()[i] == b'\n',
                "newline moved at {i}"
            );
        }
        assert_eq!(doc.words, 3);
        assert!(doc.frontmatter.is_none() && doc.list_blocks.is_empty());
    }

    #[test]
    fn html_skips_code_form_and_vector_subtrees() {
        let src = "<script>var alertText = 1;</script><style>.a{color:red}</style>\
                   <pre>preformatted</pre><p>keep <code>inline</code> this</p>\
                   <textarea>typed</textarea><svg><text>vector</text></svg><noscript>fallback</noscript>";
        let doc = ProseDoc::parse_html(src);
        assert_eq!(tokens(&doc), ["keep", "this"]);
    }

    #[test]
    fn html_attribute_gt_and_src_value() {
        let src = "<img alt=\"a > b\" src=\"hero.png\">\n<p>after</p>\n";
        let doc = ProseDoc::parse_html(src);
        assert_eq!(tokens(&doc), ["hero.png", "after"]);
        let at = src.find("hero.png").unwrap();
        assert!(doc.in_url(at) && doc.in_url(at + 7) && !doc.in_url(at + 8));
    }

    #[test]
    fn html_unterminated_comment_leaks_no_markup() {
        let src = "<p>before</p>\n<!-- never closed\n<p>after</p>\n<div class=\"x\">tail</div>\n";
        let doc = ProseDoc::parse_html(src);
        let toks = tokens(&doc);
        assert!(toks.contains(&"before"), "{toks:?}");
        assert!(
            !toks.iter().any(|t| t.contains('<') || t.contains("class")),
            "{toks:?}"
        );
    }

    #[test]
    fn html_comment_restored_and_ignore_directive_found_outside_script() {
        let src =
            "<p>x</p> <!-- ai-slop-ignore -->\n<script>// <!-- ai-slop-ignore-file --></script>\n";
        let doc = ProseDoc::parse_html(src);
        assert!(doc.masked.contains("<!-- ai-slop-ignore -->"));
        assert_eq!(doc.ignore_comments.len(), 1);
        assert_eq!(doc.ignore_comments[0].text, "<!-- ai-slop-ignore -->");
        assert_eq!(doc.ignore_comments[0].line, 1);
    }

    #[test]
    fn html_entities_stay_blank() {
        let doc = ProseDoc::parse_html("<p>Tom &amp; Jerry &mdash; friends &#x2014; end</p>\n");
        assert_eq!(tokens(&doc), ["Tom", "Jerry", "friends", "end"]);
    }

    #[test]
    fn html_template_syntax_blanked_before_the_parse() {
        let src = "<p>{{ user.café }} has {% if a < b %}few{% else %}many{% endif %} items {# nöte #}</p>\n\
                   <p>next</p>\n";
        let doc = ProseDoc::parse_html(src);
        assert_eq!(doc.masked.len(), src.len());
        assert_eq!(tokens(&doc), ["has", "few", "many", "items", "next"]);
    }

    #[test]
    fn html_unbalanced_template_opener_fails_closed() {
        let doc = ProseDoc::parse_html("<p>{% broken</p>\n<p>visible text</p>\n");
        assert!(tokens(&doc).contains(&"visible"));
    }

    #[test]
    fn html_headings_map_to_line_spans() {
        let src = "<h1>Title</h1>\n<h2 id=\"a\">Sub <em>heading</em></h2>\n<p>body</p>\n";
        let doc = ProseDoc::parse_html(src);
        assert_eq!(doc.headings.len(), 2);
        let (h1, h2) = (&doc.headings[0], &doc.headings[1]);
        assert_eq!(
            (h1.level, h1.line, h1.col, h1.text.as_str()),
            (1, 1, 1, "Title")
        );
        assert_eq!((h2.level, h2.line, h2.text.as_str()), (2, 2, "Sub heading"));
        let line2 = src.find("<h2").unwrap();
        let line3 = src.find("<p>").unwrap();
        assert!(doc.in_heading(line2) && doc.in_heading(line2 + 5));
        assert!(!doc.in_heading(line3));
    }

    #[test]
    fn html_only_h1_to_h6_are_headings() {
        let doc = ProseDoc::parse_html(
            "<html><body><hr><h7>x</h7><header>y</header><H2>Z</H2></body></html>\n",
        );
        assert_eq!(doc.headings.len(), 1);
        assert_eq!(
            (doc.headings[0].level, doc.headings[0].text.as_str()),
            (2, "Z")
        );
    }

    #[test]
    fn html_empty_or_valueless_href_has_no_url_span() {
        let doc = ProseDoc::parse_html("<a href=\"\">x</a> <a href>y</a> <a href=z>w</a>\n");
        assert_eq!(doc.url_spans.len(), 1);
        assert!(doc.masked.contains('z'));
    }

    #[test]
    fn html_attribute_values_become_strings() {
        let src = "<p>\n<img src=\"a.png\" alt='cover' hidden data-x=bare>\n</p>\n";
        let doc = ProseDoc::parse_html(src);
        let texts: Vec<&str> = doc.attr_values.iter().map(|a| a.text).collect();
        assert_eq!(texts, ["src=\"a.png\"", "alt='cover'", "data-x=bare"]);
        assert_eq!((doc.attr_values[1].line, doc.attr_values[1].col), (2, 18));
        assert!(doc.attr_values.iter().all(|a| !a.is_doc));
        assert!(ProseDoc::parse("[x](a.png)\n").attr_values.is_empty());
    }

    #[test]
    fn html_same_line_headings_keep_disjoint_spans() {
        let doc = ProseDoc::parse_html("<h1>One</h1><h2>Two</h2>\n<h3>Three</h3>\n");
        assert_eq!(doc.headings.len(), 2);
        assert!(doc
            .headings
            .windows(2)
            .all(|w| w[0].byte_end < w[1].byte_start));
    }

    #[test]
    fn html_block_starts_and_block_initial() {
        let src = "<div>\n<p>— Author</p>\n<p>text — more</p>\n<ul><li>one</li></ul>\n<a href=\"#\">link</a> <em>x</em>\n";
        let doc = ProseDoc::parse_html(src);
        let starts: Vec<usize> = ["<div", "<p>—", "<p>text", "<ul", "<li"]
            .iter()
            .map(|t| src.find(t).unwrap())
            .collect();
        assert_eq!(doc.block_starts, starts);
        assert!(doc.block_initial(src.find("— Author").unwrap()));
        assert!(!doc.block_initial(src.find("— more").unwrap()));
        assert!(!doc.block_initial(src.find("link").unwrap()));
    }

    #[test]
    fn html_href_is_a_url_span() {
        let src = "<a href=\"https://acme.test/?utm_source=chatgpt.com\">x</a>\n";
        let doc = ProseDoc::parse_html(src);
        let at = src.find("utm_source").unwrap();
        assert!(doc.masked.contains("utm_source=chatgpt.com"));
        assert!(doc.in_url(at));
        assert_eq!(doc.url_spans.len(), 1);
    }

    #[test]
    fn columns_count_source_chars_across_blanked_multibyte_spans() {
        let md = "`café` x\n";
        assert_eq!(ProseDoc::parse(md).line_col(md.find('x').unwrap()), (1, 8));
        let html = "<p title=\"café\">x</p>\n";
        assert_eq!(
            ProseDoc::parse_html(html).line_col(html.find('x').unwrap()),
            (1, 17)
        );
    }

    #[test]
    fn html_unclosed_p_keeps_the_text_before_the_list_it_swallows() {
        let src =
            "<p>Fast. Simple. Free.\n<ul><li>one</li></ul>\n<p>Tail.</p>\n<div><h2>T</h2></div>\n";
        let doc = ProseDoc::parse_html(src);
        let paragraphs: Vec<&str> = doc
            .paragraphs
            .as_ref()
            .unwrap()
            .iter()
            .map(|&(s, e)| &src[s..e])
            .collect();
        assert_eq!(paragraphs, ["<p>Fast. Simple. Free.\n", "<p>Tail.</p>"]);
    }

    #[test]
    fn html_footer_aside_and_nav_ranges_are_recorded() {
        let src = "<main><p>Body.</p></main>\n<footer><p>Rights.</p></footer>\n<aside><p>Note.</p></aside>\n";
        let doc = ProseDoc::parse_html(src);
        assert!(!doc.in_footer(src.find("Body").unwrap()));
        assert!(doc.in_footer(src.find("Rights").unwrap()));
        assert!(doc.in_footer(src.find("Note").unwrap()));
        assert_eq!(doc.paragraphs.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn html_paragraphs_are_leaf_blocks_without_list_table_or_ui_tags() {
        let src =
            "<div class=\"hero\">Lead text</div>\n<div><h2>T</h2><p>One <em>two</em>.</p></div>\n\
                   <ul><li>item</li></ul>\n<table><tr><td>cell</td></tr></table>\n\
                   <select><option>a</option></select>\n<blockquote><p>q</p></blockquote>\n";
        let doc = ProseDoc::parse_html(src);
        let paragraphs: Vec<&str> = doc
            .paragraphs
            .as_ref()
            .unwrap()
            .iter()
            .map(|&(s, e)| &src[s..e])
            .collect();
        assert_eq!(
            paragraphs,
            [
                "<div class=\"hero\">Lead text</div>",
                "<p>One <em>two</em>.</p>",
                "<p>q</p>"
            ]
        );
        assert!(ProseDoc::parse("a\n\nb\n").paragraphs.is_none());
    }

    #[test]
    fn html_inline_code_becomes_a_code_span_and_pre_does_not() {
        let src = "<p>Run <code>stopslop --fix .</code> now.</p>\n<pre><code>x y</code></pre>\n";
        let doc = ProseDoc::parse_html(src);
        assert_eq!(doc.code_spans.len(), 1);
        let span = doc.code_spans[0];
        assert_eq!(&src[span.start..span.end], "<code>stopslop --fix .</code>");
        assert_eq!(span.words, 3);
        assert!(!doc.masked.contains("stopslop"));
    }

    #[test]
    fn html_dash_and_quote_entities_are_decoded_into_the_side_table() {
        let src = "<p>a &mdash; b &ndash; c &#8212; d &#x201C;e&#x201D; &ldquo;f&rdquo; &amp; &nbsp; &#65;</p>\n";
        let doc = ProseDoc::parse_html(src);
        let chars: String = doc.entities.iter().map(|&(_, c)| c).collect();
        assert_eq!(chars, "\u{2014}\u{2014}\u{201C}\u{201D}\u{201C}\u{201D}");
        assert_eq!(doc.entities[0].0, src.find("&mdash;").unwrap());
        assert!(doc.entities.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn markdown_has_no_block_starts() {
        let doc = ProseDoc::parse("para\n\n- item\n");
        assert!(doc.block_starts.is_empty());
        assert!(!doc.block_initial(0));
    }

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
    fn line_col_memo_is_exact_for_any_query_order_and_multibyte_text() {
        for src in ["héllo — wörld\nsecond — line\n\nx\n", "no — newline", ""] {
            let doc = ProseDoc::parse(src);
            let expect = |byte: usize| {
                let byte = byte.min(src.len());
                let line_start = src[..byte].rfind('\n').map_or(0, |i| i + 1);
                let line = 1 + src[..byte].matches('\n').count();
                (line, 1 + src[line_start..byte].chars().count())
            };
            let bytes: Vec<usize> = src.char_indices().map(|(b, _)| b).collect();
            let forward_then_back = bytes.iter().chain(bytes.iter().rev());
            let zigzag = bytes
                .iter()
                .zip(bytes.iter().rev())
                .flat_map(|(a, b)| [a, b]);
            for &byte in forward_then_back.chain(zigzag) {
                assert_eq!(doc.line_col(byte), expect(byte), "{src:?} byte {byte}");
            }
            assert_eq!(doc.line_col(src.len()), expect(src.len()), "{src:?} eof");
            assert_eq!(
                doc.line_col(src.len() + 7),
                expect(src.len()),
                "{src:?} past eof"
            );
        }
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

    #[test]
    fn merged_spans_are_disjoint_and_span_contains_uses_half_open_ranges() {
        let merged = merge_overlapping(vec![(0, 5), (0, 3), (5, 10), (12, 20), (14, 16), (30, 31)]);
        assert_eq!(merged, vec![(0, 10), (12, 20), (30, 31)]);
        assert!(merge_overlapping(Vec::new()).is_empty());
        for byte in 0..40 {
            let brute = [(0, 10), (12, 20), (30, 31)]
                .iter()
                .any(|&(s, e)| byte >= s && byte < e);
            assert_eq!(span_contains(&merged, byte), brute, "byte {byte}");
        }
        assert!(!span_contains(&[], 0));
    }

    #[test]
    fn in_heading_in_url_url_span_at_and_line_span_match_brute_force_scans() {
        // The link target and the bare-URL regex both match "https://a.test/docs", so the two
        // spans must merge; ".test" is the RFC 2606 testing TLD, which SLOP009 leaves alone.
        let src = "# Heading One\n\n\
Intro prose before any code, see [docs](https://a.test/docs) for details.\n\n\
```rust\nfn code() {}\n```\n\n\
## Heading Two\n\n\
More prose after the fence with a bare https://a.test/bare url and more text.\n\n\
### Last heading without a newline";
        let doc = ProseDoc::parse(src);

        fn brute_in_heading(doc: &ProseDoc, byte: usize) -> bool {
            doc.headings
                .iter()
                .any(|h| byte >= h.byte_start && byte <= h.byte_end)
        }
        fn brute_url_span_at(doc: &ProseDoc, byte: usize) -> Option<(usize, usize)> {
            doc.url_spans
                .iter()
                .find(|&&(s, e)| byte >= s && byte < e)
                .copied()
        }
        fn brute_line_span(src: &str, byte: usize) -> (usize, usize) {
            let byte = byte.min(src.len());
            let start = src[..byte].rfind('\n').map_or(0, |i| i + 1);
            let end = src[byte..].find('\n').map_or(src.len(), |i| byte + i);
            (start, end)
        }

        assert!(!doc.headings.is_empty(), "fixture should contain headings");
        assert!(
            !doc.url_spans.is_empty(),
            "fixture should contain URL spans"
        );
        // The link-target match and the bare-URL match on the same text must merge to one span,
        // not sit in the list twice -- otherwise `partition_point`'s single-candidate assumption
        // (at most one span contains a given byte) breaks.
        assert_eq!(
            doc.url_spans.len(),
            2,
            "duplicate/overlapping matches on the same URL should merge to one disjoint span"
        );

        for byte in 0..=src.len() {
            assert_eq!(
                doc.in_heading(byte),
                brute_in_heading(&doc, byte),
                "in_heading mismatch at byte {byte}"
            );
            assert_eq!(
                doc.url_span_at(byte),
                brute_url_span_at(&doc, byte),
                "url_span_at mismatch at byte {byte}"
            );
            assert_eq!(doc.in_url(byte), doc.url_span_at(byte).is_some());
            assert_eq!(
                doc.line_span(byte),
                brute_line_span(src, byte),
                "line_span mismatch at byte {byte}"
            );
        }
    }

    #[test]
    fn html_lang_parses_common_tag_shapes() {
        for (src, want) in [
            (
                "<html lang=\"pt-BR\"><body>x</body></html>\n",
                Some(NatLang::PtBr),
            ),
            (
                "<html lang='pt_br'><body>x</body></html>\n",
                Some(NatLang::PtBr),
            ),
            ("<html lang=PT><body>x</body></html>\n", Some(NatLang::PtBr)),
            (
                "<html lang=\"en\"><body>x</body></html>\n",
                Some(NatLang::En),
            ),
            (
                "<!doctype html>\n<html lang=\"pt-BR\"><body>x</body></html>\n",
                Some(NatLang::PtBr),
            ),
        ] {
            assert_eq!(ProseDoc::parse_html(src).html_lang, want, "{src}");
        }
    }

    /// The bug this fixes: a raw-text scan for `<html ...lang=...>` matches whichever occurrence
    /// comes first in the file, comment included, so a decoy tag inside a leading comment used to
    /// win over the page's real root tag. Reading the tree's actual root element instead means a
    /// `comment` node never stands in for it.
    #[test]
    fn html_lang_ignores_a_decoy_tag_inside_a_leading_comment() {
        let src =
            "<!-- fallback: <html lang=\"en\"> -->\n<html lang=\"pt-BR\"><body>x</body></html>\n";
        assert_eq!(ProseDoc::parse_html(src).html_lang, Some(NatLang::PtBr));
    }

    #[test]
    fn html_lang_ignores_inner_element_and_unknown_or_missing_tags() {
        let inner = "<html><body><p lang=\"pt-BR\">x</p></body></html>\n";
        assert_eq!(ProseDoc::parse_html(inner).html_lang, None);
        let missing = "<html><body>x</body></html>\n";
        assert_eq!(ProseDoc::parse_html(missing).html_lang, None);
        let unknown = "<html lang=\"fr\"><body>x</body></html>\n";
        assert_eq!(ProseDoc::parse_html(unknown).html_lang, None);
    }

    #[test]
    fn html_lang_is_none_for_the_markdown_family() {
        assert_eq!(ProseDoc::parse("hello\n").html_lang, None);
    }
}
