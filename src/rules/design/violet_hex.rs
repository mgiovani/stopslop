use super::{is_gradient, unprefixed};
use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{ALL_NATLANGS, STYLE_LANGS};
use crate::registry::RuleDef;
use crate::style::utility;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP054",
    name: "Hand-copied Tailwind violet hex",
    tier: Tier::C,
    langs: STYLE_LANGS,
    natlangs: ALL_NATLANGS,
    default_on: false,
    path_gated: false,
    check,
};

static HEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i-u)#(7c3aed|8b5cf6|a855f7|9333ea|7e22ce|6d28d9|6366f1|764ba2|667eea)(?-u:\b)")
        .unwrap()
});

const FIX: &str = "use a design token";

/// A custom property (`--brand: #6366f1`) is where the token the fix asks for gets defined, so
/// its value is never flagged.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(style) = ctx.style else { return };
    let declared = style
        .decls
        .iter()
        .filter(|d| !is_gradient(d.value) && !unprefixed(&d.property).starts_with("--"))
        .map(|d| (d.value, d.value_start));
    let classes = style
        .class_lists
        .iter()
        .flatten()
        .filter(|t| !is_gradient_stop(utility(t.text)))
        .map(|t| (t.text, t.start));
    for (text, start) in declared.chain(classes).filter(|(t, _)| t.contains('#')) {
        for m in HEX.find_iter(text) {
            let (line, col) = style.line_col(start + m.start());
            out.push(Diagnostic::at_fix(
                rule,
                ctx,
                line,
                col,
                format!(
                    "{} is a Tailwind violet or indigo shade copied by hand",
                    m.as_str()
                ),
                FIX,
            ));
        }
    }
}

fn is_gradient_stop(u: &str) -> bool {
    ["from-", "via-", "to-"].iter().any(|p| u.starts_with(p))
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
    fn panel_matches_whole_hex_tokens_only() {
        assert!(HEX.is_match("color: #7C3AED"));
        assert!(HEX.is_match("#6366f1;"));
        assert!(!HEX.is_match("#7c3aed00"));
        assert!(!HEX.is_match("#7c3aeda"));
        assert!(!HEX.is_match("#123456"));
    }

    #[test]
    fn flags_a_declared_hex_with_its_position() {
        let d = diagnostics_for(Lang::Css, ".a {\n  color: #7C3AED;\n}\n");
        assert_eq!((d.len(), d[0].line, d[0].col), (1, 2, 10));
        assert!(d[0].message.starts_with("#7C3AED"));
    }

    #[test]
    fn skips_custom_properties_of_every_kind() {
        let src = ":root {\n  --tw-ring-color: #6366f1;\n  --brand: #6366f1;\n}\n";
        assert!(diagnostics_for(Lang::Css, src).is_empty());
        let tsx = "const a = <p style={{ '--brand': '#6366f1' }} />;\n";
        assert!(diagnostics_for(Lang::Tsx, tsx).is_empty());
    }

    #[test]
    fn leaves_gradient_stops_to_the_gradient_rule() {
        let css = ".a { background: linear-gradient(#667eea, #764ba2); }";
        assert!(diagnostics_for(Lang::Css, css).is_empty());
        let html = "<i class=\"from-[#7c3aed] to-[#6366f1] bg-[#8b5cf6]\">x</i>\n";
        let d = diagnostics_for(Lang::Html, html);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.starts_with("#8b5cf6"));
    }

    #[test]
    fn flags_inline_style_and_jsx_values() {
        let html = "<a style=\"color: #7c3aed\">x</a>\n";
        assert_eq!(diagnostics_for(Lang::Html, html).len(), 1);
        let tsx = "const a = <p style={{ color: `#7c3aed` }} />;\n";
        let d = diagnostics_for(Lang::Tsx, tsx);
        assert_eq!((d.len(), d[0].col), (1, 31));
    }

    #[test]
    fn ignores_look_alikes_and_empty_input() {
        assert!(diagnostics_for(Lang::Css, ".a { color: #7c3aed1}").is_empty());
        assert!(diagnostics_for(Lang::Css, "").is_empty());
    }
}
