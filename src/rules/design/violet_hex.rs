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
    Regex::new(r"(?i)#(7c3aed|8b5cf6|a855f7|9333ea|7e22ce|6d28d9|6366f1|764ba2|667eea)(?-u:\b)")
        .unwrap()
});

const FIX: &str = "use a design token";

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(style) = ctx.style else { return };
    let declared = style
        .decls
        .iter()
        .filter(|d| !is_gradient(d.value) && !unprefixed(&d.property).starts_with("--tw-"))
        .map(|d| (d.value, d.value_start));
    let classes = style
        .class_lists
        .iter()
        .flat_map(|l| &l.tokens)
        .filter(|t| !is_gradient_stop(utility(t.text)))
        .map(|t| (t.text, t.start));
    for (text, start) in declared.chain(classes) {
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
    use super::*;

    #[test]
    fn panel_matches_whole_hex_tokens_only() {
        assert!(HEX.is_match("color: #7C3AED"));
        assert!(HEX.is_match("#6366f1;"));
        assert!(!HEX.is_match("#7c3aed00"));
        assert!(!HEX.is_match("#7c3aeda"));
        assert!(!HEX.is_match("#123456"));
    }

    #[test]
    fn gradient_stops_are_left_to_the_gradient_rule() {
        assert!(is_gradient_stop("from-[#7c3aed]"));
        assert!(!is_gradient_stop("bg-[#7c3aed]"));
    }
}
