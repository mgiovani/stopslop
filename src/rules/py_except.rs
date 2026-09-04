use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, Lang};
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;
use tree_sitter::Node;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP006",
    name: "Broad / swallowing except",
    tier: Tier::A,
    langs: &[Lang::Python],
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: true,
    check,
};

static LOG_CALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(print|log)$|^logging\.").unwrap());

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&["except_clause"]) {
        let bare = node.child_by_field_name("value").is_none();
        let mut cursor = node.walk();
        let Some(body) = node
            .named_children(&mut cursor)
            .find(|c| c.kind() == "block")
        else {
            continue;
        };
        let mut body_cursor = body.walk();
        let statements: Vec<Node> = body
            .named_children(&mut body_cursor)
            .filter(|c| c.kind() != "comment")
            .collect();
        let swallows = statements.iter().all(|s| is_swallow_statement(ctx, s));

        if swallows {
            let msg = if bare {
                "bare `except:` swallows all exceptions".to_string()
            } else {
                "exception handler silently swallows the error".to_string()
            };
            let (line, col) = ctx.pos(&node);
            out.push(Diagnostic::at(rule, ctx, line, col, msg));
        }
    }
}

fn is_swallow_statement(ctx: &LintContext, stmt: &Node) -> bool {
    if stmt.kind() == "pass_statement" {
        return true;
    }
    if stmt.kind() != "expression_statement" {
        return false;
    }
    let Some(call) = stmt.named_child(0).filter(|c| c.kind() == "call") else {
        return false;
    };
    let Some(func) = call.child_by_field_name("function") else {
        return false;
    };
    LOG_CALL.is_match(ctx.node_text(&func))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
    use crate::lang::ts_language;
    use tree_sitter::Parser;

    fn run(src: &str) -> Vec<Diagnostic> {
        let mut parser = Parser::new();
        parser.set_language(&ts_language(Lang::Python)).unwrap();
        let tree = parser.parse(src, None).unwrap();
        let (comments, strings, index) = context::extract(&tree, src, Lang::Python);
        let ctx = LintContext {
            display_path: "test".into(),
            source: src,
            index: Some(&index),
            lang: Lang::Python,
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
    fn bare_except_pass_flags() {
        assert_eq!(run("try:\n    f()\nexcept:\n    pass\n").len(), 1);
    }

    #[test]
    fn broad_except_log_flags() {
        assert_eq!(
            run("try:\n    f()\nexcept Exception as e:\n    log(e)\n").len(),
            1
        );
    }

    #[test]
    fn specific_except_recovery_clean() {
        assert_eq!(
            run("try:\n    int(val)\nexcept ValueError:\n    val = 0\n").len(),
            0
        );
    }

    #[test]
    fn broad_except_reraise_clean() {
        assert_eq!(
            run("try:\n    f()\nexcept Exception:\n    raise\n").len(),
            0
        );
    }

    /// A first except with a real statement (not flagged), then a swallowing except: a
    /// `continue` mistakenly turned into `return` would drop the second except too.
    #[test]
    fn first_except_not_flagged_second_except_still_flagged() {
        let src = "try:\n    f()\nexcept ValueError:\n    handle(e)\nexcept Exception:\n    pass\n";
        let diags = run(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 5);
    }
}
