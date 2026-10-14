pub mod gradient_text;
pub mod purple_gradient;
pub mod violet_accent;
pub mod violet_hex;
pub mod violet_theme;

use crate::context::LintContext;
use crate::diagnostic::Diagnostic;
use crate::registry::RuleDef;
use crate::style::{Decl, StyleDoc};

pub(crate) const STOP_PREFIXES: [&str; 3] = ["from-", "via-", "to-"];

pub(crate) fn is_gradient_stop(u: &str) -> bool {
    STOP_PREFIXES.iter().any(|p| u.starts_with(p))
}

pub(crate) fn report(
    rule: &'static RuleDef,
    ctx: &LintContext,
    style: &StyleDoc,
    byte: usize,
    message: impl Into<String>,
    fix: &str,
    out: &mut Vec<Diagnostic>,
) {
    let (line, col) = style.line_col(byte);
    out.push(Diagnostic::at_fix(rule, ctx, line, col, message, fix));
}

pub(crate) fn is_gradient_utility(u: &str) -> bool {
    u.starts_with("bg-gradient-to-")
        || u.starts_with("bg-linear-")
        || u.starts_with("bg-radial")
        || u.starts_with("bg-conic")
        || (u.starts_with("bg-[") && u.contains("gradient("))
}

pub(crate) fn is_gradient(value: &str) -> bool {
    value.contains("gradient(")
}

/// `-webkit-background-clip` and `background-clip` name the same property.
pub(crate) fn unprefixed(property: &str) -> &str {
    property.strip_prefix("-webkit-").unwrap_or(property)
}

pub(crate) fn is_gradient_background(d: &Decl) -> bool {
    matches!(unprefixed(&d.property), "background" | "background-image") && is_gradient(d.value)
}

#[cfg(test)]
pub(crate) fn diagnostics_for(
    rule: &'static crate::registry::RuleDef,
    lang: crate::lang::Lang,
    src: &str,
) -> Vec<crate::diagnostic::Diagnostic> {
    use crate::context::{extract, LintContext};
    use crate::lang::{ts_language, Lang, ALL_NATLANGS};
    use crate::prose::ProseDoc;
    use crate::style::StyleDoc;

    let doc;
    let tree;
    let (style, comments) = match lang {
        Lang::Html => {
            doc = ProseDoc::parse_html(src);
            (StyleDoc::from_html(&doc, src), Vec::new())
        }
        _ => {
            let mut p = tree_sitter::Parser::new();
            p.set_language(&ts_language(lang)).unwrap();
            tree = p.parse(src, None).unwrap();
            let (comments, strings, index) = extract(&tree, src, lang);
            let style = StyleDoc::for_code(lang, &index, src, &strings).unwrap();
            (style, comments)
        }
    };
    let ctx = LintContext {
        display_path: "t".into(),
        source: src,
        index: None,
        lang,
        comments: &comments,
        strings: &[],
        is_test_path: false,
        is_stub_file: false,
        deps: None,
        prose: None,
        image: None,
        style: Some(&style),
        natlangs: ALL_NATLANGS,
    };
    let mut out = Vec::new();
    (rule.check)(rule, &ctx, &mut out);
    out
}
