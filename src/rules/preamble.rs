use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose_words::REASONING_CHAIN_FRAGMENT;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP002",
    name: "Chat preamble leaked into code",
    tier: Tier::A,
    langs: &[Lang::Ts, Lang::Tsx, Lang::Python, Lang::Go, Lang::Rust],
    default_on: true,
    path_gated: false,
    check,
};

// Shares REASONING_CHAIN_FRAGMENT with rules::residue (SLOP011) so a new phrase needs adding
// once. "step 1:" stays local: real precision/recall tradeoff, since ordinary numbered-step
// comments look similar and each consumer resolves the position differently.
static RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?im)^\s*(?://|#|\*+)\s*(certainly[!,]|sure[!,]|here'?s the (updated|revised|complete|new|fixed)|below is the (updated|complete|full)|as an ai\b|i hope this helps|step 1:|{REASONING_CHAIN_FRAGMENT})",
    ))
    .unwrap()
});

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for c in ctx.comments {
        if c.is_doc {
            continue;
        }
        if RE.is_match(c.text) {
            out.push(Diagnostic::at(
                rule,
                ctx,
                c.line,
                c.col,
                "chat-preamble text leaked into a source comment",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_preamble() {
        assert!(RE.is_match("// Certainly! Here's the fix"));
        assert!(RE.is_match("# Sure, below is the complete solution:"));
        assert!(RE.is_match("// As an AI language model, here's the solution:"));
    }

    #[test]
    fn does_not_match_explanation() {
        assert!(!RE.is_match("# Here's a breakdown of the parser logic:"));
    }

    #[test]
    fn matches_reasoning_chain_leakage() {
        assert!(RE.is_match("// Let's think about this differently"));
        assert!(RE.is_match("// Step 1: parse the config file"));
        assert!(RE.is_match("# Breaking this down into smaller pieces"));
    }

    #[test]
    fn does_not_match_ordinary_numbered_comment() {
        assert!(!RE.is_match("// The first step here validates the config."));
    }
}
