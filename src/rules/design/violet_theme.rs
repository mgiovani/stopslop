use super::report;
use crate::color::{colors_in, is_violet, parse_hsl_triplet};
use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{Lang, ALL_NATLANGS};
use crate::registry::RuleDef;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP056",
    name: "Violet shadcn theme preset",
    tier: Tier::B,
    langs: &[Lang::Css, Lang::Html],
    natlangs: ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

const TOKENS: &[&str] = &["primary", "ring", "accent", "sidebar-primary", "chart-1"];
/// Names a generic design system rarely defines together; two of them in one block mark it as a
/// shadcn theme rather than a site that happens to call its brand color `--primary`.
const MARKERS: &[&str] = &[
    "primary-foreground",
    "secondary-foreground",
    "accent-foreground",
    "card-foreground",
    "popover",
    "popover-foreground",
    "muted-foreground",
    "destructive",
    "sidebar",
];
const MIN_MARKERS: usize = 2;
const FIX: &str = "set --primary to the brand color";

/// `--color-primary` is the Tailwind v4 `@theme` spelling of shadcn's `--primary`.
fn token_name(property: &str) -> &str {
    let name = property.strip_prefix("--").unwrap_or("");
    name.strip_prefix("color-").unwrap_or(name)
}

fn is_violet_value(value: &str) -> bool {
    parse_hsl_triplet(value)
        .or_else(|| colors_in(value).first().copied())
        .is_some_and(is_violet)
}

/// One finding per theme block, anchored on its first violet token.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(style) = ctx.style else { return };
    for block in style.blocks() {
        let markers = block
            .iter()
            .filter(|d| MARKERS.contains(&token_name(&d.property)))
            .count();
        if markers < MIN_MARKERS {
            continue;
        }
        let first = block
            .iter()
            .find(|d| TOKENS.contains(&token_name(&d.property)) && is_violet_value(d.value));
        if let Some(d) = first {
            let message = format!(
                "{}: {} is the shadcn violet theme preset, the default generated-UI palette",
                d.property, d.value
            );
            report(rule, ctx, style, d.start, message, FIX, out);
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

    fn theme(tokens: &str) -> String {
        format!(
            ":root {{\n  --card-foreground: 0 0% 100%;\n  --primary-foreground: 0 0% 98%;\n  {tokens}\n}}\n"
        )
    }

    #[test]
    fn flags_the_v3_hsl_triplet_violet_theme_once_per_block() {
        let src = ":root {\n  --card-foreground: 0 0% 100%;\n  --primary-foreground: 0 0% 98%;\n  --primary: 262.1 83.3% 57.8%;\n  --ring: 262.1 83.3% 57.8%;\n}\n.dark {\n  --card-foreground: 224 71% 4%;\n  --popover-foreground: 0.5rem;\n  --primary: 263.4 70% 50.4%;\n}\n";
        let d = diagnostics_for(Lang::Css, src);
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].line, d[0].col), (4, 3));
        assert_eq!(d[1].line, 10);
        assert_eq!(
            d[0].fix.as_deref(),
            Some("set --primary to the brand color")
        );
        assert!(d[0]
            .message
            .starts_with("--primary: 262.1 83.3% 57.8% is the shadcn"));
    }

    #[test]
    fn reads_every_token_and_the_theme_color_spelling() {
        for token in [
            "--primary",
            "--ring",
            "--accent",
            "--sidebar-primary",
            "--chart-1",
            "--color-ring",
            "--color-primary",
        ] {
            let src = theme(&format!("{token}: oklch(0.541 0.281 293.009);"));
            assert_eq!(diagnostics_for(Lang::Css, &src).len(), 1, "{token}");
        }
    }

    #[test]
    fn reads_every_value_notation_and_an_alpha_tail() {
        for value in [
            "#7c3aed",
            "rgb(124 58 237)",
            "hsl(262, 83%, 58%)",
            "hsl(262 83 58)",
            "262.1 83.3% 57.8%",
            "262.1 83.3% 57.8% / 0.5",
            "oklch(0.606 0.25 292.717)",
            "oklab(0.55 0.1 -0.2)",
        ] {
            let src = theme(&format!("--primary: {value};"));
            assert_eq!(diagnostics_for(Lang::Css, &src).len(), 1, "{value}");
        }
    }

    #[test]
    fn a_turn_hue_outside_the_violet_band_stays_quiet() {
        let src = theme("--primary: oklch(0.7turn 0.2 none);");
        assert!(diagnostics_for(Lang::Css, &src).is_empty());
    }

    #[test]
    fn flags_a_style_element_in_html() {
        let html =
            "<style>:root { --card-foreground: 0 0% 100%; --popover: 0 0% 9%; --chart-1: #7c3aed; }</style>\n";
        assert_eq!(diagnostics_for(Lang::Html, html).len(), 1);
    }

    #[test]
    fn a_lone_brand_token_without_shadcn_context_stays_quiet() {
        assert!(diagnostics_for(Lang::Css, ":root { --primary: #9146ff; }").is_empty());
        assert!(diagnostics_for(
            Lang::Css,
            ":root { --bs-primary: #6f42c1; --bs-body-bg: #fff; }"
        )
        .is_empty());
        assert!(
            diagnostics_for(Lang::Css, ":root { --accent: #7c3aed; --radius: 4px; }").is_empty()
        );
        assert!(
            diagnostics_for(Lang::Css, ":root { --primary: #9146ff; --card: #fff; }").is_empty()
        );
        let generic = ":root { --card: #fff; --radius: 4px; --primary: #7c3aed; }";
        assert!(diagnostics_for(Lang::Css, generic).is_empty());
    }

    #[test]
    fn quiet_for_zinc_blue_fuchsia_and_neutral_themes() {
        let quiet = [
            "--primary: 240 5.9% 10%;",
            "--primary: 221.2 83.2% 53.3%;",
            "--ring: oklch(0.623 0.214 259.815);",
            "--primary: #d946ef;",
            "--primary: magenta;",
            "--primary: #0000ff;",
            "--primary: #6366f1;",
            "--primary: oklch(0.585 0.233 277.117);",
            "--accent: 262 10% 50%;",
            "--primary: oklch(foo);",
            "--primary: var(--brand);",
        ];
        for tokens in quiet {
            assert!(
                diagnostics_for(Lang::Css, &theme(tokens)).is_empty(),
                "{tokens}"
            );
        }
    }

    #[test]
    fn the_violet_band_edges_are_pinned() {
        let hits = [
            "--primary: 249 80% 50%;",
            "--primary: 283 80% 50%;",
            "--primary: 258 37% 50%;",
        ];
        let misses = [
            "--primary: 247 80% 50%;",
            "--primary: 288 80% 50%;",
            "--primary: 262 32% 50%;",
        ];
        for tokens in hits {
            assert_eq!(
                diagnostics_for(Lang::Css, &theme(tokens)).len(),
                1,
                "{tokens}"
            );
        }
        for tokens in misses {
            assert!(
                diagnostics_for(Lang::Css, &theme(tokens)).is_empty(),
                "{tokens}"
            );
        }
    }

    #[test]
    fn ignores_other_tokens_plain_properties_and_empty_input() {
        let src = theme("--brand: 262.1 83.3% 57.8%; --primary-foreground: 262.1 83.3% 57.8%;");
        assert!(diagnostics_for(Lang::Css, &src).is_empty());
        assert!(diagnostics_for(Lang::Css, ".a { color: #7c3aed; accent: #7c3aed; }").is_empty());
        assert!(diagnostics_for(Lang::Css, "").is_empty());
    }
}
