use super::is_gradient;
use crate::color::{colors_in, hsl, Rgba};
use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{ALL_NATLANGS, STYLE_LANGS};
use crate::registry::RuleDef;
use crate::style::utility;
use std::ops::RangeInclusive;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP053",
    name: "Purple-blue gradient",
    tier: Tier::C,
    langs: STYLE_LANGS,
    natlangs: ALL_NATLANGS,
    default_on: false,
    path_gated: false,
    check,
};

const MIN_SATURATION: f32 = 0.35;
const HUE_BAND: RangeInclusive<f32> = 225.0..=330.0;
const PURPLE_CORE: RangeInclusive<f32> = 255.0..=290.0;
const PALETTE_FROM: &[&str] = &["purple", "violet", "indigo"];
const PALETTE_REST: &[&str] = &[
    "purple", "violet", "indigo", "blue", "cyan", "pink", "fuchsia",
];
const FIX: &str = "pick the brand's own colors";

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(style) = ctx.style else { return };
    for d in style
        .decls
        .iter()
        .filter(|d| is_gradient(d.value) && is_purple_blue(colors_in(d.value)))
    {
        let (line, col) = style.line_col(d.start);
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            "gradient runs between saturated purple and blue stops",
            FIX,
        ));
    }
    for list in &style.class_lists {
        let from = list
            .iter()
            .find(|t| palette(utility(t.text), "from-", PALETTE_FROM));
        let rest = list.iter().any(|t| {
            let u = utility(t.text);
            ["via-", "to-"].iter().any(|p| palette(u, p, PALETTE_REST))
        });
        if let Some(from) = from.filter(|_| rest) {
            let (line, col) = style.line_col(from.start);
            out.push(Diagnostic::at_fix(
                rule,
                ctx,
                line,
                col,
                "gradient utilities run between purple, violet or indigo and a neighboring hue",
                FIX,
            ));
        }
        let arbitrary: Vec<_> = list
            .iter()
            .filter_map(|t| Some((t, arbitrary_stop(utility(t.text))?)))
            .collect();
        let stops = arbitrary.iter().map(|&(_, c)| c).collect();
        if let Some(&(first, _)) = arbitrary.first().filter(|_| is_purple_blue_stops(stops)) {
            let (line, col) = style.line_col(first.start);
            out.push(Diagnostic::at_fix(
                rule,
                ctx,
                line,
                col,
                "gradient utilities run between saturated purple and blue stops",
                FIX,
            ));
        }
    }
}

fn is_purple_blue(colors: Vec<(usize, usize, Rgba)>) -> bool {
    is_purple_blue_stops(colors.into_iter().map(|(_, _, c)| c).collect())
}

fn is_purple_blue_stops(colors: Vec<Rgba>) -> bool {
    let stops: Vec<(f32, f32)> = colors
        .into_iter()
        .filter(|c| c.a > 0.0)
        .map(|c| {
            let (h, s, _) = hsl(c);
            (h, s)
        })
        .collect();
    stops.len() >= 2
        && stops
            .iter()
            .all(|(h, s)| *s >= MIN_SATURATION && HUE_BAND.contains(h))
        && stops.iter().any(|(h, _)| PURPLE_CORE.contains(h))
}

/// The color of a bracketed gradient stop such as `from-[#7c3aed]` or `to-[rgb(99,102,241)]`.
fn arbitrary_stop(utility: &str) -> Option<Rgba> {
    let rest = ["from-", "via-", "to-"]
        .iter()
        .find_map(|p| utility.strip_prefix(p))?;
    let inner = rest.strip_prefix('[')?.strip_suffix(']')?;
    colors_in(inner).first().map(|&(_, _, c)| c)
}

