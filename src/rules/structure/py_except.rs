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

/// A swallowing body only counts against a broad handler: bare `except:`, or a caught type
/// named `Exception`/`BaseException` (alone, among several types, or wrapped in a tuple/
/// parenthesized expression). cpython-lib (pinned 2019 human stdlib): 147 files / 465 SLOP006
/// findings before this change, 421 of them (90.5%) a narrow named exception such as
/// `except KeyError: pass` recovering on purpose, not swallowing an error.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&["except_clause"]) {
        let bare = node.child_by_field_name("value").is_none();
        if !bare && !is_broad_except(ctx, node) {
            continue;
        }
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

/// tree-sitter-python 0.25's `except_clause.value` field is `multiple: true` (the old
/// `except A, B:` two-value shape); `except A as e:` wraps the type in an `as_pattern` whose own
/// `alias` field carries the binding, so `except_clause` never populates its own (vestigial)
/// `alias` field for that syntax. `except (A, B) as e:` nests a `tuple` inside the `as_pattern`.
fn is_broad_except(ctx: &LintContext, except_clause: Node) -> bool {
    let mut cursor = except_clause.walk();
    let mut types = Vec::new();
    for value in except_clause.children_by_field_name("value", &mut cursor) {
        collect_exception_types(value, &mut types);
    }
    types.iter().any(|t| is_broad_type_name(ctx.node_text(t)))
}

fn collect_exception_types<'a>(node: Node<'a>, out: &mut Vec<Node<'a>>) {
    match node.kind() {
        "as_pattern" => {
            let alias = node.child_by_field_name("alias");
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if Some(child) != alias {
                    collect_exception_types(child, out);
                }
            }
        }
        "tuple" | "parenthesized_expression" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_exception_types(child, out);
            }
        }
        _ => out.push(node),
    }
}

fn is_broad_type_name(name: &str) -> bool {
    name == "Exception"
        || name == "BaseException"
        || name.ends_with(".Exception")
        || name.ends_with(".BaseException")
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

    #[test]
    fn narrow_named_except_pass_clean() {
        assert_eq!(run("try:\n    f()\nexcept KeyError:\n    pass\n").len(), 0);
    }

    #[test]
    fn tuple_with_broad_type_as_alias_flags() {
        let src = "try:\n    f()\nexcept (ValueError, Exception) as e:\n    pass\n";
        assert_eq!(run(src).len(), 1);
    }

    #[test]
    fn broad_named_except_as_alias_pass_flags() {
        assert_eq!(
            run("try:\n    f()\nexcept Exception as e:\n    pass\n").len(),
            1
        );
    }

    #[test]
    fn narrow_named_except_log_only_clean() {
        let src = "try:\n    f()\nexcept ValueError:\n    logging.warning(x)\n";
        assert_eq!(run(src).len(), 0);
    }
}
