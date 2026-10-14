use super::{is_gradient, report, STOP_PREFIXES};
use crate::color::{colors_in, hsl, Rgba, MIN_SATURATION};
use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{ALL_NATLANGS, STYLE_LANGS};
use crate::registry::RuleDef;
use crate::style::ClassToken;
use std::ops::{Range, RangeInclusive};

pub static RULE: RuleDef = RuleDef {
    code: "SLOP053",
    name: "Purple-blue gradient",
    tier: Tier::B,
    langs: STYLE_LANGS,
    natlangs: ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

const MIN_ALPHA: f32 = 0.5;
const HUE_BAND: RangeInclusive<f32> = 225.0..=330.0;
const BLUE_SIDE: Range<f32> = 225.0..255.0;
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
        .filter(|d| is_gradient(d.value) && is_purple_blue(&colors_in(d.value)))
    {
        let message = format!(
            "{}: {} runs between saturated purple and blue stops",
            d.property, d.value
        );
        report(rule, ctx, style, d.start, message, FIX, out);
    }
    for list in &style.class_lists {
        if let Some((from, rest)) = palette_pair(list) {
            let message = format!(
                "{} into {} runs between purple, violet or indigo and a neighboring hue",
                from.text, rest.text
            );
            report(rule, ctx, style, from.start, message, FIX, out);
        }
        if let Some(first) = arbitrary_gradient(list) {
            let message = format!(
                "{} starts a gradient between saturated purple and blue stops",
                first.text
            );
            report(rule, ctx, style, first.start, message, FIX, out);
        }
    }
}

/// A gradient spans purple and blue when it has two or more opaque stops, every one saturated and
/// inside the blue-to-pink band, with one on the blue side and one in the purple core. Stops that
/// sit on the purple side only, or that are mostly transparent, are a tint, not the stock gradient.
fn is_purple_blue(colors: &[Rgba]) -> bool {
    let (mut stops, mut purple, mut blue) = (0, false, false);
    for c in colors.iter().filter(|c| c.a >= MIN_ALPHA) {
        let (h, s, _) = hsl(*c);
        if s < MIN_SATURATION || !HUE_BAND.contains(&h) {
            return false;
        }
        stops += 1;
        purple |= PURPLE_CORE.contains(&h);
        blue |= BLUE_SIDE.contains(&h);
    }
    stops >= 2 && purple && blue
}

/// The `from-` stop and the `via-`/`to-` stop it pairs with. Stops only pair inside one variant
/// (`dark:from-x` never pairs with a bare `to-y`), and a stop never pairs with its own hue name.
fn palette_pair<'l, 'a>(
    list: &'l [ClassToken<'a>],
) -> Option<(&'l ClassToken<'a>, &'l ClassToken<'a>)> {
    list.iter().find_map(|from| {
        let name = palette(from.utility, STOP_PREFIXES[0], PALETTE_FROM)?;
        let rest = list.iter().find(|t| {
            t.variant == from.variant
                && STOP_PREFIXES[1..]
                    .iter()
                    .any(|p| palette(t.utility, p, PALETTE_REST).is_some_and(|n| n != name))
        })?;
        Some((from, rest))
    })
}

/// The first bracketed stop of a list when the bracketed stops of its variant span purple and blue.
fn arbitrary_gradient<'l, 'a>(list: &'l [ClassToken<'a>]) -> Option<&'l ClassToken<'a>> {
    let mut stops = list.iter().filter_map(|t| Some((t, arbitrary_stop(t)?)));
    let (first, color) = stops.next()?;
    let colors: Vec<Rgba> = std::iter::once(color)
        .chain(
            stops
                .filter(|(t, _)| t.variant == first.variant)
                .map(|(_, c)| c),
        )
        .collect();
    is_purple_blue(&colors).then_some(first)
}

/// The color of a bracketed gradient stop such as `from-[#7c3aed]`, `to-[rgb(99,102,241)]`,
/// `from-[color:oklch(0.6_0.25_292)]` or `from-[#7c3aed]/50`. Tailwind writes a space as `_`.
fn arbitrary_stop(token: &ClassToken) -> Option<Rgba> {
    let rest = STOP_PREFIXES
        .iter()
        .find_map(|p| token.utility.strip_prefix(p))?;
    let inner = rest
        .strip_prefix('[')?
        .rsplit_once(']')?
        .0
        .replace('_', " ");
    let inner = inner.strip_prefix("color:").unwrap_or(&inner);
    colors_in(inner).first().copied()
}

