use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;
use tree_sitter::Node;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP039",
    name: "Pass-through wrapper function",
    tier: Tier::B,
    langs: &[Lang::Ts, Lang::Tsx, Lang::Python, Lang::Go, Lang::Rust],
    default_on: true,
    path_gated: true,
    check,
};

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    match ctx.lang {
        Lang::Ts | Lang::Tsx => check_ts(rule, ctx, out),
        Lang::Python => check_python(rule, ctx, out),
        Lang::Go => check_go(rule, ctx, out),
        Lang::Rust => check_rust(rule, ctx, out),
        Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst => {} // rule.langs excludes prose; never reached
    }
}

/// Shared across all five languages: every grammar's call node exposes `function` and
/// `arguments` fields (Python's node kind is `call`, the others `call_expression`, but the
/// field names line up), so this one check works for all of them.
fn call_matches_forward<'a>(
    ctx: &LintContext<'a>,
    call: Node<'a>,
    wrapper_name: &str,
    params: &[&str],
) -> Option<&'a str> {
    let func = call.child_by_field_name("function")?;
    if func.kind() != "identifier" {
        return None; // method call / member access: not a plain forward
    }
    let callee = ctx.node_text(&func);
    if callee == wrapper_name {
        return None; // recursion, not a forward
    }
    let args_node = call.child_by_field_name("arguments")?;
    let mut cursor = args_node.walk();
    let args: Vec<Node> = args_node.named_children(&mut cursor).collect();
    if args.len() != params.len() {
        return None;
    }
    for (a, p) in args.iter().zip(params.iter()) {
        if a.kind() != "identifier" || ctx.node_text(a) != *p {
            return None;
        }
    }
    Some(callee)
}

/// A node's named children with comments stripped out -- a trailing `// note` on the same
/// source line as an opening brace parses as a named `comment` child of the body, which would
/// otherwise miscount an actual single-statement body as having two statements.
fn non_comment_children<'a>(node: Node<'a>) -> Vec<Node<'a>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|c| !matches!(c.kind(), "comment" | "line_comment" | "block_comment"))
        .collect()
}

fn try_flag(
    rule: &'static RuleDef,
    ctx: &LintContext,
    def_node: Node,
    wrapper_name: &str,
    params: &[&str],
    call: Node,
    out: &mut Vec<Diagnostic>,
) {
    let Some(callee) = call_matches_forward(ctx, call, wrapper_name, params) else {
        return;
    };
    let (line, col) = ctx.pos(&def_node);
    out.push(Diagnostic::at_fix(
        rule,
        ctx,
        line,
        col,
        format!("`{wrapper_name}` only forwards to `{callee}`"),
        format!("call `{callee}` directly and delete the wrapper"),
    ));
}

// --- TypeScript / TSX ---

fn check_ts(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&["function_declaration"]) {
        check_ts_function_declaration(rule, ctx, node, out);
    }
    for node in ctx.nodes(&["arrow_function"]) {
        check_ts_arrow(rule, ctx, node, out);
    }
}

fn check_ts_function_declaration(
    rule: &'static RuleDef,
    ctx: &LintContext,
    node: Node,
    out: &mut Vec<Diagnostic>,
) {
    let Some(name_node) = node.child_by_field_name("name") else {
        return;
    };
    let Some(params_node) = node.child_by_field_name("parameters") else {
        return;
    };
    let Some(body_node) = node.child_by_field_name("body") else {
        return;
    };
    let wrapper_name = ctx.node_text(&name_node);
    let Some(params) = ts_params(ctx, params_node) else {
        return;
    };
    let Some(call) = ts_sole_return_call(body_node) else {
        return;
    };
    try_flag(rule, ctx, node, wrapper_name, &params, call, out);
}

fn check_ts_arrow(
    rule: &'static RuleDef,
    ctx: &LintContext,
    node: Node,
    out: &mut Vec<Diagnostic>,
) {
    let Some(parent) = node.parent() else {
        return;
    };
    if parent.kind() != "variable_declarator" {
        return; // only the named `const f = (a, b) => g(a, b);` shape counts
    }
    let Some(name_node) = parent.child_by_field_name("name") else {
        return;
    };
    if name_node.kind() != "identifier" {
        return;
    }
    let wrapper_name = ctx.node_text(&name_node);

    let params: Vec<&str> = if let Some(p) = node.child_by_field_name("parameter") {
        if p.kind() != "identifier" {
            return;
        }
        vec![ctx.node_text(&p)]
    } else if let Some(params_node) = node.child_by_field_name("parameters") {
        let Some(list) = ts_params(ctx, params_node) else {
            return;
        };
        list
    } else {
        Vec::new()
    };

    let Some(body) = node.child_by_field_name("body") else {
        return;
    };
    let call = if body.kind() == "call_expression" {
        Some(body)
    } else {
        ts_sole_return_call(body)
    };
    let Some(call) = call else {
        return;
    };
    try_flag(rule, ctx, node, wrapper_name, &params, call, out);
}

