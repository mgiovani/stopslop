use std::sync::LazyLock;

use regex::Regex;

use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP009",
    name: "Placeholder / sample credential value",
    tier: Tier::A,
    langs: &[Lang::Ts, Lang::Tsx, Lang::Python, Lang::Go, Lang::Rust],
    default_on: true,
    path_gated: true,
    check,
};

const MESSAGE: &str = "hardcoded sample/credential value";

// Two of this file's own string literals below contain the trigger tokens they describe,
// so dogfooding stopslop against its own src/ would self-flag them; `// ai-slop-ignore`
// (trailing, same line as the string) suppresses those per suppress.rs semantics.
static RE_CI: LazyLock<Regex> = LazyLock::new(|| {
    // ai-slop-ignore
    Regex::new(r"(?i)YOUR_[A-Z0-9_]+|<your[ -][^>]*>|example\.(com|org|net)|123[- ]?456[- ]?7890|John Doe|Jane Doe|foo@bar\.|user@example\.|change[_ ]?me").unwrap()
});
static RE_CS: LazyLock<Regex> = LazyLock::new(|| {
    // ai-slop-ignore
    Regex::new(r"sk-[A-Za-z0-9]{16,}|AKIA[0-9A-Z]{12,}|ghp_[A-Za-z0-9]{20,}|xox[baprs]-").unwrap()
});

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for s in ctx.strings {
        if s.is_doc {
            continue;
        }
        if RE_CI.is_match(s.text) || RE_CS.is_match(s.text) {
            out.push(Diagnostic::at(rule, ctx, s.line, s.col, MESSAGE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_your_api_key() {
        assert!(RE_CI.is_match("YOUR_API_KEY")); // ai-slop-ignore
    }

    #[test]
    fn flags_example_domain() {
        assert!(RE_CI.is_match("https://example.com/api")); // ai-slop-ignore
    }

    #[test]
    fn flags_john_doe() {
        assert!(RE_CI.is_match("John Doe")); // ai-slop-ignore
    }

    #[test]
    fn flags_changeme() {
        assert!(RE_CI.is_match("change_me")); // ai-slop-ignore
    }

    #[test]
    fn flags_stripe_secret_shape() {
        assert!(RE_CS.is_match("sk-abcdefghijklmnop1234")); // ai-slop-ignore
    }

    #[test]
    fn flags_aws_key_shape() {
        assert!(RE_CS.is_match("AKIAIOSFODNN7EXAMPLE")); // ai-slop-ignore
    }

    #[test]
    fn flags_github_token_shape() {
        assert!(RE_CS.is_match("ghp_abcdefghijklmnopqrstuvwxyz012345")); // ai-slop-ignore
    }

    #[test]
    fn clean_production_url_not_flagged() {
        assert!(
            !RE_CI.is_match("https://api.production.com")
                && !RE_CS.is_match("https://api.production.com")
        );
    }

    #[test]
    fn clean_specific_type_cast_like_string_not_flagged() {
        assert!(!RE_CI.is_match("hello world") && !RE_CS.is_match("hello world"));
    }
}
