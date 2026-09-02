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
    &crate::rules::residue::RULE,          // SLOP011
    &crate::rules::citation::RULE,         // SLOP012
    &crate::rules::template::RULE,         // SLOP013
    &crate::rules::cliche::RULE,           // SLOP014
    &crate::rules::hedging::RULE,          // SLOP015
    &crate::rules::vocabulary::RULE,       // SLOP016
    &crate::rules::parallelism::RULE,      // SLOP017
    &crate::rules::emdash::RULE,           // SLOP018
    &crate::rules::boldface::RULE,         // SLOP019
    &crate::rules::smartquotes::RULE,      // SLOP020
    &crate::rules::heading_style::RULE,    // SLOP021
    &crate::rules::opener::RULE,           // SLOP022
    &crate::rules::contrast::RULE,         // SLOP023
    &crate::rules::puffery::RULE,          // SLOP024
    &crate::rules::weasel::RULE,           // SLOP025
    &crate::rules::colon_reveal::RULE,     // SLOP026
    &crate::rules::filler::RULE,           // SLOP027
    &crate::rules::weak_verb::RULE,        // SLOP028
    &crate::rules::recap::RULE,            // SLOP029
    &crate::rules::fragmentation::RULE,    // SLOP030
    &crate::rules::promo::RULE,            // SLOP031
    &crate::rules::hyphen::RULE,           // SLOP032
    &crate::rules::sentence_length::RULE,  // SLOP033
    &crate::rules::synonym_rotation::RULE, // SLOP034
    &crate::rules::outline_section::RULE,  // SLOP035
    &crate::rules::diff_anchor::RULE,      // SLOP036
    &crate::rules::reinvent::RULE,         // SLOP037
    &crate::rules::redundant_dep::RULE,    // SLOP038
    &crate::rules::wrapper::RULE,          // SLOP039
    &crate::rules::single_impl::RULE,      // SLOP040
    &crate::rules::uniformity::RULE,       // SLOP041
    &crate::rules::restate::RULE,          // SLOP042
    &crate::rules::comment_length::RULE,   // SLOP043
    &crate::rules::html_title::RULE,       // SLOP044
];
