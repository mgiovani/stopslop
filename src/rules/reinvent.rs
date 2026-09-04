// ai-slop-ignore-file: SLOP037 -- this rule's own tests must contain the patterns it detects
use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, Lang, CODE_LANGS};
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;
use tree_sitter::Node;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP037",
    name: "Reinvented stdlib / native platform feature",
    tier: Tier::B,
    langs: CODE_LANGS,
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: true,
    check,
};

// Rust has no language-specific pattern here: candidates considered (fold sums, manual min/max,
// to_string+format! chains) have too many legitimate uses to clear this family's near-zero
// false-positive bar. Rust still gets the cross-language email-regex check below.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    match ctx.lang {
        Lang::Ts | Lang::Tsx => check_ts(rule, ctx, out),
        Lang::Python => check_python(rule, ctx, out),
        Lang::Go => check_go(rule, ctx, out),
        Lang::Rust => {}
        Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst | Lang::Html | Lang::Image => {} // rule.langs excludes prose and images; never reached
    }
    check_email_regex(rule, ctx, out);
}

/// Byte offset -> 1-based (line, col) for the raw-source regex scans below (this rule's AST
/// walks use `ctx.pos` instead; this is only for `Regex::find_iter(ctx.source)` matches).
/// `line_starts` is built lazily by the caller: most files have no match, and a file with many
/// matches must not rescan from byte 0 per match (issue #21, the shape #3 and #20 fixed).
fn byte_pos(source: &str, line_starts: &[usize], byte: usize) -> (usize, usize) {
    let line = line_starts.partition_point(|&start| start <= byte);
    let col = source[line_starts[line - 1]..byte].chars().count() + 1;
    (line, col)
}

fn line_starts(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(source.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}

// --- TypeScript / TSX ---

static JSON_CLONE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"JSON\.parse\s*\(\s*JSON\.stringify\s*\(").unwrap());
static RANDOM_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Math\.random\s*\(\s*\)\s*\.\s*toString\s*\(\s*36\s*\)").unwrap());
static SPLIT_AMP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\.split\s*\(\s*['"]&['"]\s*\)"#).unwrap());
static SPLIT_EQ_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\.split\s*\(\s*['"]=['"]\s*\)"#).unwrap());
// Body capture assumes a brace-free loop body (true for the manual left-pad idiom this hunts);
// a body with nested braces just won't match, which only costs recall, never a false positive.
static LEFT_PAD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"while\s*\(\s*(\w+)\.length\s*<[^)]*\)\s*\{([^{}]*)\}").unwrap());

fn check_ts(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let starts = std::cell::OnceCell::new();
    let pos = |byte: usize| {
        byte_pos(
            ctx.source,
            starts.get_or_init(|| line_starts(ctx.source)),
            byte,
        )
    };
    for m in JSON_CLONE_RE.find_iter(ctx.source) {
        if ctx.in_comment_or_string(m.start()) {
            continue;
        }
        let (line, col) = pos(m.start());
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            "hand-rolled deep clone via a JSON round-trip",
            "use `structuredClone(value)`",
        ));
    }

    for m in RANDOM_ID_RE.find_iter(ctx.source) {
        if ctx.in_comment_or_string(m.start()) {
            continue;
        }
        let (line, col) = pos(m.start());
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            "id generated from a stringified random float",
            "use `crypto.randomUUID()`",
        ));
    }

    for m in SPLIT_AMP_RE.find_iter(ctx.source) {
        if ctx.in_comment_or_string(m.start()) {
            continue;
        }
        let window_end = (m.end() + 400).min(ctx.source.len());
        if SPLIT_EQ_RE.is_match(&ctx.source[m.end()..window_end]) {
            let (line, col) = pos(m.start());
            out.push(Diagnostic::at_fix(
                rule,
                ctx,
                line,
                col,
                "hand-parsed query string",
                "use `new URLSearchParams(search)`",
            ));
        }
    }

    for caps in LEFT_PAD_RE.captures_iter(ctx.source) {
        let whole = caps.get(0).unwrap();
        if ctx.in_comment_or_string(whole.start()) {
            continue;
        }
        let ident = &caps[1];
        let body = &caps[2];
        if body.contains("+=") || concat_reassigns(ident, body) {
            let (line, col) = pos(whole.start());
            out.push(Diagnostic::at_fix(
                rule,
                ctx,
                line,
                col,
                "manual left-pad loop",
                "use `String.prototype.padStart`",
            ));
        }
    }

    for node in ctx.nodes(&["new_expression"]) {
        let Some(ctor) = node.child_by_field_name("constructor") else {
            continue;
        };
        if ctx.node_text(&ctor) != "Promise" {
            continue;
        }
        if contains_call_named(ctx, node, "setTimeout") {
            let (line, col) = ctx.pos(&node);
            out.push(Diagnostic::at_fix(
                rule,
                ctx,
                line,
                col,
                "promise-wrapped `setTimeout` used as a sleep",
                "use `setTimeout` from `node:timers/promises`",
            ));
        }
    }
}

