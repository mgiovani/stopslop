use super::is_gradient_utility;
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
        r"^(?:text|bg|border(?:-[xytrblse])?|ring(?:-offset)?|inset-ring|inset-shadow|fill|stroke|outline|decoration|accent|caret|divide|shadow|placeholder)-(?:purple|violet|indigo)-(?:400|500|600|700)(?:/(?:\d+|\[[^\]]+\]|\(--[\w-]+\)))?$",
    )
    .unwrap()
});

const FIX: &str = "use the project's brand color token";

/// Gradient stops (`from-`, `via-`, `to-`) never match `ACCENT`, so SLOP053 keeps them. A list that
/// clips a gradient to its text is SLOP052's; `bg-clip-text` without a gradient is left alone here.
/// One diagnostic per class list keeps a styled button quiet. A v3 `tw-` config prefix is not
/// read: that needs the project's tailwind config.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(style) = ctx.style else { return };
    for list in &style.class_lists {
        let utilities: Vec<_> = list.iter().map(|t| (t, utility(t.text))).collect();
        let clips_gradient = utilities.iter().any(|&(_, u)| u == "bg-clip-text")
            && utilities.iter().any(|&(_, u)| is_gradient_utility(u));
        if clips_gradient {
            continue;
        }
        if let Some((token, _)) = utilities.iter().find(|&&(_, u)| ACCENT.is_match(u)) {
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

    fn html(class: &str) -> Vec<Diagnostic> {
        diagnostics_for(Lang::Html, &format!("<a class=\"{class}\">x</a>\n"))
    }

    #[test]
    fn flags_accent_utilities_with_variants_and_opacity() {
        for class in [
            "bg-violet-600",
            "hover:text-indigo-400",
            "ring-purple-500/50",
            "md:!border-violet-700",
        ] {
            assert_eq!(html(&format!("p-2 {class}")).len(), 1, "{class}");
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
    fn skips_tints_shades_and_other_hues_one_edge_at_a_time() {
        for class in [
            "bg-purple-50",
            "text-violet-900",
            "bg-violet-300",
            "bg-violet-800",
            "bg-blue-600",
            "border-indigo-800",
            "bg-fuchsia-500",
            "from-violet-500",
            "bg-violet-5000",
        ] {
            assert!(html(class).is_empty(), "{class}");
        }
        for class in ["bg-violet-400", "bg-violet-700"] {
            assert_eq!(html(class).len(), 1, "{class}");
        }
    }

    #[test]
    fn leaves_gradient_stops_and_gradient_text_to_their_rules() {
        assert!(html("from-violet-500 via-purple-500 to-indigo-600").is_empty());
        for clip in [
            "bg-clip-text",
            "md:bg-clip-text",
            "!bg-clip-text",
            "bg-clip-text!",
        ] {
            assert!(
                html(&format!("{clip} bg-gradient-to-r text-violet-500")).is_empty(),
                "{clip}"
            );
        }
    }

    #[test]
    fn bg_clip_text_without_a_gradient_is_not_slop052_so_the_accent_still_counts() {
        assert_eq!(html("bg-clip-text text-violet-500").len(), 1);
    }

    #[test]
    fn reads_side_specific_offset_placeholder_and_arbitrary_opacity_forms() {
        for class in [
            "border-t-violet-500",
            "ring-offset-violet-500",
            "placeholder-violet-500",
            "inset-ring-indigo-600",
            "bg-violet-600/[0.5]",
            "bg-violet-600/(--o)",
            "peer-hover:text-purple-400",
        ] {
            assert_eq!(html(class).len(), 1, "{class}");
        }
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
    fn clean_twin() {
        assert!(html("bg-slate-900 text-white").is_empty());
    }

    #[test]
    fn empty_input_and_the_fix_text() {
        assert!(diagnostics_for(Lang::Html, "").is_empty());
        let d = html("bg-violet-600");
        assert_eq!(
            d[0].fix.as_deref(),
            Some("use the project's brand color token")
        );
    }

    #[test]
    fn reads_the_important_marker_in_both_positions_and_the_v4_prefix() {
        for class in [
            "!bg-violet-600",
            "bg-violet-600!",
            "tw:bg-violet-600",
            "tw:hover:bg-violet-600",
        ] {
            assert_eq!(html(class).len(), 1, "{class}");
        }
    }

    #[test]
    fn skips_the_v4_variable_shorthand_and_arbitrary_values() {
        assert!(html("bg-(--brand) text-[#7c3aed] border-[oklch(0.606_0.25_292.717)]").is_empty());
    }

    #[test]
    fn v3_config_prefixes_are_a_known_ceiling() {
        assert!(html("tw-bg-violet-600").is_empty());
    }
}