/// True for `<prefix><name>-<shade>` (optionally `/<opacity>`) with `name` in `names`.
fn palette(utility: &str, prefix: &str, names: &[&str]) -> bool {
    let Some(rest) = utility.strip_prefix(prefix) else {
        return false;
    };
    let rest = rest.split('/').next().unwrap_or(rest);
    rest.rsplit_once('-').is_some_and(|(name, shade)| {
        names.contains(&name) && !shade.is_empty() && shade.bytes().all(|b| b.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::super::diagnostics_for as run;
    use super::*;
    use crate::lang::Lang;

    fn diagnostics_for(lang: Lang, src: &str) -> Vec<Diagnostic> {
        run(&RULE, lang, src)
    }

    fn purple_blue(value: &str) -> bool {
        is_purple_blue(colors_in(value))
    }

    #[test]
    fn the_stock_purple_gradients_are_flagged() {
        assert!(purple_blue(
            "linear-gradient(135deg, #667eea 0%, #764ba2 100%)"
        ));
        assert!(purple_blue(
            "linear-gradient(to right, #6366f1, #8b5cf6, #a855f7)"
        ));
        assert!(purple_blue(
            "linear-gradient(rgb(99 102 241), hsl(270, 80%, 60%))"
        ));
    }

    #[test]
    fn other_gradients_and_partial_matches_are_not() {
        assert!(!purple_blue("linear-gradient(135deg, #ff7e5f, #feb47b)"));
        assert!(!purple_blue("linear-gradient(#7c3aed, white)"));
        assert!(!purple_blue("linear-gradient(#7c3aed)"));
        assert!(!purple_blue("linear-gradient(#0000ff, #0080ff)"));
        assert!(!purple_blue("linear-gradient(#808080, #7c3aed)"));
        assert!(!purple_blue("linear-gradient(var(--a), var(--b))"));
        assert!(!purple_blue(
            "linear-gradient(var(--indigo), url(purple.png))"
        ));
        assert!(!purple_blue("linear-gradient(#7c3aed, transparent)"));
    }

    #[test]
    fn check_flags_a_css_gradient_once_at_the_declaration() {
        let src = ".a {\n  background: linear-gradient(135deg, #667eea, #764ba2);\n}\n";
        let d = diagnostics_for(Lang::Css, src);
        assert_eq!((d.len(), d[0].line, d[0].col), (1, 2, 3));
        assert!(
            diagnostics_for(Lang::Css, ".a { background: linear-gradient(red, #ff0) }").is_empty()
        );
        assert!(diagnostics_for(Lang::Css, "").is_empty());
    }

    #[test]
    fn check_flags_palette_stops_and_bracketed_stops() {
        let html = |src| diagnostics_for(Lang::Html, src);
        assert_eq!(
            html("<i class=\"bg-gradient-to-r hover:from-purple-500 to-blue-500\">x</i>\n").len(),
            1
        );
        assert!(html("<i class=\"from-purple-500 to-red-500\">x</i>\n").is_empty());
        assert_eq!(
            html("<i class=\"from-[#7c3aed] to-[#6366f1]\">x</i>\n").len(),
            1
        );
        assert!(html("<i class=\"from-[#7c3aed] to-[#ff0000]\">x</i>\n").is_empty());
        assert!(html("<i class=\"from-[#7c3aed]\">x</i>\n").is_empty());
    }

    #[test]
    fn check_reads_jsx_style_objects_and_class_names() {
        let src = "const a = <div className=\"from-indigo-500 via-violet-500 to-pink-500\" style={{ background: 'linear-gradient(#667eea, #764ba2)' }} />;\n";
        assert_eq!(diagnostics_for(Lang::Tsx, src).len(), 2);
    }

    #[test]
    fn palette_reads_name_shade_and_opacity() {
        assert!(palette("from-purple-500", "from-", &["purple"]));
        assert!(palette("to-indigo-600/50", "to-", &["indigo"]));
        assert!(!palette("from-purple", "from-", &["purple"]));
        assert!(!palette("from-red-500", "from-", &["purple"]));
        assert!(!palette("via-purple-500", "from-", &["purple"]));
    }
}
