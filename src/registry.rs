use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;

/// Uniform check fn. Text rules iterate ctx.comments/ctx.strings; AST rules call ctx.walk().
pub type CheckFn = fn(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>);

pub struct RuleDef {
    pub code: &'static str, // "SLOP001"
    pub name: &'static str,
    pub tier: Tier,
    pub langs: &'static [Lang], // rule runs only for these langs
    pub default_on: bool,       // false only for SLOP010
    pub path_gated: bool,       // honor is_test_path exemption (SLOP005/6/8/9/10)
    pub check: CheckFn,
}

// Each rule module exposes `pub static RULE: RuleDef`.
#[allow(clippy::redundant_static_lifetimes)] // frozen contract signature (PLAN §3), keep verbatim
pub static RULES: &[&'static RuleDef] = &[
    &crate::rules::elision::RULE,
    &crate::rules::preamble::RULE,
    &crate::rules::fence::RULE,
    &crate::rules::attribution::RULE,
    &crate::rules::empty_catch::RULE,
    &crate::rules::py_except::RULE,
    &crate::rules::type_escape::RULE,
    &crate::rules::stub::RULE,
    &crate::rules::placeholder::RULE,
    &crate::rules::imports::RULE,
];
