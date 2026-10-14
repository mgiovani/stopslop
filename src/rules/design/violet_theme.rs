use crate::color::{colors_in, hsl, parse_hsl_triplet};
use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{Lang, ALL_NATLANGS};
use crate::registry::RuleDef;
use std::ops::RangeInclusive;

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
const HUE_BAND: RangeInclusive<f32> = 240.0..=300.0;
const MIN_SATURATION: f32 = 0.35;
const FIX: &str = "set --primary to the brand color";

/// `--color-primary` is the Tailwind v4 `@theme` spelling of shadcn's `--primary`. The selector is
/// not checked: these token names are specific enough to shadcn that the block is a theme.
fn is_theme_token(property: &str) -> bool {
    let name = property.strip_prefix("--").unwrap_or("");
    TOKENS.contains(&name.strip_prefix("color-").unwrap_or(name))
}

fn is_violet(value: &str) -> bool {
    let color = parse_hsl_triplet(value).or_else(|| colors_in(value).first().map(|&(_, _, c)| c));
    color.is_some_and(|c| {
        let (h, s, _) = hsl(c);
        s >= MIN_SATURATION && HUE_BAND.contains(&h)
    })
}

/// One finding per theme block, anchored on its first violet token.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(style) = ctx.style else { return };
    for block in style.blocks() {
        let first = block
            .iter()
            .find(|d| is_theme_token(&d.property) && is_violet(d.value));
        if let Some(d) = first {
            let (line, col) = style.line_col(d.start);
            out.push(Diagnostic::at_fix(
                rule,
                ctx,
                line,
                col,
                "shadcn violet theme preset, the default generated-UI palette",
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
    fn flags_the_v3_hsl_triplet_violet_theme_once_per_block() {
        let src = ":root {\n  --background: 0 0% 100%;\n  --primary: 262.1 83.3% 57.8%;\n  --ring: 262.1 83.3% 57.8%;\n}\n.dark {\n  --primary: 263.4 70% 50.4%;\n}\n";
        let d = diagnostics_for(Lang::Css, src);
        assert_eq!(d.len(), 2);
        assert_eq!((d[0].line, d[0].col), (3, 3));
        assert_eq!(d[1].line, 7);
    }

    #[test]
    fn flags_v4_oklch_and_theme_color_names() {
        let src = "@theme {\n  --color-primary: oklch(0.541 0.281 293.009);\n}\n";
        assert_eq!(diagnostics_for(Lang::Css, src).len(), 1);
        let src = ":root { --sidebar-primary: oklch(0.606 0.25 292.717); }";
        assert_eq!(diagnostics_for(Lang::Css, src).len(), 1);
    }

    #[test]
    fn flags_a_style_element_in_html() {
        let html = "<style>:root { --chart-1: #7c3aed; }</style>\n";
        assert_eq!(diagnostics_for(Lang::Html, html).len(), 1);
    }

    #[test]
    fn quiet_for_zinc_blue_and_neutral_themes() {
        let zinc = ":root {\n  --primary: 240 5.9% 10%;\n  --ring: 240 5.9% 10%;\n  --accent: 240 4.8% 95.9%;\n}\n";
        assert!(diagnostics_for(Lang::Css, zinc).is_empty());
        let blue = ":root { --primary: 221.2 83.2% 53.3%; --ring: oklch(0.623 0.214 259.815); }";
        assert!(diagnostics_for(Lang::Css, blue).is_empty());
    }

    #[test]
    fn ignores_other_tokens_plain_properties_and_empty_input() {
        let src = ":root { --brand: 262.1 83.3% 57.8%; --primary-foreground: 262.1 83.3% 57.8%; }\n.a { color: #7c3aed; accent: #7c3aed; }\n";
        assert!(diagnostics_for(Lang::Css, src).is_empty());
        assert!(diagnostics_for(Lang::Css, "").is_empty());
    }
}
