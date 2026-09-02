use std::sync::LazyLock;

use regex::Regex;
use tree_sitter::Node;

use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, Lang, CODE_LANGS};
use crate::registry::RuleDef;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP008",
    name: "Stub-only / unimplemented body",
    tier: Tier::A,
    langs: CODE_LANGS,
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: true,
    check,
};

const MESSAGE: &str = "unimplemented stub function body";

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    if ctx.is_stub_file {
        return;
    }
    match ctx.lang {
        Lang::Python => check_python(rule, ctx, out),
        Lang::Rust => check_rust(rule, ctx, out),
        Lang::Ts | Lang::Tsx => check_ts(rule, ctx, out),
        Lang::Go => check_go(rule, ctx, out),
        Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst | Lang::Html => {} // rule.langs excludes prose; never reached
    }
}

// --- Python ---

fn check_python(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&["function_definition"]) {
        if is_exempt_python(ctx, node) {
            continue;
        }
        let Some(body) = node.child_by_field_name("body") else {
            continue;
        };
        if is_stub_body_python(ctx, body) {
            let (line, col) = ctx.pos(&node);
            out.push(Diagnostic::at(rule, ctx, line, col, MESSAGE));
        }
    }
}

/// @abstractmethod / @overload on the immediate wrapper; nearest enclosing class's
/// superclasses containing Protocol/ABC(Meta); nearest enclosing class's own name
/// suggesting an informal abstract base/mixin (Base/Abstract/Mixin); or a class method
/// whose entire body is a message-carrying `raise NotImplementedError("...")` — all
/// common Python idioms for interface methods that never import the abc module.
fn is_exempt_python(ctx: &LintContext, func: Node) -> bool {
    if let Some(parent) = func.parent() {
        if parent.kind() == "decorated_definition" {
            let mut c = parent.walk();
            for child in parent.children(&mut c) {
                if child.kind() == "decorator" {
                    let t = ctx.node_text(&child);
                    if t.contains("abstractmethod") || t.contains("overload") {
                        return true;
                    }
                }
            }
        }
    }
    let mut cur = func;
    while let Some(parent) = cur.parent() {
        if parent.kind() == "class_definition" {
            if let Some(sc) = parent.child_by_field_name("superclasses") {
                let t = ctx.node_text(&sc);
                if t.contains("Protocol") || t.contains("ABC") {
                    return true;
                }
            }
            if let Some(name) = parent.child_by_field_name("name") {
                let t = ctx.node_text(&name);
                if t.contains("Base") || t.contains("Abstract") || t.contains("Mixin") {
                    return true;
                }
            }
            if let Some(body) = func.child_by_field_name("body") {
                if let Some(stmt) = sole_body_stmt(body) {
                    if stmt.kind() == "raise_statement"
                        && ctx.node_text(&stmt).contains("NotImplementedError")
                        && raises_with_message(ctx, stmt)
                    {
                        return true;
                    }
                }
            }
            break;
        }
        cur = parent;
    }
    false
}

/// The function body's sole statement after stripping a leading docstring, or `None` if
/// there's more than one (or zero).
fn sole_body_stmt(body: Node) -> Option<Node> {
    let mut cursor = body.walk();
    let mut stmts: Vec<Node> = body.named_children(&mut cursor).collect();
    if let Some(first) = stmts.first() {
        if first.kind() == "expression_statement"
            && first.named_child(0).map(|c| c.kind()) == Some("string")
        {
            stmts.remove(0);
        }
    }
    (stmts.len() == 1).then(|| stmts[0])
}

/// The body must be exactly one of: pass / `...` / `raise NotImplementedError(...)`.
fn is_stub_body_python(ctx: &LintContext, body: Node) -> bool {
    let Some(stmt) = sole_body_stmt(body) else {
        return false;
    };
    match stmt.kind() {
        "pass_statement" => true,
        "expression_statement" => stmt.named_child(0).map(|c| c.kind()) == Some("ellipsis"),
        "raise_statement" => ctx.node_text(&stmt).contains("NotImplementedError"),
        _ => false,
    }
}