fn ts_params<'a>(ctx: &LintContext<'a>, params_node: Node<'a>) -> Option<Vec<&'a str>> {
    let mut cursor = params_node.walk();
    let mut out = Vec::new();
    for child in params_node.named_children(&mut cursor) {
        if child.kind() != "required_parameter" {
            return None; // optional_parameter (has a default): bail
        }
        let mut c2 = child.walk();
        let core: Vec<Node> = child
            .named_children(&mut c2)
            .filter(|n| !matches!(n.kind(), "decorator" | "type_annotation"))
            .collect();
        if core.len() != 1 || core[0].kind() != "identifier" {
            return None; // destructured/rest/defaulted param: bail
        }
        out.push(ctx.node_text(&core[0]));
    }
    Some(out)
}

fn ts_sole_return_call(body: Node) -> Option<Node> {
    if body.kind() != "statement_block" {
        return None;
    }
    let stmts = non_comment_children(body);
    if stmts.len() != 1 || stmts[0].kind() != "return_statement" {
        return None;
    }
    let arg = stmts[0].named_child(0)?;
    (arg.kind() == "call_expression").then_some(arg)
}

// --- Python ---

fn check_python(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&["function_definition"]) {
        if node
            .parent()
            .is_some_and(|p| p.kind() == "decorated_definition")
        {
            continue; // decorated forwarder is doing something
        }
        let Some(name_node) = node.child_by_field_name("name") else {
            continue;
        };
        let Some(params_node) = node.child_by_field_name("parameters") else {
            continue;
        };
        let Some(body_node) = node.child_by_field_name("body") else {
            continue;
        };
        let wrapper_name = ctx.node_text(&name_node);
        let Some(params) = python_params(ctx, params_node) else {
            continue;
        };
        let Some(call) = python_sole_return_call(body_node) else {
            continue;
        };
        try_flag(rule, ctx, node, wrapper_name, &params, call, out);
    }
}

fn python_params<'a>(ctx: &LintContext<'a>, params_node: Node<'a>) -> Option<Vec<&'a str>> {
    let mut cursor = params_node.walk();
    let mut out = Vec::new();
    for child in params_node.named_children(&mut cursor) {
        match child.kind() {
            "identifier" => out.push(ctx.node_text(&child)),
            "typed_parameter" => {
                let ident = child.named_child(0)?;
                if ident.kind() != "identifier" {
                    return None;
                }
                out.push(ctx.node_text(&ident));
            }
            // default_parameter / typed_default_parameter / *args / **kwargs / tuple params: bail
            _ => return None,
        }
    }
    Some(out)
}

fn python_sole_return_call(body: Node) -> Option<Node> {
    let stmts = non_comment_children(body);
    if stmts.len() != 1 || stmts[0].kind() != "return_statement" {
        return None;
    }
    let expr = stmts[0].named_child(0)?;
    (expr.kind() == "call").then_some(expr)
}

// --- Go ---

fn check_go(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&["function_declaration"]) {
        let Some(name_node) = node.child_by_field_name("name") else {
            continue;
        };
        let Some(params_node) = node.child_by_field_name("parameters") else {
            continue;
        };
        let Some(body_node) = node.child_by_field_name("body") else {
            continue;
        };
        let wrapper_name = ctx.node_text(&name_node);
        let Some(params) = go_params(ctx, params_node) else {
            continue;
        };
        let Some(call) = go_sole_return_call(body_node) else {
            continue;
        };
        try_flag(rule, ctx, node, wrapper_name, &params, call, out);
    }
}

fn go_params<'a>(ctx: &LintContext<'a>, params_node: Node<'a>) -> Option<Vec<&'a str>> {
    let mut cursor = params_node.walk();
    let mut out = Vec::new();
    for child in params_node.named_children(&mut cursor) {
        if child.kind() != "parameter_declaration" {
            return None; // variadic_parameter_declaration: bail
        }
        let mut nc = child.walk();
        for name in child.children_by_field_name("name", &mut nc) {
            if name.kind() != "identifier" {
                return None;
            }
            out.push(ctx.node_text(&name));
        }
    }
    Some(out)
}

