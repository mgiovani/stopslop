use super::{is_gradient_background, is_gradient_utility, report, unprefixed};
use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{ALL_NATLANGS, STYLE_LANGS};
use crate::registry::RuleDef;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP052",
    name: "Gradient text",
    tier: Tier::B,
    langs: STYLE_LANGS,
    natlangs: ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

const FIX: &str = "use a solid text color";

fn clips_to_text(value: &str) -> bool {
    value
        .trim_end_matches("!important")
        .split(',')
        .any(|item| item.trim().eq_ignore_ascii_case("text"))
}

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(style) = ctx.style else { return };
    for block in style.blocks() {
        let clip = block
            .iter()
            .find(|d| unprefixed(&d.property) == "background-clip" && clips_to_text(d.value));
        if let Some(clip) = clip.filter(|_| block.iter().any(is_gradient_background)) {
            let message = format!(
                "text is filled with a gradient through {}: {}",
                clip.property, clip.value
            );
            report(rule, ctx, style, clip.start, message, FIX, out);
        }
    }
    for list in &style.class_lists {
        let clip = list.iter().find(|t| t.utility == "bg-clip-text");
        let gradient = list.iter().find(|t| is_gradient_utility(t.utility));
        if let Some((clip, gradient)) = clip.zip(gradient) {
            let message = format!(
                "text is filled with a gradient through {} and {}",
                clip.text, gradient.text
            );
            report(rule, ctx, style, clip.start, message, FIX, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::diagnostics_for as run;
    use super::*;
    use crate::lang::Lang;

    fn diagnostics_for(lang: Lang, src: &str) -> Vec<Diagnostic> {
        run(&RULE, lang, src)
    }

    #[test]
    fn flags_clip_text_over_a_gradient_in_one_block() {
        let src = ".t {\n  background: linear-gradient(90deg, red, blue);\n  -webkit-background-clip: text;\n}\n";
        let d = diagnostics_for(Lang::Css, src);
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].line, d[0].col), (3, 3));
    }

    #[test]
    fn ignores_clip_text_over_an_image_and_gradients_without_clip() {
        let css = |src| diagnostics_for(Lang::Css, src);
        assert!(css(".t { background: url(a.png); background-clip: text }").is_empty());
        assert!(css(".t { background: linear-gradient(red, blue) }").is_empty());
        assert!(
            css(".a { background-clip: text } .b { background: linear-gradient(red, blue) }")
                .is_empty()
        );
        assert!(css("").is_empty());
    }

    #[test]
    fn flags_tailwind_clip_text_with_a_gradient_utility() {
        let html = |src| diagnostics_for(Lang::Html, src);
        assert_eq!(
            html("<h1 class=\"md:bg-clip-text bg-linear-to-r text-transparent\">x</h1>\n").len(),
            1
        );
        assert_eq!(
            html("<h1 class = \"bg-clip-text bg-gradient-to-r\">x</h1>\n").len(),
            1
        );
        assert!(html("<h1 class=\"bg-clip-text bg-red-500\">x</h1>\n").is_empty());
    }

    #[test]
    fn flags_a_style_element_and_a_style_attribute() {
        let src = "<style>\n.h { background-image: linear-gradient(red, blue); background-clip: text }\n</style>\n<p style=\"background: linear-gradient(red, blue); -webkit-background-clip: text\">x</p>\n";
        let d = diagnostics_for(Lang::Html, src);
        assert_eq!(d.iter().map(|d| d.line).collect::<Vec<_>>(), [2, 4]);
    }

    #[test]
    fn flags_a_jsx_style_object() {
        let src = "const a = <h1 style={{ backgroundImage: 'linear-gradient(red, blue)', WebkitBackgroundClip: 'text' }} />;\n";
        assert_eq!(diagnostics_for(Lang::Tsx, src).len(), 1);
    }

    #[test]
    fn crlf_and_non_ascii_input_keep_line_and_column() {
        let src = ".t {\r\n  /* é */ background: linear-gradient(red, blue); background-clip: text;\r\n}\r\n";
        let d = diagnostics_for(Lang::Css, src);
        assert_eq!((d[0].line, d[0].col), (2, 51));
    }

    #[test]
    fn reads_every_tailwind_v3_and_v4_gradient_utility() {
        for gradient in [
            "bg-gradient-to-r",
            "bg-linear-to-br",
            "bg-linear-45",
            "bg-radial",
            "bg-radial-[at_25%_25%]",
            "bg-conic",
            "bg-conic-180",
            "bg-[linear-gradient(red,blue)]",
        ] {
            let src = format!("<h1 class=\"bg-clip-text {gradient} text-transparent\">x</h1>\n");
            assert_eq!(diagnostics_for(Lang::Html, &src).len(), 1, "{gradient}");
        }
    }

    #[test]
    fn reads_the_important_marker_and_variant_prefixes_on_bg_clip_text() {
        for clip in [
            "!bg-clip-text",
            "bg-clip-text!",
            "tw:bg-clip-text",
            "md:bg-clip-text",
        ] {
            let src = format!("<h1 class=\"{clip} bg-linear-to-r\">x</h1>\n");
            assert_eq!(diagnostics_for(Lang::Html, &src).len(), 1, "{clip}");
        }
        assert!(diagnostics_for(
            Lang::Html,
            "<h1 class=\"bg-clip-text bg-(--brand)\">x</h1>\n"
        )
        .is_empty());
    }

    #[test]
    fn flags_a_tsx_class_name_and_a_nested_scss_block() {
        let tsx = "const a = <h1 className=\"bg-clip-text bg-gradient-to-r\">x</h1>;\n";
        let d = diagnostics_for(Lang::Tsx, tsx);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("bg-clip-text and bg-gradient-to-r"));
        let scss = ".a { .b { background: linear-gradient(red, blue); background-clip: text; } }";
        assert_eq!(diagnostics_for(Lang::Css, scss).len(), 1);
    }

    #[test]
    fn reads_clip_text_in_any_case_with_important_or_in_a_list() {
        for clip in [
            "TEXT",
            "text !important",
            "border-box, text",
            "text, border-box",
        ] {
            let src =
                format!(".a {{ background: linear-gradient(red, blue); background-clip: {clip} }}");
            assert_eq!(diagnostics_for(Lang::Css, &src).len(), 1, "{clip}");
        }
        for clip in ["border-box", "textual", "padding-box, content-box"] {
            let src =
                format!(".a {{ background: linear-gradient(red, blue); background-clip: {clip} }}");
            assert!(diagnostics_for(Lang::Css, &src).is_empty(), "{clip}");
        }
    }
}
