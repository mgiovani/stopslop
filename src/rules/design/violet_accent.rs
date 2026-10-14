use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{Lang, ALL_NATLANGS};
use crate::registry::RuleDef;
use crate::style::utility;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP055",
    name: "Violet Tailwind accent",
    tier: Tier::B,
    langs: &[Lang::Html, Lang::Tsx],
    natlangs: ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

static ACCENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:text|bg|border|ring|fill|stroke|outline|decoration|accent|caret|divide|shadow)-(?:purple|violet|indigo)-(?:400|500|600|700)(?:/\d+)?$",
    )
    .unwrap()
});

const FIX: &str = "use the project's brand color token";

/// Gradient stops (`from-`, `via-`, `to-`) never match `ACCENT`, so SLOP053 keeps them; a list
/// with `bg-clip-text` is SLOP052's. One diagnostic per class list keeps a styled button quiet.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(style) = ctx.style else { return };
    for list in &style.class_lists {
        if list.iter().any(|t| utility(t.text) == "bg-clip-text") {
            continue;
        }
        if let Some(token) = list.iter().find(|t| ACCENT.is_match(utility(t.text))) {
            let (line, col) = style.line_col(token.start);
            out.push(Diagnostic::at_fix(
                rule,
                ctx,
                line,
                col,
                "violet tailwind accent, the default generated-UI palette",
                FIX,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::diagnostics_for as run;
    use super::*;

    fn diagnostics_for(lang: Lang, src: &str) -> Vec<Diagnostic> {
        run(&RULE, lang, src)
    }

    #[test]
    fn flags_accent_utilities_with_variants_and_opacity() {
        for class in [
            "bg-violet-600",
            "hover:text-indigo-400",
            "ring-purple-500/50",
            "md:!border-violet-700",
        ] {
            let src = format!("<a class=\"p-2 {class}\">x</a>\n");
            assert_eq!(diagnostics_for(Lang::Html, &src).len(), 1, "{class}");
        }
    }

    #[test]
    fn reports_the_first_token_of_a_list_only() {
        let d = diagnostics_for(
            Lang::Tsx,
            "const a = <b className=\"bg-violet-600 text-indigo-500\" />;\n",
        );
        assert_eq!((d.len(), d[0].col), (1, 25));
    }

    #[test]
    fn skips_tints_shades_and_other_hues() {
        let src = "<a class=\"bg-purple-50 text-violet-900 bg-violet-300 bg-blue-600 border-indigo-800\">x</a>\n";
        assert!(diagnostics_for(Lang::Html, src).is_empty());
    }

    #[test]
    fn leaves_gradients_to_their_rules() {
        let stops = "<a class=\"from-violet-500 via-purple-500 to-indigo-600\">x</a>\n";
        assert!(diagnostics_for(Lang::Html, stops).is_empty());
        let clip = "<h1 class=\"bg-clip-text text-violet-500\">x</h1>\n";
        assert!(diagnostics_for(Lang::Html, clip).is_empty());
    }

    #[test]
    fn flags_jsx_class_names_and_ignores_raw_css() {
        assert_eq!(
            diagnostics_for(
                Lang::Tsx,
                "const a = <p className=\"text-violet-500\" />;\n"
            )
            .len(),
            1
        );
        assert!(
            diagnostics_for(Lang::Tsx, "const a = <p style={{ color: '#7c3aed' }} />;\n")
                .is_empty()
        );
        assert!(
            diagnostics_for(Lang::Html, "<p style=\"color: rebeccapurple\">x</p>\n").is_empty()
        );
    }

    #[test]
    fn clean_twin_and_empty_input() {
        assert!(
            diagnostics_for(Lang::Html, "<a class=\"bg-slate-900 text-white\">x</a>\n").is_empty()
        );
        assert!(diagnostics_for(Lang::Html, "").is_empty());
    }

    #[test]
    fn reads_the_important_marker_in_both_positions_and_the_v4_prefix() {
        for class in [
            "!bg-violet-600",
            "bg-violet-600!",
            "tw:bg-violet-600",
            "tw:hover:bg-violet-600",
        ] {
            let src = format!("<a class=\"{class}\">x</a>\n");
            assert_eq!(diagnostics_for(Lang::Html, &src).len(), 1, "{class}");
        }
    }

    #[test]
    fn skips_the_v4_variable_shorthand_and_arbitrary_values() {
        let src =
            "<a class=\"bg-(--brand) text-[#7c3aed] border-[oklch(0.606_0.25_292.717)]\">x</a>\n";
        assert!(diagnostics_for(Lang::Html, src).is_empty());
    }

    // ponytail: a v3 `tw-` config prefix (`tw-bg-violet-600`) is not matched; reading it needs the
    // project's tailwind config, so a prefixed v3 project is a known miss.
    #[test]
    fn v3_config_prefixes_are_a_known_ceiling() {
        assert!(diagnostics_for(Lang::Html, "<a class=\"tw-bg-violet-600\">x</a>\n").is_empty());
    }
}