/// Cheap textual check for `<ident> = ...+...` (no regex compile per call): find `ident`, then
/// the next `=`, then a `+` before the statement's `;` (or end of the captured body slice).
fn concat_reassigns(ident: &str, body: &str) -> bool {
    let Some(after_ident) = body.split_once(ident).map(|(_, rest)| rest) else {
        return false;
    };
    let Some(rhs) = after_ident.split_once('=').map(|(_, rest)| rest) else {
        return false;
    };
    let stop = rhs.find(';').unwrap_or(rhs.len());
    rhs[..stop].contains('+')
}

fn contains_call_named(ctx: &LintContext, node: Node, name: &str) -> bool {
    if node.kind() == "call_expression" {
        if let Some(f) = node.child_by_field_name("function") {
            if ctx.node_text(&f) == name {
                return true;
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if contains_call_named(ctx, child, name) {
            return true;
        }
    }
    false
}

// --- Python ---

fn check_python(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&["for_statement"]) {
        check_python_range_len(rule, ctx, node, out);
    }
    for node in ctx.nodes(&["call"]) {
        check_python_open_chain(rule, ctx, node, out);
    }
    for node in ctx.nodes(&["function_definition"]) {
        check_python_deepcopy_def(rule, ctx, node, out);
    }
    for node in ctx.nodes(&["if_statement"]) {
        check_python_defaultdict(rule, ctx, node, out);
    }
}

fn check_python_range_len(
    rule: &'static RuleDef,
    ctx: &LintContext,
    node: Node,
    out: &mut Vec<Diagnostic>,
) {
    let Some(right) = node.child_by_field_name("right") else {
        return;
    };
    if right.kind() != "call" {
        return;
    }
    let Some(func) = right.child_by_field_name("function") else {
        return;
    };
    if ctx.node_text(&func) != "range" {
        return;
    }
    let Some(args) = right.child_by_field_name("arguments") else {
        return;
    };
    let mut cursor = args.walk();
    let named: Vec<Node> = args.named_children(&mut cursor).collect();
    if named.len() != 1 || named[0].kind() != "call" {
        return;
    }
    let Some(inner_func) = named[0].child_by_field_name("function") else {
        return;
    };
    if ctx.node_text(&inner_func) != "len" {
        return;
    }
    let (line, col) = ctx.pos(&node);
    out.push(Diagnostic::at_fix(
        rule,
        ctx,
        line,
        col,
        "manual index loop over `range(len(...))`",
        "iterate directly, or use `enumerate` when you need the index",
    ));
}

fn check_python_open_chain(
    rule: &'static RuleDef,
    ctx: &LintContext,
    node: Node,
    out: &mut Vec<Diagnostic>,
) {
    let Some(func) = node.child_by_field_name("function") else {
        return;
    };
    if func.kind() != "attribute" {
        return;
    }
    let Some(attr) = func.child_by_field_name("attribute") else {
        return;
    };
    let attr_text = ctx.node_text(&attr);
    if attr_text != "read" && attr_text != "write" {
        return;
    }
    let Some(object) = func.child_by_field_name("object") else {
        return;
    };
    if object.kind() != "call" {
        return;
    }
    let Some(obj_func) = object.child_by_field_name("function") else {
        return;
    };
    if ctx.node_text(&obj_func) != "open" {
        return;
    }
    let (line, col) = ctx.pos(&node);
    out.push(Diagnostic::at_fix(
        rule,
        ctx,
        line,
        col,
        "file opened and used without a context manager",
        "use `pathlib.Path.read_text`/`write_text`, or a `with` block",
    ));
}

fn check_python_deepcopy_def(
    rule: &'static RuleDef,
    ctx: &LintContext,
    node: Node,
    out: &mut Vec<Diagnostic>,
) {
    let Some(name) = node.child_by_field_name("name") else {
        return;
    };
    let text = ctx.node_text(&name);
    if text != "deep_copy" && text != "deepcopy" {
        return;
    }
    let (line, col) = ctx.pos(&node);
    out.push(Diagnostic::at_fix(
        rule,
        ctx,
        line,
        col,
        "hand-written deep copy",
        "use `copy.deepcopy`",
    ));
}

fn check_python_defaultdict(
    rule: &'static RuleDef,
    ctx: &LintContext,
    node: Node,
    out: &mut Vec<Diagnostic>,
) {
    let Some(cond) = node.child_by_field_name("condition") else {
        return;
    };
    if cond.kind() != "comparison_operator" || !ctx.node_text(&cond).contains("not in") {
        return;
    }
    let mut cursor = cond.walk();
    let operands: Vec<Node> = cond.named_children(&mut cursor).collect();
    if operands.len() != 2 {
        return;
    }
    let k_text = ctx.node_text(&operands[0]);
    let d_text = ctx.node_text(&operands[1]);

    let Some(body) = node.child_by_field_name("consequence") else {
        return;
    };
    let mut bc = body.walk();
    let stmts: Vec<Node> = body.named_children(&mut bc).collect();
    let Some(first) = stmts.first() else {
        return;
    };
    if first.kind() != "expression_statement" {
        return;
    }
    let Some(assign) = first.named_child(0) else {
        return;
    };
    if assign.kind() != "assignment" {
        return;
    }
    let Some(left) = assign.child_by_field_name("left") else {
        return;
    };
    if left.kind() != "subscript" {
        return;
    }
    let Some(value_node) = left.child_by_field_name("value") else {
        return;
    };
    if ctx.node_text(&value_node) != d_text {
        return;
    }
    let mut sc = left.walk();
    let subs: Vec<Node> = left.children_by_field_name("subscript", &mut sc).collect();
    if subs.len() != 1 || ctx.node_text(&subs[0]) != k_text {
        return;
    }
    let Some(right) = assign.child_by_field_name("right") else {
        return;
    };
    let is_empty_list = right.kind() == "list" && right.named_child_count() == 0;
    let is_zero = right.kind() == "integer" && ctx.node_text(&right) == "0";
    if !is_empty_list && !is_zero {
        return;
    }
    let (line, col) = ctx.pos(&node);
    out.push(Diagnostic::at_fix(
        rule,
        ctx,
        line,
        col,
        "manual accumulator init guarded by a membership check",
        "use `collections.defaultdict` or `collections.Counter`",
    ));
}

// --- Go ---

fn check_go(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&["call_expression"]) {
        let Some(func) = node.child_by_field_name("function") else {
            continue;
        };
        if func.kind() != "selector_expression" {
            continue;
        }
        let Some(operand) = func.child_by_field_name("operand") else {
            continue;
        };
        if ctx.node_text(&operand) != "ioutil" {
            continue;
        }
        let Some(field) = func.child_by_field_name("field") else {
            continue;
        };
        let field_text = ctx.node_text(&field);
        if !matches!(
            field_text,
            "ReadFile" | "WriteFile" | "ReadAll" | "TempFile" | "TempDir"
        ) {
            continue;
        }
        let (line, col) = ctx.pos(&node);
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            format!("`ioutil.{field_text}` has a direct `os`/`io` replacement"),
            "use `os.ReadFile`, `os.WriteFile`, `io.ReadAll`, or `os.CreateTemp`",
        ));
    }
}

