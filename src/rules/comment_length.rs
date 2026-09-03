use crate::context::{self, LintContext, TextNode};
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, Lang};
use crate::registry::RuleDef;
use crate::suppress::comment_body;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP043",
    name: "Comment that runs long",
    tier: Tier::B,
    langs: &[Lang::Ts, Lang::Tsx, Lang::Python, Lang::Go, Lang::Rust],
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: true,
    check,
};

// Steidl, Hummel & Jürgens (ICPC 2013): developers keep 30-plus-word inline comments for their
// global information, which belongs in doc comments and READMEs. Measured here: 6-10% of human
// plain comment blocks exceed 40 words; this repo was at 21%.
const MAX_WORDS: usize = 40;
const FIX: &str = "keep the reason in a sentence or two; the rest belongs in a doc comment, the \
README or the commit message";

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let comments = ctx.comments;
    if is_generated(ctx) {
        return;
    }
    let bodies = if ctx.lang == Lang::Go {
        function_bodies(ctx)
    } else {
        Vec::new()
    };
    let mut i = 0;
    while i < comments.len() {
        let head = &comments[i];
        if head.is_doc || (ctx.lang == Lang::Go && !inside(&bodies, head)) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < comments.len() {
            let next = &comments[j + 1];
            let continues = !next.is_doc
                && next.line == end_line(&comments[j]) + 1
                && is_leading(ctx, &comments[j])
                && is_leading(ctx, next);
            if !continues {
                break;
            }
            j += 1;
        }
        let block = &comments[i..=j];
        let words: usize = block.iter().map(|c| word_count(c.text)).sum();
        if words > MAX_WORDS && !is_exempt(block) {
            let msg = format!("comment runs {words} words");
            out.push(Diagnostic::at_fix(rule, ctx, head.line, head.col, msg, FIX));
        }
        i = j + 1;
    }
}

/// Go has no doc-comment syntax: a `//` outside a function body (on a type, a field, a grouped
/// const or var) is godoc, so only comments inside function bodies are judged there.
fn function_bodies(ctx: &LintContext) -> Vec<(usize, usize)> {
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for n in ctx.nodes(&["function_declaration", "method_declaration", "func_literal"]) {
        let (start, end) = (n.start_byte(), n.end_byte());
        if ranges.last().is_some_and(|&(_, prev_end)| end <= prev_end) {
            continue;
        }
        ranges.push((start, end));
    }
    ranges
}

fn inside(ranges: &[(usize, usize)], c: &TextNode) -> bool {
    let i = ranges.partition_point(|&(start, _)| start <= c.start_byte);
    i > 0 && c.end_byte <= ranges[i - 1].1
}

/// The `DO NOT EDIT` / `@generated` marker may follow a license header (protoc-gen-go does), so
/// every comment ahead of the first line of code is checked.
fn is_generated(ctx: &LintContext) -> bool {
    let first_code = first_code_byte(ctx.source);
    ctx.comments
        .iter()
        .take_while(|c| c.start_byte < first_code)
        .any(|c| {
            let t = c.text.to_lowercase();
            t.contains("do not edit") || t.contains("@generated")
        })
}

fn first_code_byte(source: &str) -> usize {
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let t = line.trim_start();
        let header = t.is_empty()
            || t.starts_with("//")
            || t.starts_with('#')
            || t.starts_with("/*")
            || t.starts_with('*');
        if !header {
            return offset;
        }
        offset += line.len();
    }
    offset
}

fn end_line(c: &TextNode) -> usize {
    c.line + c.text.matches('\n').count()
}

fn is_leading(ctx: &LintContext, c: &TextNode) -> bool {
    context::is_leading(ctx.source, c.start_byte, c.col - 1)
}

/// A license header is boilerplate nobody wrote for this file, and commented-out code isn't
/// prose (it fails the symbol-density test the same way a line of code would).
fn is_exempt(block: &[TextNode]) -> bool {
    let body: String = block
        .iter()
        .map(|c| body_of(c.text))
        .collect::<Vec<_>>()
        .join("\n");
    let lower = body.to_lowercase();
    if block[0].line <= 3
        && (lower.contains("copyright") || lower.contains("license") || lower.contains("spdx"))
    {
        return true;
    }
    let non_ws: Vec<char> = body.chars().filter(|c| !c.is_whitespace()).collect();
    let symbols = non_ws
        .iter()
        .filter(|c| !(c.is_alphanumeric() || **c == '_'))
        .count();
    symbols * 4 > non_ws.len()
}

fn body_of(raw: &str) -> String {
    let body = comment_body(raw);
    if !raw.trim_start().starts_with("/*") {
        return body.to_string();
    }
    body.strip_suffix("*/")
        .unwrap_or(body)
        .lines()
        .map(|l| l.trim().strip_prefix('*').map_or(l.trim(), str::trim))
        .collect::<Vec<_>>()
        .join("\n")
}

