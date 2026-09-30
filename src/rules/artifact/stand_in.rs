use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, CODE_LANGS};
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP050",
    name: "Comment narrates a stand-in",
    tier: Tier::B,
    langs: CODE_LANGS,
    natlangs: &[NatLang::En],
    default_on: true,
    path_gated: false,
    check,
};

// The bare `here` and `logic` boundaries are load-bearing: without them the panel matches
// inside "thereby" and "logician".
static RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(would go (?-u:\b)here(?-u:\b)|(logic|code|implementation) goes (?-u:\b)here(?-u:\b)|replace with (real|actual|your) |in a real(-world)? (scenario|application|world|system)|for demonstration( purposes)?|simulated? (data|response|delay|the (data|response|process))|for illustration|(this is a )?placeholder (for the (actual|real)|implementation|function|(?-u:\b)logic(?-u:\b))|implement (the|this|your) \w+( \w+){0,4} ((?-u:\b)logic(?-u:\b)|(?-u:\b)here(?-u:\b))|your (code|(?-u:\b)logic(?-u:\b)|implementation) (?-u:\b)here(?-u:\b))",
    )
    .unwrap()
});

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for c in ctx.comments {
        if !c.is_doc && RE.is_match(c.text) {
            out.push(Diagnostic::at(
                rule,
                ctx,
                c.line,
                c.col,
                "comment says the code beneath it is a placeholder, simulation or demo",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use tree_sitter::Parser;

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        let mut p = Parser::new();
        p.set_language(&crate::lang::ts_language(Lang::Ts)).unwrap();
        let tree = p.parse(src, None).unwrap();
        let (comments, strings, index) = crate::context::extract(&tree, src, Lang::Ts);
        let ctx = LintContext {
            display_path: "test.ts".to_string(),
            source: src,
            index: Some(&index),
            lang: Lang::Ts,
            comments: &comments,
            strings: &strings,
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: None,
            image: None,
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn flags_stand_in_narration() {
        for s in [
            "// Actual logic would go here\nfunction f() {}\n",
            "// In a real-world scenario this would call the API\nfunction f() {}\n",
            "// Simulate the response\nfunction f() {}\n",
            "// Replace with your actual key\nconst k = 1;\n",
            "// Implement the retry logic here\nfunction f() {}\n",
            "// for demonstration purposes only\nfunction f() {}\n",
        ] {
            assert_eq!(diagnostics_for(s).len(), 1, "{s}");
        }
    }

    #[test]
    fn ignores_words_containing_here_or_logic() {
        let src = "// thereby closing the socket\n// the logician's proof\n// wherever the cache lives\nfunction f() {}\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn ignores_doc_comments() {
        let src = "/** Logic goes here when the feature flag is set. */\nfunction f() {}\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