/// The hue name of `<prefix><name>-<shade>` (optionally `/<opacity>`) when it is in `names`.
fn palette<'u>(utility: &'u str, prefix: &str, names: &[&str]) -> Option<&'u str> {
    let rest = utility.strip_prefix(prefix)?;
    let rest = rest.split('/').next().unwrap_or(rest);
    let (name, shade) = rest.rsplit_once('-')?;
    (names.contains(&name) && !shade.is_empty() && shade.bytes().all(|b| b.is_ascii_digit()))
        .then_some(name)
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
        is_purple_blue(&colors_in(value))
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
    fn a_gradient_must_span_purple_and_blue() {
        assert!(!purple_blue("linear-gradient(#8b5cf6, #a855f7)"));
        assert!(!purple_blue("linear-gradient(#7c3aed, #7c3aed)"));
        assert!(!purple_blue("linear-gradient(#6366f1, #3b82f6)"));
        assert!(!purple_blue(
            "linear-gradient(rgba(124, 58, 237, .05), rgba(124, 58, 237, .1))"
        ));
        assert!(!purple_blue(
            "linear-gradient(rgba(99, 102, 241, .1), rgba(139, 92, 246, .9))"
        ));
        assert!(purple_blue(
            "linear-gradient(rgba(99, 102, 241, .6), rgba(139, 92, 246, .9))"
        ));
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
            html("<i class=\"bg-gradient-to-r hover:from-purple-500 hover:to-blue-500\">x</i>\n")
                .len(),
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
    fn palette_stops_of_one_hue_or_of_different_variants_stay_quiet() {
        let quiet = [
            "from-purple-500 to-purple-700",
            "from-violet-500 via-violet-600 to-violet-700",
            "dark:from-purple-500 to-blue-500",
            "from-purple-500 dark:to-blue-500",
            "from-blue-500 to-purple-500",
            "from-pink-500 to-fuchsia-500",
            "from-purple-500 via-purple-600",
            "from-[#7c3aed] to-purple-500",
            "from-[#7c3aed] to-[transparent]",
            "from-[#8b5cf6] to-[#a855f7]",
            "from-[rgba(99,102,241,0.1)] to-[rgba(139,92,246,0.1)]",
        ];
        for class in quiet {
            let src = format!("<i class=\"{class}\">x</i>\n");
            assert!(diagnostics_for(Lang::Html, &src).is_empty(), "{class}");
        }
        let loud = "from-purple-500 to-blue-500 dark:from-purple-400 dark:to-purple-600";
        let src = format!("<i class=\"{loud}\">x</i>\n");
        assert_eq!(diagnostics_for(Lang::Html, &src).len(), 1);
    }

    #[test]
    fn messages_name_the_matched_utilities_and_property() {
        let d = diagnostics_for(
            Lang::Html,
            "<i class=\"from-purple-500 to-blue-500\">x</i>\n",
        );
        assert!(d[0].message.starts_with("from-purple-500 into to-blue-500"));
        let d = diagnostics_for(
            Lang::Css,
            ".a { background-image: linear-gradient(#667eea, #764ba2) }",
        );
        assert!(d[0].message.starts_with("background-image:"));
    }

    #[test]
    fn check_reads_jsx_style_objects_and_class_names() {
        let src = "const a = <div className=\"from-indigo-500 via-violet-500 to-pink-500\" style={{ background: 'linear-gradient(#667eea, #764ba2)' }} />;\n";
        assert_eq!(diagnostics_for(Lang::Tsx, src).len(), 2);
    }

    #[test]
    fn palette_reads_name_shade_and_opacity() {
        assert_eq!(
            palette("from-purple-500", "from-", &["purple"]),
            Some("purple")
        );
        assert_eq!(
            palette("to-indigo-600/50", "to-", &["indigo"]),
            Some("indigo")
        );
        assert_eq!(palette("from-purple", "from-", &["purple"]), None);
        assert_eq!(palette("from-red-500", "from-", &["purple"]), None);
        assert_eq!(palette("via-purple-500", "from-", &["purple"]), None);
    }

    #[test]
    fn reads_v4_oklch_stops_in_css_and_brackets() {
        let violet_to_indigo = ".a { background: linear-gradient(in oklch, oklch(0.606 0.25 292.717), oklch(0.585 0.233 277.117)); }";
        assert_eq!(diagnostics_for(Lang::Css, violet_to_indigo).len(), 1);
        let warm = ".a { background: linear-gradient(oklch(0.7 0.2 40), oklch(0.8 0.15 80)); }";
        assert!(diagnostics_for(Lang::Css, warm).is_empty());
        let html = "<i class=\"bg-linear-to-r from-[oklch(0.606_0.25_292.717)] to-[oklch(0.585_0.233_277.117)]\">x</i>\n";
        assert_eq!(diagnostics_for(Lang::Html, html).len(), 1);
        let slashed =
            "<i class=\"from-[color:oklch(0.606_0.25_292.717)]/50 to-[#6366f1]/40\">x</i>\n";
        assert_eq!(diagnostics_for(Lang::Html, slashed).len(), 1);
        let comma = "<i class=\"from-[oklch(0.606,0.25,292.717)] to-[#6366f1]\">x</i>\n";
        assert_eq!(diagnostics_for(Lang::Html, comma).len(), 1);
    }

    #[test]
    fn reads_v3_and_v4_stop_spellings() {
        for class in [
            "!from-violet-500 to-blue-500",
            "from-violet-500 via-purple-500 to-indigo-600",
            "from-violet-500! to-blue-500!",
            "tw:from-violet-500 tw:to-blue-500",
            "bg-linear-to-r from-purple-500 via-pink-500",
        ] {
            let src = format!("<i class=\"{class}\">x</i>\n");
            assert_eq!(diagnostics_for(Lang::Html, &src).len(), 1, "{class}");
        }
        assert!(diagnostics_for(
            Lang::Html,
            "<i class=\"from-(--brand) to-blue-500\">x</i>\n"
        )
        .is_empty());
        assert!(diagnostics_for(
            Lang::Html,
            "<i class=\"from-[#7c3aed] to-(--brand)\">x</i>\n"
        )
        .is_empty());
    }
}
