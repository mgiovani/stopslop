use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP042",
    name: "Comment that restates the code",
    tier: Tier::B,
    langs: &[Lang::Ts, Lang::Tsx, Lang::Python, Lang::Go, Lang::Rust],
    default_on: true,
    path_gated: false,
    check,
};

fn check(_rule: &'static RuleDef, _ctx: &LintContext, _out: &mut Vec<Diagnostic>) {}
