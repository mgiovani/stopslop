use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{Lang, NatLang};

/// Uniform check fn. Text rules iterate ctx.comments/ctx.strings; AST rules query ctx.nodes().
pub type CheckFn = fn(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>);

pub struct RuleDef {
    pub code: &'static str, // "SLOP001"
    pub name: &'static str,
    pub tier: Tier,
    pub langs: &'static [Lang], // rule runs only for these langs
    /// Natural languages this rule's lexicon is validated on, proven by a fixture. A rule with
    /// no natural-language lexicon (AST shape, punctuation, statistics) declares every language.
    pub natlangs: &'static [NatLang],
    pub default_on: bool, // three-state w/ tier: A+on blocks CI, B+on warns without
    // blocking, B+off is opt-in only (see Tier's doc comment)
    pub path_gated: bool, // honor is_test_path exemption (SLOP005/6/8/9/10)
    pub check: CheckFn,
}

// Each rule module exposes `pub static RULE: RuleDef`.
#[allow(clippy::redundant_static_lifetimes)] // frozen contract signature (PLAN §3), keep verbatim
pub static RULES: &[&'static RuleDef] = &[
    &crate::rules::artifact::elision::RULE,
    &crate::rules::artifact::preamble::RULE,
    &crate::rules::artifact::fence::RULE,
    &crate::rules::artifact::attribution::RULE,
    &crate::rules::structure::empty_catch::RULE,
    &crate::rules::structure::py_except::RULE,
    &crate::rules::structure::type_escape::RULE,
    &crate::rules::structure::stub::RULE,
    &crate::rules::structure::placeholder::RULE,
    &crate::rules::structure::imports::RULE,
    &crate::rules::artifact::residue::RULE,          // SLOP011
    &crate::rules::artifact::citation::RULE,         // SLOP012
    &crate::rules::artifact::template::RULE,         // SLOP013
    &crate::rules::rhetoric::cliche::RULE,           // SLOP014
    &crate::rules::verbosity::hedging::RULE,         // SLOP015
    &crate::rules::verbosity::vocabulary::RULE,      // SLOP016
    &crate::rules::rhetoric::parallelism::RULE,      // SLOP017
    &crate::rules::format::emdash::RULE,             // SLOP018
    &crate::rules::format::boldface::RULE,           // SLOP019
    &crate::rules::format::smartquotes::RULE,        // SLOP020
    &crate::rules::format::heading_style::RULE,      // SLOP021
    &crate::rules::rhetoric::opener::RULE,           // SLOP022
    &crate::rules::rhetoric::contrast::RULE,         // SLOP023
    &crate::rules::rhetoric::puffery::RULE,          // SLOP024
    &crate::rules::sourcing::weasel::RULE,           // SLOP025
    &crate::rules::rhetoric::colon_reveal::RULE,     // SLOP026
    &crate::rules::verbosity::filler::RULE,          // SLOP027
    &crate::rules::verbosity::weak_verb::RULE,       // SLOP028
    &crate::rules::rhetoric::recap::RULE,            // SLOP029
    &crate::rules::rhetoric::fragmentation::RULE,    // SLOP030
    &crate::rules::rhetoric::promo::RULE,            // SLOP031
    &crate::rules::verbosity::hyphen::RULE,          // SLOP032
    &crate::rules::verbosity::sentence_length::RULE, // SLOP033
    &crate::rules::verbosity::synonym_rotation::RULE, // SLOP034
    &crate::rules::rhetoric::outline_section::RULE,  // SLOP035
    &crate::rules::rhetoric::diff_anchor::RULE,      // SLOP036
    &crate::rules::stdlib::reinvent::RULE,           // SLOP037
    &crate::rules::stdlib::redundant_dep::RULE,      // SLOP038
    &crate::rules::structure::wrapper::RULE,         // SLOP039
    &crate::rules::structure::single_impl::RULE,     // SLOP040
    &crate::rules::verbosity::uniformity::RULE,      // SLOP041
    &crate::rules::verbosity::restate::RULE,         // SLOP042
    &crate::rules::verbosity::comment_length::RULE,  // SLOP043
    &crate::rules::artifact::html_title::RULE,       // SLOP044
];
