use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;
use tree_sitter::Node;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP005",
    name: "Empty / log-only catch",
    tier: Tier::A,
    langs: &[Lang::Ts, Lang::Tsx, Lang::Go, Lang::Rust],
    default_on: true,
    path_gated: true,
    check,
};

const MSG: &str = "error swallowed by empty or log-only handler";

static INTENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)intentional|ignore|expected|on purpose|best.?effort|no[- ]?op|swallow|deliberate",
    )
    .unwrap()
});
static CONSOLE_CALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^console\.(log|warn|error|info|debug)$").unwrap());

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    match ctx.lang {
        Lang::Ts | Lang::Tsx => check_ts(rule, ctx, out),
        Lang::Go => check_go(rule, ctx, out),
        Lang::Rust => check_rust(rule, ctx, out),
        Lang::Python => {}                                 // owned by SLOP006
        Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst => {} // rule.langs excludes prose; never reached
    }
}

fn check_ts(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    ctx.walk(|node| {
        if node.kind() != "catch_clause" {
            return;
        }
        let Some(body) = node.child_by_field_name("body") else {
            return;
        };
        let mut cursor = body.walk();
        let children: Vec<Node> = body.named_children(&mut cursor).collect();
        let comments: Vec<&Node> = children.iter().filter(|c| c.kind() == "comment").collect();
        let others: Vec<&Node> = children.iter().filter(|c| c.kind() != "comment").collect();

        let swallows = if others.is_empty() {
            // empty body or comment-only body: flag unless a comment states intent.
            !comments.iter().any(|c| INTENT.is_match(ctx.node_text(c)))
        } else {
            others.iter().all(|s| is_console_call_statement(ctx, s))
        };

        if swallows {
            let (line, col) = ctx.pos(&node);
            out.push(Diagnostic::at(rule, ctx, line, col, MSG));
        }
    });
}

fn is_console_call_statement(ctx: &LintContext, stmt: &Node) -> bool {
    if stmt.kind() != "expression_statement" {
        return false;
    }
    let Some(call) = stmt
        .named_child(0)
        .filter(|c| c.kind() == "call_expression")
    else {
        return false;
    };
    let Some(func) = call.child_by_field_name("function") else {
        return false;
    };
    CONSOLE_CALL.is_match(ctx.node_text(&func))
}

fn check_go(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    ctx.walk(|node| {
        if node.kind() != "if_statement" {
            return;
        }
        let Some(consequence) = node.child_by_field_name("consequence") else {
            return;
        };
        // Header = "if [init;] cond" text, so a `recover()` in an initializer (the common
        // `if err := recover(); err != nil` shape) is seen even though it's outside `condition`.
        let header = &ctx.source[node.start_byte()..consequence.start_byte()];
        let is_err_check =
            header.contains("recover(") || (header.contains("err") && header.contains("!= nil"));
        if !is_err_check {
            return;
        }
        // ponytail: err-name heuristic on raw header text; refine if it FPs.
        let mut cursor = consequence.walk();
        let has_statements = consequence
            .named_children(&mut cursor)
            .any(|c| c.kind() == "statement_list");
        if !has_statements {
            let (line, col) = ctx.pos(&node);
            out.push(Diagnostic::at(rule, ctx, line, col, MSG));
        }
    });
}

fn check_rust(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    ctx.walk(|node| {
        if node.kind() != "match_arm" {
            return;
        }
        let Some(pattern) = node.child_by_field_name("pattern") else {
            return;
        };
        if !ctx.node_text(&pattern).trim_start().starts_with("Err") {
            return;
        }
        let Some(value) = node.child_by_field_name("value") else {
            return;
        };
        if value.kind() != "block" {
            return;
        }
        let mut cursor = value.walk();
        let children: Vec<Node> = value.named_children(&mut cursor).collect();
        let all_comments = children
            .iter()
            .all(|c| c.kind() == "line_comment" || c.kind() == "block_comment");
        if !all_comments {
            return; // has a real statement: recovery/logging with side effect beyond comments
        }
        let flagged = if children.is_empty() {
            true
        } else {
            !children.iter().any(|c| INTENT.is_match(ctx.node_text(c)))
        };
        if flagged {
            let (line, col) = ctx.pos(&node);
            out.push(Diagnostic::at(rule, ctx, line, col, MSG));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
    use crate::lang::ts_language;
    use tree_sitter::Parser;

    fn run(lang: Lang, src: &str) -> Vec<Diagnostic> {
        let mut parser = Parser::new();
        parser.set_language(&ts_language(lang)).unwrap();
        let tree = parser.parse(src, None).unwrap();
        let (comments, strings) = context::extract(&tree, src, lang);
        let ctx = LintContext {
            display_path: "test".into(),
            source: src,
            tree: Some(&tree),
            lang,
            comments: &comments,
            strings: &strings,
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: None,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn ts_empty_catch_flags() {
        assert_eq!(run(Lang::Ts, "try { risky(); } catch (e) {}").len(), 1);
    }

    #[test]
    fn ts_log_only_flags() {
        assert_eq!(
            run(Lang::Ts, "try { x(); } catch (e) { console.log(e); }").len(),
            1
        );
    }

    #[test]
    fn ts_rethrow_clean() {
        assert_eq!(
            run(Lang::Ts, "try { x(); } catch (e) { throw e; }").len(),
            0
        );
    }

    #[test]
    fn ts_intent_comment_clean() {
        assert_eq!(
            run(
                Lang::Ts,
                "try { x(); } catch (e) { // intentional: skip\n }"
            )
            .len(),
            0
        );
    }

    #[test]
    fn go_empty_err_check_flags() {
        assert_eq!(
            run(
                Lang::Go,
                "package main\nfunc f() {\n\tif err != nil {\n\t}\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn go_recover_initializer_flags() {
        assert_eq!(
            run(
                Lang::Go,
                "package main\nfunc f() {\n\tdefer func() {\n\t\tif r := recover(); r != nil {\n\t\t}\n\t}()\n}"
            )
            .len(),
            1
        );
    }

    #[test]
    fn go_recovery_clean() {
        assert_eq!(
            run(
                Lang::Go,
                "package main\nfunc f() {\n\tif err != nil {\n\t\treturn err\n\t}\n}"
            )
            .len(),
            0
        );
    }

    #[test]
    fn rust_empty_err_arm_flags() {
        assert_eq!(
            run(
                Lang::Rust,
                "fn f(v: Result<i32,E>) { match v { Err(e) => {}, Ok(v) => println!(\"{}\", v), } }"
            )
            .len(),
            1
        );
    }

    #[test]
    fn rust_recovery_arm_clean() {
        assert_eq!(
            run(
                Lang::Rust,
                "fn f(v: Result<i32,E>) { match v { Err(e) => println!(\"{}\", e), Ok(v) => println!(\"{}\", v), } }"
            )
            .len(),
            0
        );
    }
}