fn word_count(raw: &str) -> usize {
    body_of(raw)
        .split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
    use tree_sitter::Parser;

    fn run(lang: Lang, src: &str) -> Vec<Diagnostic> {
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

    fn words(n: usize) -> String {
        (0..n)
            .map(|i| format!("w{i}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn block_over_the_cap_flagged_once_at_its_first_line() {
        let src = format!(
            "fn f() {{\n    // {}\n    // {}\n    g();\n}}\n",
            words(25),
            words(20)
        );
        let d = run(Lang::Rust, &src);
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].line, d[0].col), (2, 5));
        assert_eq!(d[0].message, "comment runs 45 words");
    }

    #[test]
    fn block_at_the_cap_not_flagged() {
        let src = format!("fn f() {{\n    // {}\n    g();\n}}\n", words(MAX_WORDS));
        assert!(run(Lang::Rust, &src).is_empty());
    }

    #[test]
    fn separate_short_blocks_are_not_summed() {
        let src = format!(
            "fn f() {{\n    // {}\n    g();\n    // {}\n    h();\n}}\n",
            words(25),
            words(25)
        );
        assert!(run(Lang::Rust, &src).is_empty());
    }

    #[test]
    fn trailing_comment_does_not_merge_with_the_next_line() {
        let src = format!(
            "fn f() {{\n    g(); // {}\n    // {}\n    h();\n}}\n",
            words(25),
            words(25)
        );
        assert!(run(Lang::Rust, &src).is_empty());
    }

    #[test]
    fn doc_comments_are_out_of_scope() {
        let src = format!("/// {}\nfn f() {{}}\n", words(60));
        assert!(run(Lang::Rust, &src).is_empty());
        let src = format!("def f():\n    \"\"\"{}\"\"\"\n", words(60));
        assert!(run(Lang::Python, &src).is_empty());
    }

    #[test]
    fn block_comment_counts_its_words_once() {
        let src = format!(
            "fn f() {{\n    /*\n     * {}\n     * {}\n     */\n    g();\n}}\n",
            words(30),
            words(20)
        );
        let d = run(Lang::Rust, &src);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].message, "comment runs 50 words");
    }

    #[test]
    fn license_header_exempt() {
        let src = format!("// Copyright 2024 Someone\n// {}\nfn f() {{}}\n", words(60));
        assert!(run(Lang::Rust, &src).is_empty());
    }

    #[test]
    fn commented_out_code_exempt() {
        let code = "// let x = foo(a, b) + bar(c[0], d.e) * 2;\n".repeat(12);
        let src = format!("fn f() {{\n{code}    g();\n}}\n");
        assert!(run(Lang::Rust, &src).is_empty());
    }

    #[test]
    fn go_comment_outside_a_function_body_is_godoc() {
        let src = format!(
            "package main\n\nvar (\n\t// {}\n\tErrX = errors.New(\"x\")\n)\n\ntype T struct {{\n\t// {}\n\tF int\n}}\n",
            words(60),
            words(60)
        );
        assert!(run(Lang::Go, &src).is_empty());
        let src = format!(
            "package main\n\nvar f = func() {{\n\t// {}\n\tg()\n}}\n",
            words(60)
        );
        assert_eq!(run(Lang::Go, &src).len(), 1);
    }

    #[test]
    fn generated_file_exempt() {
        let src = format!(
            "// Code generated by gen.go. DO NOT EDIT.\n\npackage main\n\nfunc f() {{\n\t// {}\n\tg()\n}}\n",
            words(60)
        );
        assert!(run(Lang::Go, &src).is_empty());
        let src = format!(
            "// Copyright 2024 Example Corp.\n\n// Code generated by protoc-gen-go. DO NOT EDIT.\n\npackage main\n\nfunc f() {{\n\t// {}\n\tg()\n}}\n",
            words(60)
        );
        assert!(run(Lang::Go, &src).is_empty());
        let src = format!(
            "package main\n\n// DO NOT EDIT this function by hand.\nfunc f() {{\n\t// {}\n\tg()\n}}\n",
            words(60)
        );
        assert_eq!(run(Lang::Go, &src).len(), 1);
    }

    #[test]
    fn python_and_go_blocks_counted() {
        let src = format!(
            "def f():\n    # {}\n    # {}\n    pass\n",
            words(30),
            words(20)
        );
        assert_eq!(run(Lang::Python, &src).len(), 1);
        let src = format!(
            "package main\nfunc f() {{\n\t// {}\n\t// {}\n\tg()\n}}\n",
            words(30),
            words(20)
        );
        assert_eq!(run(Lang::Go, &src).len(), 1);
    }
}
