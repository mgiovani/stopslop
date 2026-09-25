use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, PROSE_LANGS};
use crate::registry::RuleDef;

/// `default_on: false` (issue #61): rust-book (98.10%, 103/105 files) and diplomatrix (65.91%
/// human vs 2.05% AI) are genuine editorial curly-quote house style, not an AI tell, and the
/// pooled AI rate has since risen to 6.61% vs 7.29% human (lift 0.91, effectively chance) as newer
/// AI datasets increasingly type real curly quotes too. Tier stays B: this remains a legitimate
/// opt-in style check for a project that wants straight quotes, just not a default-on slop signal.
pub static RULE: RuleDef = RuleDef {
    code: "SLOP020",
    name: "Typographic (smart) quotes in source",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: lang::ALL_NATLANGS,
    default_on: false,
    path_gated: false,
    check,
};

/// Counts U+2018/2019/201C/201D (curly single/double quotes and apostrophes) in the masked
/// prose stream. Headings and frontmatter are in scope (only code is masked out); a single
/// stray curly apostrophe (as in a pasted "it's") is common enough on its own not to fire, so
/// the floor is 2.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let mut count = 0usize;
    let mut first_byte = None;
    // `&ldquo;`-style entities are the same quotes; the parse decodes them into `doc.entities`.
    let typed = doc.masked.char_indices();
    for (byte, ch) in typed.chain(doc.entities.iter().copied()) {
        if !matches!(ch, '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}') {
            continue;
        }
        count += 1;
        first_byte = Some(first_byte.map_or(byte, |f: usize| f.min(byte)));
    }
    if count >= 2 {
        let (line, col) = doc.line_col(first_byte.unwrap());
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "typographic (smart) quotes in source: prefer straight ASCII quotes".to_string(),
        ));
    }
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
            comments: &doc.comments,
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: Some(&doc),
            image: None,
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn html_quote_entities_count_as_smart_quotes() {
        let src = "<p>They said &ldquo;done&rdquo; and left.</p>\n";
        let diags = diagnostics_for_html(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].col, src.find("&ldquo;").unwrap() + 1);
        assert!(diagnostics_for_html("<p>one &rdquo; only</p>\n").is_empty());
    }

    #[test]
    fn flags_curly_quotes_at_or_above_floor() {
        let src = "It\u{2019}s a \u{201C}quoted\u{201D} word.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP020");
    }

    #[test]
    fn clean_single_curly_apostrophe_below_floor() {
        let src = "It\u{2019}s the only curly mark in this file.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn skips_curly_quotes_inside_code_fence() {
        let src =
            "Body text.\n```\n\u{201C}curly\u{201D} inside a fence\n```\nMore plain body text.\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
