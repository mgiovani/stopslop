use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP020",
    name: "Typographic (smart) quotes in source",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: true,
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
    for (byte, ch) in doc.masked.char_indices() {
        if !matches!(ch, '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}') {
            continue;
        }
        count += 1;
        first_byte.get_or_insert(byte);
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