/// `raise NotImplementedError("explains why")` — a non-empty string-literal argument is
/// near-always deliberate interface documentation, not an AI-left stub.
fn raises_with_message(ctx: &LintContext, raise: Node) -> bool {
    let Some(call) = raise.named_child(0).filter(|n| n.kind() == "call") else {
        return false;
    };
    let Some(args) = call.child_by_field_name("arguments") else {
        return false;
    };
    let mut c = args.walk();
    let has_message = args
        .named_children(&mut c)
        .any(|a| a.kind() == "string" && !ctx.node_text(&a).trim_matches(['"', '\'']).is_empty());
    has_message
}

// --- Rust ---

fn check_rust(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&["function_item"]) {
        if is_cfg_test_ancestor(ctx, node) {
            continue;
        }
        let Some(body) = node.child_by_field_name("body") else {
            continue;
        };
        if is_stub_body_rust(ctx, body) {
            let (line, col) = ctx.pos(&node);
            out.push(Diagnostic::at(rule, ctx, line, col, MESSAGE));
        }
    }
}

fn is_stub_body_rust(ctx: &LintContext, body: Node) -> bool {
    let mut cursor = body.walk();
    let children: Vec<Node> = body.named_children(&mut cursor).collect();
    if children.len() != 1 || children[0].kind() != "macro_invocation" {
        return false;
    }
    let Some(m) = children[0].child_by_field_name("macro") else {
        return false;
    };
    matches!(ctx.node_text(&m), "todo" | "unimplemented")
}

/// Any ancestor `mod_item` preceded by a `#[cfg(test)]`-ish `attribute_item` sibling.
pub(crate) fn is_cfg_test_ancestor(ctx: &LintContext, node: Node) -> bool {
    let mut cur = node;
    while let Some(parent) = cur.parent() {
        if parent.kind() == "mod_item" {
            let mut sib = parent.prev_sibling();
            while let Some(s) = sib {
                if s.kind() != "attribute_item" {
                    break;
                }
                let t = ctx.node_text(&s);
                if t.contains("cfg") && t.contains("test") {
                    return true;
                }
                sib = s.prev_sibling();
            }
        }
        cur = parent;
    }
    false
}

// --- TypeScript / TSX ---

static TS_NOT_IMPLEMENTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)not.implemented").unwrap());

fn check_ts(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&[
        "function_declaration",
        "method_definition",
        "arrow_function",
    ]) {
        let Some(body) = node.child_by_field_name("body") else {
            continue;
        };
        if body.kind() != "statement_block" {
            continue;
        }
        if is_stub_body_ts(ctx, body) {
            let (line, col) = ctx.pos(&node);
            out.push(Diagnostic::at(rule, ctx, line, col, MESSAGE));
        }
    }
}

fn is_stub_body_ts(ctx: &LintContext, body: Node) -> bool {
    let mut cursor = body.walk();
    let children: Vec<Node> = body.named_children(&mut cursor).collect();
    if children.len() != 1 || children[0].kind() != "throw_statement" {
        return false;
    }
    let Some(arg) = children[0].named_child(0) else {
        return false;
    };
    if arg.kind() != "new_expression" {
        return false;
    }
    let Some(ctor) = arg.child_by_field_name("constructor") else {
        return false;
    };
    if ctx.node_text(&ctor) != "Error" {
        return false;
    }
    let Some(args) = arg.child_by_field_name("arguments") else {
        return false;
    };
    TS_NOT_IMPLEMENTED.is_match(ctx.node_text(&args))
}

// --- Go ---