fn go_sole_return_call(body: Node) -> Option<Node> {
    let top = non_comment_children(body);
    if top.len() != 1 || top[0].kind() != "statement_list" {
        return None;
    }
    let stmts = non_comment_children(top[0]);
    if stmts.len() != 1 || stmts[0].kind() != "return_statement" {
        return None;
    }
    let els = non_comment_children(stmts[0]);
    if els.len() != 1 || els[0].kind() != "expression_list" {
        return None;
    }
    let items = non_comment_children(els[0]);
    if items.len() != 1 || items[0].kind() != "call_expression" {
        return None;
    }
    Some(items[0])
}

// --- Rust ---

fn check_rust(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&["function_item"]) {
        if has_attribute(node) {
            continue; // attribute macro other than visibility: doing something
        }
        let Some(name_node) = node.child_by_field_name("name") else {
            continue;
        };
        let Some(params_node) = node.child_by_field_name("parameters") else {
            continue;
        };
        let Some(body_node) = node.child_by_field_name("body") else {
            continue;
        };
        let wrapper_name = ctx.node_text(&name_node);
        let Some(params) = rust_params(ctx, params_node) else {
            continue;
        };
        let Some(call) = rust_sole_return_call(body_node) else {
            continue;
        };
        try_flag(rule, ctx, node, wrapper_name, &params, call, out);
    }
}

/// A preceding `attribute_item` sibling (`#[...]`) directly on this function -- `pub`/
/// `pub(crate)` is a child `visibility_modifier`, not a sibling, so it never trips this.
fn has_attribute(node: Node) -> bool {
    node.prev_sibling()
        .is_some_and(|s| s.kind() == "attribute_item")
}

fn rust_params<'a>(ctx: &LintContext<'a>, params_node: Node<'a>) -> Option<Vec<&'a str>> {
    let mut cursor = params_node.walk();
    let mut out = Vec::new();
    for child in params_node.named_children(&mut cursor) {
        if child.kind() != "parameter" {
            return None; // self_parameter / variadic_parameter / attribute_item: bail
        }
        let pattern = child.child_by_field_name("pattern")?;
        if pattern.kind() != "identifier" {
            return None; // ref/tuple/mut-binding pattern beyond a plain name: bail
        }
        out.push(ctx.node_text(&pattern));
    }
    Some(out)
}

