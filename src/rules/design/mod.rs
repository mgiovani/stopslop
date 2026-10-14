pub mod gradient_text;
pub mod purple_gradient;
pub mod violet_hex;

use crate::style::Decl;

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
            let style = if lang == Lang::Css {
                StyleDoc::from_css(&index, src)
            } else {
                StyleDoc::from_jsx(&index, src, &strings)
            };
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
