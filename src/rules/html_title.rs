use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, Lang};
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP044",
    name: "Boilerplate or empty page title",
    tier: Tier::B,
    langs: &[Lang::Html],
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

/// `<title>Document</title>` is what the VS Code Emmet `!` expansion (and the pages generated
/// from it) leaves in place; an empty `<title>` is the other way a page ships unnamed. Any other
/// title is the author's call, so the panel stops at these two. Scanned in `ctx.source` rather
/// than the masked stream: the tag is what identifies the span, and the masked stream blanks it.
static RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<title(?:\s[^>]*)?>\s*(document)?\s*</title\s*>").unwrap());
static OPEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<title[\s>]").unwrap());

/// Only the first `<title>` in the document is the page title. A later one is an SVG
/// accessible name (`<svg><title></title></svg>`), which is a different element with its own
/// rules, so it never counts here.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let Some(first) = OPEN.find(ctx.source) else {
        return;
    };
    let Some(caps) = RE.captures_at(ctx.source, first.start()) else {
        return;
    };
    let m = caps.get(0).unwrap();
    if m.start() != first.start() {
        return;
    }
    let (line, col) = doc.line_col(m.start());
    let message = if caps.get(1).is_some() {
        "page title is the editor boilerplate \"Document\""
    } else {
        "page title is empty"
    };
    out.push(Diagnostic::at_fix(
        rule,
        ctx,
        line,
        col,
        message,
        "name the page",
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::ProseDoc;

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        let doc = ProseDoc::parse_html(src);
        let ctx = LintContext {
            display_path: "test.html".to_string(),
            source: src,
            index: None,
            lang: Lang::Html,
            comments: &doc.ignore_comments,
            strings: &doc.attr_values,
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
    fn flags_emmet_default_title() {
        let diags =
            diagnostics_for("<html>\n<head>\n  <title>Document</title>\n</head>\n</html>\n");
        assert_eq!(diags.len(), 1);
        assert_eq!((diags[0].line, diags[0].col), (3, 3));
        assert!(diags[0].message.contains("Document"));
    }

    #[test]
    fn flags_case_and_whitespace_variants() {
        assert_eq!(diagnostics_for("<TITLE>document</TITLE>\n").len(), 1);
        assert_eq!(
            diagnostics_for("<title lang=\"en\">\n  Document\n</title>\n").len(),
            1
        );
    }

    #[test]
    fn flags_empty_title() {
        let diags = diagnostics_for("<head>\n<title>\n</title>\n</head>\n");
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("empty"));
    }

    #[test]
    fn clean_named_titles() {
        for src in [
            "<title>Acme Notes</title>\n",
            "<title>Documentation</title>\n",
            "<title>{{ page.title }}</title>\n",
            "<title>Acme</title>\n<svg><title></title></svg>\n",
        ] {
            assert!(diagnostics_for(src).is_empty(), "{src}");
        }
    }
}
