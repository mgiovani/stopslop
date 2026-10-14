use super::is_gradient;
use crate::color::{colors_in, hsl};
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

// 0.35 rather than 0.4: the stock #764ba2 stop sits at HSL saturation 0.37.
const MIN_SATURATION: f32 = 0.35;
const HUE_BAND: RangeInclusive<f32> = 225.0..=330.0;
const PURPLE_CORE: RangeInclusive<f32> = 255.0..=290.0;
const FIX: &str = "pick the brand's own colors";

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(style) = ctx.style else { return };
    for d in style
        .decls
        .iter()
        .filter(|d| is_gradient(d.value) && is_purple_blue(d.value))
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
            .tokens
            .iter()
            .find(|t| palette(utility(t.text), "from-", &["purple", "violet", "indigo"]));
        let rest = list.tokens.iter().any(|t| {
            let u = utility(t.text);
            ["via-", "to-"].iter().any(|p| {
                palette(
                    u,
                    p,
                    &[
                        "purple", "violet", "indigo", "blue", "cyan", "pink", "fuchsia",
                    ],
                )
            })
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
    }
}

fn is_purple_blue(value: &str) -> bool {
    let stops: Vec<(f32, f32)> = colors_in(value)
        .into_iter()
        .filter(|(_, _, c)| c.a > 0.0)
        .map(|(_, _, c)| {
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
    use super::*;

    #[test]
    fn the_stock_purple_gradients_are_flagged() {
        assert!(is_purple_blue(
            "linear-gradient(135deg, #667eea 0%, #764ba2 100%)"
        ));
        assert!(is_purple_blue(
            "linear-gradient(to right, #6366f1, #8b5cf6, #a855f7)"
        ));
        assert!(is_purple_blue(
            "linear-gradient(rgb(99 102 241), hsl(270, 80%, 60%))"
        ));
    }

    #[test]
    fn other_gradients_and_partial_matches_are_not() {
        assert!(!is_purple_blue("linear-gradient(135deg, #ff7e5f, #feb47b)"));
        assert!(!is_purple_blue("linear-gradient(#7c3aed, white)"));
        assert!(!is_purple_blue("linear-gradient(#7c3aed)"));
        assert!(!is_purple_blue("linear-gradient(#0000ff, #0080ff)"));
        assert!(!is_purple_blue("linear-gradient(#808080, #7c3aed)"));
        assert!(!is_purple_blue("linear-gradient(var(--a), var(--b))"));
    }

    #[test]
    fn transparent_stops_do_not_count_as_colors() {
        assert!(!is_purple_blue("linear-gradient(#7c3aed, transparent)"));
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