// --- All languages: bespoke email-validation regex ---

// A hand-character-classed `@`-matcher, e.g. `[^\s@]+@[^\s@]+` or `[A-Za-z0-9._%+-]+@...`.
static CHAR_CLASS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\^?[^\]\r\n]{1,40}\]").unwrap());

fn looks_like_email_regex(text: &str) -> bool {
    if !text.contains('@') {
        return false;
    }
    CHAR_CLASS_RE
        .find_iter(text)
        .any(|m| m.as_str().contains("\\s@") || m.as_str().contains("A-Za-z0-9"))
}

/// Python/Go/Rust have no regex-literal syntax: a hand-rolled email pattern in those languages
/// is necessarily a string handed to `re.compile`/`regexp.MustCompile`/`Regex::new`, so this
/// scans the already-extracted string nodes (skipping `is_doc` docstrings, same exemption
/// `SLOP009` uses) rather than `ctx.source` + `in_comment_or_string` — that combo would
/// self-defeat here, since a string literal's own content always reads back as "inside a
/// string". TS/TSX additionally gets a real `/regex/` literal via the AST's `regex` node kind.
fn check_email_regex(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for s in ctx.strings {
        if s.is_doc {
            continue;
        }
        if looks_like_email_regex(s.text) {
            out.push(Diagnostic::at_fix(
                rule,
                ctx,
                s.line,
                s.col,
                "bespoke email-validation regex",
                "check for a single `@`, then confirm by sending mail",
            ));
        }
    }
    if matches!(ctx.lang, Lang::Ts | Lang::Tsx) {
        for node in ctx.nodes(&["regex"]) {
            if looks_like_email_regex(ctx.node_text(&node)) {
                let (line, col) = ctx.pos(&node);
                out.push(Diagnostic::at_fix(
                    rule,
                    ctx,
                    line,
                    col,
                    "bespoke email-validation regex",
                    "check for a single `@`, then confirm by sending mail",
                ));
            }
        }
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
            image: None,
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    // --- TS ---

    #[test]
    fn ts_json_clone_flagged() {
        let src = "const b = JSON.parse(JSON.stringify(a));\n";
        assert_eq!(lint(Lang::Ts, src).len(), 1);
    }

    #[test]
    fn ts_positions_count_chars_after_multibyte_text_and_on_the_last_line() {
        let src = "// café — notes\nconst b = JSON.parse(JSON.stringify(a));\nconst id = Math.random().toString(36);";
        let found = lint(Lang::Ts, src);
        let positions: Vec<(usize, usize)> = found.iter().map(|d| (d.line, d.col)).collect();
        assert_eq!(positions, vec![(2, 11), (3, 12)]);
    }

    #[test]
    fn ts_structured_clone_clean() {
        assert_eq!(lint(Lang::Ts, "const b = structuredClone(a);\n").len(), 0);
    }

    #[test]
    fn ts_random_id_flagged() {
        let src = "const id = Math.random().toString(36);\n";
        assert_eq!(lint(Lang::Ts, src).len(), 1);
    }

    #[test]
    fn ts_random_uuid_clean() {
        assert_eq!(lint(Lang::Ts, "const id = crypto.randomUUID();\n").len(), 0);
    }

    #[test]
    fn ts_query_split_flagged() {
        let src = "const parts = search.split('&');\nconst kv = parts[0].split('=');\n";
        assert_eq!(lint(Lang::Ts, src).len(), 1);
    }

    #[test]
    fn ts_url_search_params_clean() {
        assert_eq!(
            lint(Lang::Ts, "const p = new URLSearchParams(search);\n").len(),
            0
        );
    }

    #[test]
    fn ts_left_pad_flagged() {
        let src = "while (str.length < width) { str = '0' + str; }\n";
        assert_eq!(lint(Lang::Ts, src).len(), 1);
    }

    #[test]
    fn ts_pad_start_clean() {
        assert_eq!(
            lint(Lang::Ts, "const s = str.padStart(width, '0');\n").len(),
            0
        );
    }

    #[test]
    fn ts_sleep_promise_flagged() {
        let src = "const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));\n";
        assert_eq!(lint(Lang::Ts, src).len(), 1);
    }

    #[test]
    fn ts_promise_without_settimeout_clean() {
        let src = "const p = new Promise((resolve, reject) => { doWork(resolve, reject); });\n";
        assert_eq!(lint(Lang::Ts, src).len(), 0);
    }

    #[test]
    fn ts_email_regex_literal_flagged() {
        let src = "const re = /^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+$/;\n";
        assert_eq!(lint(Lang::Ts, src).len(), 1);
    }

    #[test]
    fn ts_comment_mention_not_flagged() {
        let src = "// avoid JSON.parse(JSON.stringify(x)) for cloning\n";
        assert_eq!(lint(Lang::Ts, src).len(), 0);
    }

    // --- Python ---

    #[test]
    fn python_range_len_flagged() {
        let src = "def f(xs):\n    for i in range(len(xs)):\n        print(xs[i])\n";
        assert_eq!(lint(Lang::Python, src).len(), 1);
    }

    #[test]
    fn python_enumerate_clean() {
        let src = "def f(xs):\n    for i, x in enumerate(xs):\n        print(i, x)\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
    }

    #[test]
    fn python_open_read_chain_flagged() {
        assert_eq!(lint(Lang::Python, "data = open('f.txt').read()\n").len(), 1);
    }

    #[test]
    fn python_with_open_clean() {
        let src = "with open('f.txt') as fh:\n    data = fh.read()\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
    }

    #[test]
    fn python_deepcopy_def_flagged() {
        let src = "def deep_copy(obj):\n    return obj\n";
        assert_eq!(lint(Lang::Python, src).len(), 1);
    }

    #[test]
    fn python_copy_deepcopy_clean() {
        let src = "import copy\ndef f(obj):\n    return copy.deepcopy(obj)\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
    }

    #[test]
    fn python_defaultdict_guard_flagged() {
        let src = "def f(groups, k):\n    if k not in groups:\n        groups[k] = []\n";
        assert_eq!(lint(Lang::Python, src).len(), 1);
    }

    #[test]
    fn python_defaultdict_clean() {
        let src = "from collections import defaultdict\ndef f():\n    groups = defaultdict(list)\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
    }

    #[test]
    fn python_email_regex_string_flagged() {
        let src = "import re\nPATTERN = re.compile(r\"[^\\s@]+@[^\\s@]+\")\n";
        assert_eq!(lint(Lang::Python, src).len(), 1);
    }

    #[test]
    fn python_docstring_example_not_flagged() {
        let src = "def f():\n    \"\"\"Matches [^\\s@]+@[^\\s@]+ for docs only.\"\"\"\n    pass\n";
        assert_eq!(lint(Lang::Python, src).len(), 0);
    }

    // --- Go ---

    #[test]
    fn go_ioutil_readfile_flagged() {
        let src = "package main\nimport \"io/ioutil\"\nfunc f() {\n\tioutil.ReadFile(\"x\")\n}\n";
        assert_eq!(lint(Lang::Go, src).len(), 1);
    }

    #[test]
    fn go_os_readfile_clean() {
        let src = "package main\nimport \"os\"\nfunc f() {\n\tos.ReadFile(\"x\")\n}\n";
        assert_eq!(lint(Lang::Go, src).len(), 0);
    }

    // --- Rust: only the shared email-regex pattern applies ---

    #[test]
    fn rust_email_regex_string_flagged() {
        let src = r#"fn f() { Regex::new(r"[^\s@]+@[^\s@]+").unwrap(); }"#;
        assert_eq!(lint(Lang::Rust, src).len(), 1);
    }

    #[test]
    fn rust_unrelated_regex_clean() {
        let src = r#"fn f() { Regex::new(r"^\d{3}-\d{4}$").unwrap(); }"#;
        assert_eq!(lint(Lang::Rust, src).len(), 0);
    }
}