fn rust_sole_return_call(body: Node) -> Option<Node> {
    let stmts = non_comment_children(body);
    if stmts.len() != 1 {
        return None;
    }
    match stmts[0].kind() {
        // implicit tail-expression return: `g(a, b)`, no semicolon, bare in the block
        "call_expression" => Some(stmts[0]),
        // `g(a, b);` (semicolon, no `return`) discards the value and returns unit --
        // semantically different from a forward, so it must stay disqualified rather than
        // treated as equivalent.
        "expression_statement" => {
            let inner = stmts[0].named_child(0)?;
            if inner.kind() != "return_expression" {
                return None;
            }
            let call = inner.named_child(0)?;
            (call.kind() == "call_expression").then_some(call)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
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
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    // --- TS ---

    #[test]
    fn ts_function_wrapper_flagged() {
        let diags = lint(Lang::Ts, "function f(a, b) { return g(a, b); }\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "`f` only forwards to `g`");
    }

    #[test]
    fn ts_arrow_wrapper_flagged() {
        assert_eq!(lint(Lang::Ts, "const f = (a, b) => g(a, b);\n").len(), 1);
    }

    #[test]
    fn ts_reordered_args_clean() {
        assert_eq!(
            lint(Lang::Ts, "function f(a, b) { return g(b, a); }\n").len(),
            0
        );
    }

    #[test]
    fn ts_extra_arg_clean() {
        assert_eq!(
            lint(Lang::Ts, "function f(a, b) { return g(a, b, 1); }\n").len(),
            0
        );
    }

    #[test]
    fn ts_default_param_clean() {
        assert_eq!(
            lint(Lang::Ts, "function f(a, b = 1) { return g(a, b); }\n").len(),
            0
        );
    }

    #[test]
    fn ts_body_does_real_work_clean() {
        let src = "function f(a, b) { const c = a + b; return g(c); }\n";
        assert_eq!(lint(Lang::Ts, src).len(), 0);
    }

    #[test]
    fn ts_trailing_comment_on_brace_line_still_flagged() {
        // A `// note` on the same line as the opening `{` parses as a named `comment` child of
        // the body; it must not be miscounted as a second statement.
        let src = "function f(a, b) { // note\n  return g(a, b);\n}\n";
        assert_eq!(lint(Lang::Ts, src).len(), 1);
    }

    #[test]
    fn ts_recursive_clean() {
        assert_eq!(
            lint(Lang::Ts, "function f(a, b) { return f(a, b); }\n").len(),
            0
        );
    }

    // --- Python ---

    #[test]
    fn python_wrapper_flagged() {
        let diags = lint(Lang::Python, "def f(a, b):\n    return g(a, b)\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "`f` only forwards to `g`");
    }

    #[test]
    fn python_decorated_clean() {
        let src = "@lru_cache\ndef f(a, b):\n    return g(a, b)\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
    }

    #[test]
    fn python_star_args_clean() {
        let src = "def f(*args, **kwargs):\n    return g(*args, **kwargs)\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
    }

    /// A first def that fails the sole-return-call guard, then a real forwarder: a `continue`
    /// mistakenly turned into `return` would drop the second def too.
    #[test]
    fn python_first_def_fails_guard_second_still_flagged() {
        let src =
            "def helper(a):\n    x = a + 1\n    return x\n\ndef wrap(a, b):\n    return g(a, b)\n";
        let diags = lint(Lang::Python, src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 5);
    }

    // --- Go ---

    #[test]
    fn go_wrapper_flagged() {
        let src = "package main\nfunc F(a int) int {\n\treturn g(a)\n}\n";
        assert_eq!(lint(Lang::Go, src).len(), 1);
    }

    #[test]
    fn go_grouped_params_flagged() {
        let src = "package main\nfunc F(a, b int) int {\n\treturn g(a, b)\n}\n";
        assert_eq!(lint(Lang::Go, src).len(), 1);
    }

    #[test]
    fn go_trailing_comment_on_brace_line_still_flagged() {
        let src = "package main\nfunc F(a int) int { // note\n\treturn g(a)\n}\n";
        assert_eq!(lint(Lang::Go, src).len(), 1);
    }

    #[test]
    fn go_extra_logic_clean() {
        let src = "package main\nfunc F(a int) int {\n\tlog.Println(a)\n\treturn g(a)\n}\n";
        assert_eq!(lint(Lang::Go, src).len(), 0);
    }

    // --- Rust ---

    #[test]
    fn rust_tail_expr_wrapper_flagged() {
        let diags = lint(Lang::Rust, "fn f(a: A, b: B) -> R { g(a, b) }\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "`f` only forwards to `g`");
    }

    #[test]
    fn rust_explicit_return_wrapper_flagged() {
        assert_eq!(
            lint(Lang::Rust, "fn f(a: A, b: B) -> R { return g(a, b); }\n").len(),
            1
        );
    }

    #[test]
    fn rust_trailing_comment_on_brace_line_still_flagged() {
        let src = "fn f(a: A, b: B) -> R { // note\n    g(a, b)\n}\n";
        assert_eq!(lint(Lang::Rust, src).len(), 1);
    }

    #[test]
    fn rust_semicolon_discarded_call_clean() {
        // `g(a, b);` with a semicolon and no `return` discards the value and returns `()` --
        // not an equivalent forward, even though it superficially looks like one.
        assert_eq!(lint(Lang::Rust, "fn f(a: A, b: B) { g(a, b); }\n").len(), 0);
    }

    #[test]
    fn rust_attribute_exempt() {
        let src = "#[inline]\nfn f(a: A) -> R { g(a) }\n";
        assert_eq!(lint(Lang::Rust, src).len(), 0);
    }

    #[test]
    fn rust_pub_visibility_still_flagged() {
        assert_eq!(lint(Lang::Rust, "pub fn f(a: A) -> R { g(a) }\n").len(), 1);
    }

    #[test]
    fn rust_ref_type_still_flagged() {
        // `&A` is part of the parameter's type, not its pattern -- `a` itself is still a plain
        // identifier binding, so this is a wrapper like any other.
        assert_eq!(lint(Lang::Rust, "fn f(a: &A) -> R { g(a) }\n").len(), 1);
    }
}
