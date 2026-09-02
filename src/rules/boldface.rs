use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, Lang};
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP019",
    name: "Boldface & bold-lead-in list overuse",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Html],
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

/// `**bold**` / `__bold__` inline spans. Line-local only (mirrors ProseDoc's own inline-code
/// masking convention: a bold span can't cross a newline).
static BOLD_SPAN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\*\*[^*\n]+\*\*|__[^_\n]+__").unwrap());

/// Bold-lead-in list item: `- **Term**: ...` / `1. **Term**:`. The colon/dash must be OUTSIDE
/// the closing bold markers — a colon baked inside the bold span itself (`- **Term:** ...`) does
/// not count, per the catalog detection_spec.
static BOLD_LEAD_IN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:[-*+]\s+\*\*[^*\n]+\*\*\s*[:—-]|\d+[.)]\s+\*\*[^*\n]+\*\*\s*:)").unwrap()
});

/// (a) boldface density over body prose (skip headings/frontmatter; code is already masked) and
/// (b) a run of >=3 consecutive bold-lead-in list items in one block. Up to 2 diagnostics, each
/// anchored at its first offender.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    let mut bold_count = 0usize;
    let mut first_bold_byte = None;
    // Markdown bold is `**` in the masked stream; HTML `<strong>`/`<b>` tags are blanked there,
    // so the parse records their start bytes instead. A document has one or the other.
    let markdown_bold = BOLD_SPAN_RE.find_iter(&doc.masked).map(|m| m.start());
    for byte in markdown_bold.chain(doc.bold_spans.iter().copied()) {
        if doc.in_heading(byte) || doc.in_frontmatter(byte) {
            continue;
        }
        bold_count += 1;
        first_bold_byte.get_or_insert(byte);
    }
    if bold_count >= 4 && bold_count * 40 > doc.words {
        let (line, col) = doc.line_col(first_bold_byte.unwrap());
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            format!("boldface overuse in body prose ({bold_count} bold spans)"),
        ));
    }

    if let Some((start_byte, run_len)) = first_bold_lead_in_run(doc) {
        let (line, col) = doc.line_col(start_byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            format!("bold-lead-in list overuse ({run_len} consecutive bolded items)"),
        ));
    }
}

/// First contiguous run of >=3 bold-lead-in items within a single list block, scanning blocks in
/// document order. Returns (marker byte of the run's first item, full run length).
fn first_bold_lead_in_run(doc: &crate::prose::ProseDoc<'_>) -> Option<(usize, usize)> {
    for block in &doc.list_blocks {
        let mut run_start = 0usize;
        let mut run_len = 0usize;
        for item in &block.items {
            // `ListItem` only records the marker byte, so slice the full line to anchor `^`.
            let (start, end) = doc.line_span(item.marker_byte);
            let is_match = BOLD_LEAD_IN_RE.is_match(&doc.masked[start..end]);
            if is_match {
                if run_len == 0 {
                    run_start = item.marker_byte;
                }
                run_len += 1;
            } else {
                if run_len >= 3 {
                    return Some((run_start, run_len));
                }
                run_len = 0;
            }
        }
        if run_len >= 3 {
            return Some((run_start, run_len));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn html_strong_and_b_count_toward_density_outside_headings() {
        let dense = "<h2><strong>Not counted</strong></h2>\n<p>Body with <strong>one</strong> <b>two</b> <strong>three</strong> <strong>four</strong> bold spans.</p>\n";
        let diags = diagnostics_for_html(dense);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 2);
        let sparse = "<p>One <strong>bold</strong> word in a paragraph that otherwise runs on plainly with enough ordinary words around it to keep the density low.</p>\n";
        assert!(diagnostics_for_html(sparse).is_empty());
    }

    #[test]
    fn flags_bold_density_overuse() {
        let src =
            "Body text with **one** **two** **three** **four** bold spans in one short line.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP019");
    }

    #[test]
    fn flags_bold_lead_in_list_run() {
        let src = "- **One**: first item.\n- **Two**: second item.\n- **Three**: third item.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP019");
    }

    #[test]
    fn flags_both_density_and_lead_in_run_together() {
        // Satisfies both sub-checks in one doc: 4 bold spans dense enough to trip (a), plus a
        // 3-item bold-lead-in run to trip (b) -- must emit both diagnostics, not just one.
        let src = "- **One**: first item.\n- **Two**: second item.\n- **Three**: third item.\n\n**Four** more bold here.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.code == "SLOP019"));
    }

    #[test]
    fn clean_two_item_glossary_and_single_stray_bold() {
        let src = "- **One**: first item.\n- **Two**: second item.\n\nA long paragraph with just one **bold** word among plenty of ordinary text that keeps the density low across many words in this sentence right here today for good measure and then some.\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
