use std::sync::LazyLock;

use regex::Regex;

use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{Lang, NatLang};
use crate::registry::RuleDef;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP009",
    name: "Placeholder / sample credential value",
    tier: Tier::A,
    // Every code lang's string literals, plus HTML attribute values (`ctx.strings` carries
    // `name="value"` there, see `ProseDoc::attr_values`).
    langs: &[
        Lang::Ts,
        Lang::Tsx,
        Lang::Python,
        Lang::Go,
        Lang::Rust,
        Lang::Html,
    ],
    natlangs: &[NatLang::En],
    default_on: true,
    path_gated: true,
    check,
};

const MESSAGE: &str = "hardcoded sample/credential value";

// This file's own pattern below spells out several of the trigger tokens it describes, so
// dogfooding stopslop against its own src/ would self-flag it.
static RE_CI: LazyLock<Regex> = LazyLock::new(|| {
    // ai-slop-ignore
    Regex::new(r"(?i)(?-u:\b)YOUR_[A-Z0-9_]+|<your[ -][^>]*>|example\.(com|org|net)|123[- ]?456[- ]?7890|John Doe|Jane Doe|foo@bar\.|user@example\.|change[_ ]?me").unwrap()
});
// The credential patterns are prefixes and character classes, never a literal sample value, so
// this one needs no suppression -- SLOP009 has nothing to match here.
static RE_CS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"sk-[A-Za-z0-9]{16,}|AKIA[0-9A-Z]{12,}|ghp_[A-Za-z0-9]{20,}|xox[baprs]-").unwrap()
});
/// HTML attributes: the four placeholder-image generators, and an `alt` whose whole value is a
/// generic word (AccessGuru and A11YN both report generic alt text as the recurring defect in
/// generated markup). `picsum.photos` serves real photographs for demos and is not a placeholder;
/// `alt=""` is the correct markup for a decorative image and never matches. Each string is one
/// whole `name="value"` attribute (`ProseDoc::attr_values`), so `^alt=` cannot hit `data-alt=`.
static RE_HTML: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)via\.placeholder\.com|placehold\.co|placekitten\.com|dummyimage\.com|^alt=["'](?:image|img|photo|picture|placeholder|image description|alt text|description)["']$"#).unwrap()
});

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for s in ctx.strings {
        if s.is_doc {
            continue;
        }
        if RE_CI.is_match(s.text) || RE_CS.is_match(s.text) || RE_HTML.is_match(s.text) {
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
    fn your_inside_a_filename_token_is_not_a_placeholder() {
        assert!(!RE_CI.is_match("/files/Leave_Your_Dog_at_Home_600x.png"));
        assert!(RE_CI.is_match("/files/YOUR_LOGO_HERE.png")); // ai-slop-ignore
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
    fn flags_placeholder_image_host_and_generic_alt() {
        assert!(RE_HTML.is_match("src=\"https://via.placeholder.com/150\"")); // ai-slop-ignore
        assert!(RE_HTML.is_match("src=\"https://placehold.co/600x400\"")); // ai-slop-ignore
        assert!(RE_HTML.is_match("alt=\"image\""));
        assert!(RE_HTML.is_match("alt='Image description'"));
        assert!(RE_HTML.is_match("ALT=\"IMAGE\""));
    }

    #[test]
    fn clean_decorative_descriptive_alt_and_demo_photo_host() {
        assert!(!RE_HTML.is_match("alt=\"\""));
        assert!(!RE_HTML.is_match("alt=\"image of the office lobby\""));
        assert!(!RE_HTML.is_match("src=\"https://picsum.photos/200\""));
        assert!(!RE_HTML.is_match("data-alt=\"image\""));
        assert!(!RE_HTML.is_match("title=\"image\""));
    }

    #[test]
    fn clean_specific_type_cast_like_string_not_flagged() {
        assert!(!RE_CI.is_match("hello world") && !RE_CS.is_match("hello world"));
    }
}