fn check_go(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&["function_declaration", "method_declaration"]) {
        let Some(body) = node.child_by_field_name("body") else {
            continue;
        };
        if body.kind() == "block" && body.named_child_count() == 0 {
            let (line, col) = ctx.pos(&node);
            out.push(Diagnostic::at(rule, ctx, line, col, MESSAGE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
    use crate::engine::{resolve_enabled, Settings};
    use tree_sitter::Parser;

    fn lint(lang: Lang, src: &str) -> Vec<Diagnostic> {
        let mut p = Parser::new();
        p.set_language(&crate::lang::ts_language(lang)).unwrap();
        let tree = p.parse(src, None).unwrap();
        let (comments, strings, index) = context::extract(&tree, src, lang);
        let ctx = LintContext {
            display_path: "t".into(),
            source: src,
            index: Some(&index),
            lang,
            comments: &comments,
            strings: &strings,
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: None,
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn python_pass_flagged() {
        assert_eq!(lint(Lang::Python, "def f(a):\n    pass\n").len(), 1);
    }

    #[test]
    fn python_ellipsis_flagged() {
        assert_eq!(lint(Lang::Python, "def f():\n    ...\n").len(), 1);
    }

    #[test]
    fn python_abstractmethod_exempt() {
        let src = "from abc import ABC, abstractmethod\nclass C(ABC):\n    @abstractmethod\n    def f(self):\n        pass\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
    }

    #[test]
    fn python_informal_base_class_exempt() {
        // BaseAdapter-style: plain class named *Base*, bare `raise NotImplementedError`.
        let src =
            "class BaseAdapter:\n    def send(self, request):\n        raise NotImplementedError\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
        // SessionRedirectMixin-style: plain class named *Mixin*, ellipsis body.
        let src2 = "class SessionRedirectMixin:\n    def send(self) -> None: ...\n";
        assert_eq!(lint(Lang::Python, src2).len(), 0);
    }

    #[test]
    fn python_message_carrying_not_implemented_exempt() {
        // AuthBase/cookiejar-style: raise with an explanatory message, no ABC/Base/Mixin needed.
        let src = "class C:\n    def __call__(self, r):\n        raise NotImplementedError(\"Auth hooks must be callable.\")\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
        // Sanity: a message-less raise on a plain (non-Base/Mixin) class is still flagged.
        let src2 = "class C:\n    def f(self):\n        raise NotImplementedError\n";
        assert_eq!(lint(Lang::Python, src2).len(), 1);
    }

    #[test]
    fn python_protocol_exempt() {
        let src = "class R(Protocol):\n    def read(self, n: int) -> bytes: ...\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
    }

    #[test]
    fn python_overload_exempt() {
        let src = "@overload\ndef f(a: int) -> str: ...\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
    }

    #[test]
    fn rust_todo_flagged() {
        assert_eq!(lint(Lang::Rust, "fn f() { todo!() }\n").len(), 1);
    }

    #[test]
    fn rust_cfg_test_exempt() {
        let src = "#[cfg(test)]\nmod tests {\n    fn h() { todo!() }\n}\n";
        assert_eq!(lint(Lang::Rust, src).len(), 0);
    }

    #[test]
    fn rust_trait_sig_not_matched() {
        let src = "trait T { fn i(&self); }\n";
        assert_eq!(lint(Lang::Rust, src).len(), 0);
    }

    #[test]
    fn ts_throw_not_implemented_flagged() {
        let src = "function f() { throw new Error(\"Not implemented\"); }\n";
        assert_eq!(lint(Lang::Ts, src).len(), 1);
    }

    #[test]
    fn ts_interface_not_matched() {
        let src = "interface I { m(): void; }\n";
        assert_eq!(lint(Lang::Ts, src).len(), 0);
    }

    #[test]
    fn go_empty_body_flagged() {
        assert_eq!(lint(Lang::Go, "package main\nfunc F() {}\n").len(), 1);
    }

    #[test]
    fn go_interface_not_matched() {
        let src = "package main\ntype I interface { M() }\n";
        assert_eq!(lint(Lang::Go, src).len(), 0);
    }

    #[test]
    fn go_comment_body_not_flagged() {
        let src = "package main\nfunc G() {\n // TODO: implement\n}\n";
        assert_eq!(lint(Lang::Go, src).len(), 0);
    }

    #[test]
    fn resolve_enabled_includes_slop008() {
        let s = Settings {
            enabled: resolve_enabled(&[], &[], &[], &[], &[], false),
            deps: None,
            custom_rules: Vec::new(),
            natlangs: crate::lang::ALL_NATLANGS.to_vec(),
        };
        assert!(s.enabled.contains("SLOP008"));
    }
}
