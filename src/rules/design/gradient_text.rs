use super::{is_gradient_background, unprefixed};
use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{ALL_NATLANGS, STYLE_LANGS};
use crate::registry::RuleDef;
use crate::style::utility;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP052",
    name: "Gradient text",
    tier: Tier::C,
    langs: STYLE_LANGS,
    natlangs: ALL_NATLANGS,
    default_on: false,
    path_gated: false,
    check,
};

const FIX: &str = "use a solid text color";

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(style) = ctx.style else { return };
    for block in style.blocks() {
        let clip = block
            .iter()
            .find(|d| unprefixed(&d.property) == "background-clip" && d.value.starts_with("text"));
        if let Some(clip) = clip.filter(|_| block.iter().any(is_gradient_background)) {
            let (line, col) = style.line_col(clip.start);
            out.push(Diagnostic::at_fix(
                rule,
                ctx,
                line,
                col,
                "text is filled with a gradient through background-clip: text",
                FIX,
            ));
        }
    }
    for list in &style.class_lists {
        let clip = list
            .tokens
            .iter()
            .find(|t| utility(t.text) == "bg-clip-text");
        let gradient = list
            .tokens
            .iter()
            .any(|t| is_gradient_utility(utility(t.text)));
        if let Some(clip) = clip.filter(|_| gradient) {
            let (line, col) = style.line_col(clip.start);
            out.push(Diagnostic::at_fix(
                rule,
                ctx,
                line,
                col,
                "text is filled with a gradient through bg-clip-text",
                FIX,
            ));
        }
    }
}

fn is_gradient_utility(u: &str) -> bool {
    u.starts_with("bg-gradient-to-")
        || u.starts_with("bg-linear-")
        || u.starts_with("bg-radial")
        || u.starts_with("bg-conic")
        || (u.starts_with("bg-[") && u.contains("gradient("))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::extract;
    use crate::lang::{ts_language, Lang};
    use crate::prose::ProseDoc;
    use crate::style::StyleDoc;
    use tree_sitter::Parser;

    fn run(lang: Lang, src: &str) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        let mut p = Parser::new();
        match lang {
            Lang::Html => {
                let style = StyleDoc::from_html(&ProseDoc::parse_html(src), src);
                check(&RULE, &ctx(lang, src, &style), &mut out);
            }
            _ => {
                p.set_language(&ts_language(lang)).unwrap();
                let tree = p.parse(src, None).unwrap();
                let (_, strings, index) = extract(&tree, src, lang);
                let style = if lang == Lang::Css {
                    StyleDoc::from_css(&index, src)
                } else {
                    StyleDoc::from_jsx(&index, src, &strings)
                };
                check(&RULE, &ctx(lang, src, &style), &mut out);
            }
        }
        out
    }

    fn ctx<'a>(lang: Lang, src: &'a str, style: &'a StyleDoc<'a>) -> LintContext<'a> {
        LintContext {
            display_path: "t".into(),
            source: src,
            index: None,
            lang,
            comments: &[],
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: None,
            image: None,
            style: Some(style),
            natlangs: ALL_NATLANGS,
        }
    }

    #[test]
    fn flags_clip_text_over_a_gradient_in_one_block() {
        let src = ".t {\n  background: linear-gradient(90deg, red, blue);\n  -webkit-background-clip: text;\n}\n";
        let d = run(Lang::Css, src);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].line, 3);
    }

    #[test]
    fn ignores_clip_text_over_an_image_and_gradients_without_clip() {
        assert!(run(
            Lang::Css,
            ".t { background: url(a.png); background-clip: text }"
        )
        .is_empty());
        assert!(run(Lang::Css, ".t { background: linear-gradient(red, blue) }").is_empty());
        assert!(run(
            Lang::Css,
            ".a { background-clip: text } .b { background: linear-gradient(red, blue) }"
        )
        .is_empty());
    }

    #[test]
    fn flags_tailwind_clip_text_with_a_gradient_utility() {
        let src = "<h1 class=\"md:bg-clip-text bg-linear-to-r text-transparent\">x</h1>\n";
        assert_eq!(run(Lang::Html, src).len(), 1);
        assert!(run(Lang::Html, "<h1 class=\"bg-clip-text bg-red-500\">x</h1>\n").is_empty());
    }

    #[test]
    fn flags_a_jsx_style_object() {
        let src = "const a = <h1 style={{ backgroundImage: 'linear-gradient(red, blue)', WebkitBackgroundClip: 'text' }} />;\n";
        assert_eq!(run(Lang::Tsx, src).len(), 1);
    }
}
