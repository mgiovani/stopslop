use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, CODE_LANGS};
use crate::registry::RuleDef;
use crate::suppress::comment_body;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP051",
    name: "Comment announces demo code",
    tier: Tier::B,
    langs: CODE_LANGS,
    natlangs: &[NatLang::En],
    default_on: true,
    path_gated: false,
    check,
};

static USAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:(?:example|sample|test) usage|example \d+:)").unwrap());

static DATA_ANNOUNCEMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:(?:(?:creat|generat|defin|build|initiali[sz]|instantiat|mak)(?:e|ing) )?(?:some |a |an |the )?(?:sample|example|dummy|mock|synthetic) (?:data|dataset|dataframe|input|inputs|values|records|users|payload|response|responses|config|configuration|entries|database)|(?:creat|generat|defin|build|initiali[sz]|instantiat|mak)(?:e|ing) (?:some |a |an |the )?(?:sample|example|dummy|mock|synthetic) )",
    )
    .unwrap()
});

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for c in ctx.comments {
        if c.is_doc {
            continue;
        }
        let body = comment_body(c.text);
        if USAGE.is_match(body) || DATA_ANNOUNCEMENT.is_match(body) {
            out.push(Diagnostic::at(
                rule,
                ctx,
                c.line,
                c.col,
                "comment opens with an example-usage or sample-data announcement",
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
    fn flags_usage_and_data_announcements() {
        for s in [
            "// Example usage:\nfoo();\n",
            "// sample usage\n",
            "// Example 2: two users\nfoo();\n",
            "// Create sample data\nconst d = [];\n",
            "// Mock response\nconst r = {};\n",
            "// Generating a dummy dataset\nconst d = [];\n",
        ] {
            assert_eq!(diagnostics_for(s).len(), 1, "{s}");
        }
    }

    #[test]
    fn ignores_mid_comment_mentions_and_doc_comments() {
        let src = "// see the example usage in the README\n// fixtures hold the mock data\n/** Example usage: call f() */\nfunction f() {}\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
