use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;
use crate::rules::imports_data::{self, DepIndex};
use tree_sitter::Node;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP010",
    name: "Unresolved package import",
    tier: Tier::B,
    langs: &[Lang::Ts, Lang::Tsx, Lang::Python, Lang::Go, Lang::Rust],
    default_on: false,
    path_gated: true,
    check,
};

fn msg(name: &str) -> String {
    format!("package '{name}' not found in project dependencies or stdlib")
}

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    // Guarded twice on purpose: engine only calls rules with settings.enabled containing the
    // code, which is only inserted under --check-imports (see engine::resolve_enabled) — but
    // ctx.deps is the authoritative signal per PLAN §4 SLOP010, so check it directly too.
    let Some(deps) = ctx.deps else {
        return;
    };
    match ctx.lang {
        Lang::Python => check_python(rule, ctx, deps, out),
        Lang::Ts | Lang::Tsx => check_ts(rule, ctx, deps, out),
        Lang::Go => check_go(rule, ctx, deps, out),
        Lang::Rust => check_rust(rule, ctx, deps, out),
        Lang::Md | Lang::Mdx | Lang::Txt | Lang::Rst => {} // rule.langs excludes prose; never reached
    }
}

// --- Python: `import x[.y][ as z]`, `import a, b`, `from x import y` ---

fn check_python(
    rule: &'static RuleDef,
    ctx: &LintContext,
    deps: &DepIndex,
    out: &mut Vec<Diagnostic>,
) {
    if deps.python.is_empty() {
        return; // no manifest found: never FP
    }
    ctx.walk(|node| match node.kind() {
        "import_statement" => {
            let mut cursor = node.walk();
            for name_node in node.children_by_field_name("name", &mut cursor) {
                if let Some(top) = python_top_segment(name_node, ctx) {
                    flag_python(rule, ctx, deps, name_node, top, out);
                }
            }
        }
        "import_from_statement" => {
            if let Some(module_node) = node.child_by_field_name("module_name") {
                // relative_import (leading dots, e.g. `from . import x`) -> always skip.
                if module_node.kind() == "dotted_name" {
                    if let Some(top) = first_identifier_text(module_node, ctx) {
                        flag_python(rule, ctx, deps, module_node, top, out);
                    }
                }
            }
        }
        _ => {}
    });
}

fn python_top_segment<'a>(name_node: Node, ctx: &LintContext<'a>) -> Option<&'a str> {
    match name_node.kind() {
        "dotted_name" => first_identifier_text(name_node, ctx),
        "aliased_import" => name_node
            .child_by_field_name("name")
            .and_then(|n| first_identifier_text(n, ctx)),
        _ => None,
    }
}

fn first_identifier_text<'a>(dotted_name: Node, ctx: &LintContext<'a>) -> Option<&'a str> {
    Some(ctx.node_text(&dotted_name.named_child(0)?))
}

fn flag_python(
    rule: &'static RuleDef,
    ctx: &LintContext,
    deps: &DepIndex,
    node: Node,
    name: &str,
    out: &mut Vec<Diagnostic>,
) {
    if imports_data::python_resolved(deps, name) {
        return;
    }
    if is_import_error_guarded(node, ctx) {
        return; // `try: import x / except ImportError:` = optional-dependency idiom, not slop
    }
    let (line, col) = ctx.pos(&node);
    out.push(Diagnostic::at(rule, ctx, line, col, msg(name)));
}

/// True if `node` sits inside a `try` block whose `try_statement` has an `except` clause
/// catching `ImportError`/`ModuleNotFoundError` (or a bare `except:`) — the standard
/// optional-dependency idiom.
fn is_import_error_guarded(node: Node, ctx: &LintContext) -> bool {
    let mut cur = node.parent();
    while let Some(n) = cur {
        if n.kind() == "try_statement" {
            let mut c = n.walk();
            for child in n.children(&mut c) {
                if child.kind() != "except_clause" {
                    continue;
                }
                // except_clause's named children are [type?, body]; a bare `except:` has
                // only the body block, so a single named child means no type was given.
                let mut nc = child.walk();
                let named: Vec<Node> = child.named_children(&mut nc).collect();
                if named.len() <= 1 {
                    return true; // bare `except:`
                }
                let text = ctx.node_text(&named[0]);
                if text.contains("ImportError") || text.contains("ModuleNotFoundError") {
                    return true;
                }
            }
        }
        cur = n.parent();
    }
    false
}

// --- TS/TSX: `import ... from 'source'` ---

fn check_ts(rule: &'static RuleDef, ctx: &LintContext, deps: &DepIndex, out: &mut Vec<Diagnostic>) {
    if deps.ts.is_empty() {
        return;
    }
    ctx.walk(|node| {
        if node.kind() != "import_statement" {
            return;
        }
        let Some(source_node) = node.child_by_field_name("source") else {
            return;
        };
        let raw = ctx.node_text(&source_node);
        let path = raw.trim_matches(|c| c == '\'' || c == '"' || c == '`');
        if !imports_data::ts_resolved(deps, path) {
            let (line, col) = ctx.pos(&source_node);
            let name = imports_data::ts_package_name(path);
            out.push(Diagnostic::at(rule, ctx, line, col, msg(&name)));
        }
    });
}

// --- Go: `import_spec` (covers both single and grouped `import (...)` forms) ---

fn check_go(rule: &'static RuleDef, ctx: &LintContext, deps: &DepIndex, out: &mut Vec<Diagnostic>) {
    if deps.go.is_empty() {
        return;
    }
    ctx.walk(|node| {
        if node.kind() != "import_spec" {
            return;
        }
        let Some(path_node) = node.child_by_field_name("path") else {
            return;
        };
        let raw = ctx.node_text(&path_node);
        let path = raw.trim_matches(|c| c == '"' || c == '`');
        if !imports_data::go_resolved(deps, path) {
            let (line, col) = ctx.pos(&node);
            out.push(Diagnostic::at(rule, ctx, line, col, msg(path)));
        }
    });
}

// --- Rust: `use path::to::Item;`, `extern crate name;` ---

fn check_rust(
    rule: &'static RuleDef,
    ctx: &LintContext,
    deps: &DepIndex,
    out: &mut Vec<Diagnostic>,
) {
    if deps.rust.is_empty() {
        return;
    }
    let locals = local_item_names(ctx);
    ctx.walk(|node| match node.kind() {
        "use_declaration" => {
            if let Some(arg) = node.child_by_field_name("argument") {
                if let Some(name) = rust_first_segment(arg, ctx) {
                    if !locals.contains(name) {
                        flag_rust(rule, ctx, deps, node, name, out);
                    }
                }
            }
        }
        "extern_crate_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = ctx.node_text(&name_node);
                flag_rust(rule, ctx, deps, node, name, out);
            }
        }
        _ => {}
    });
}

/// Top-level item names declared in this file (enum/struct/union/type alias/mod). A bare
/// `use LocalEnum::*;` brings a same-crate item's members into scope and is not an external
/// package import — even though its leftmost segment looks just like a crate name.
fn local_item_names<'a>(ctx: &LintContext<'a>) -> std::collections::HashSet<&'a str> {
    let mut names = std::collections::HashSet::new();
    ctx.walk(|node| {
        if matches!(
            node.kind(),
            "enum_item" | "struct_item" | "union_item" | "type_item" | "mod_item"
        ) {
            if let Some(name_node) = node.child_by_field_name("name") {
                names.insert(ctx.node_text(&name_node));
            }
        }
    });
    names
}

/// Leftmost segment of a `use` argument. Returns `None` (skip) for `crate`/`self`/`super`
/// roots and other shapes we don't resolve (metavariables, malformed trees).
fn rust_first_segment<'a>(node: Node, ctx: &LintContext<'a>) -> Option<&'a str> {
    match node.kind() {
        "identifier" => Some(ctx.node_text(&node)),
        "scoped_identifier" | "scoped_use_list" | "use_as_clause" => node
            .child_by_field_name("path")
            .and_then(|p| rust_first_segment(p, ctx)),
        "use_wildcard" => node.named_child(0).and_then(|c| rust_first_segment(c, ctx)),
        _ => None,
    }
}

fn flag_rust(
    rule: &'static RuleDef,
    ctx: &LintContext,
    deps: &DepIndex,
    node: Node,
    name: &str,
    out: &mut Vec<Diagnostic>,
) {
    if imports_data::rust_resolved(deps, name) {
        return;
    }
    let (line, col) = ctx.pos(&node);
    out.push(Diagnostic::at(rule, ctx, line, col, msg(name)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
    use std::collections::HashSet;
    use tree_sitter::Parser;

    fn lint(lang: Lang, src: &str, deps: &DepIndex) -> Vec<Diagnostic> {
        let mut p = Parser::new();
        p.set_language(&crate::lang::ts_language(lang)).unwrap();
        let tree = p.parse(src, None).unwrap();
        let (comments, strings) = context::extract(&tree, src, lang);
        let ctx = LintContext {
            display_path: "t".into(),
            source: src,
            tree: Some(&tree),
            lang,
            comments: &comments,
            strings: &strings,
            is_test_path: false,
            is_stub_file: false,
            deps: Some(deps),
            prose: None,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    // --- Python ---

    #[test]
    fn python_unresolved_flagged() {
        let deps = DepIndex {
            python: set(&["requests"]),
            ..Default::default()
        };
        assert_eq!(lint(Lang::Python, "import fastapi_auth\n", &deps).len(), 1);
    }

    #[test]
    fn python_stdlib_clean() {
        let deps = DepIndex {
            python: set(&["requests"]),
            ..Default::default()
        };
        assert_eq!(
            lint(Lang::Python, "import os\nimport os.path\n", &deps).len(),
            0
        );
    }

    #[test]
    fn python_declared_clean() {
        let deps = DepIndex {
            python: set(&["requests"]),
            ..Default::default()
        };
        assert_eq!(lint(Lang::Python, "import requests\n", &deps).len(), 0);
    }

    #[test]
    fn python_alias_pil_clean() {
        let deps = DepIndex {
            python: set(&["pillow"]),
            ..Default::default()
        };
        assert_eq!(lint(Lang::Python, "import PIL\n", &deps).len(), 0);
    }

    #[test]
    fn python_alias_google_cloud_clean() {
        let deps = DepIndex {
            python: set(&["google-cloud-storage"]),
            ..Default::default()
        };
        assert_eq!(
            lint(Lang::Python, "import google.cloud.storage\n", &deps).len(),
            0
        );
    }

    #[test]
    fn python_pep503_underscore_hyphen_clean() {
        let deps = DepIndex {
            python: set(&["scikit-learn"]),
            ..Default::default()
        };
        assert_eq!(lint(Lang::Python, "import sklearn\n", &deps).len(), 0);
        let deps2 = DepIndex {
            python: set(&["python-dateutil"]), // pep503-normalized form (as discover() stores it)
            ..Default::default()
        };
        assert_eq!(lint(Lang::Python, "import dateutil\n", &deps2).len(), 0);
    }

    #[test]
    fn python_relative_import_clean() {
        let deps = DepIndex {
            python: set(&["requests"]),
            ..Default::default()
        };
        assert_eq!(
            lint(
                Lang::Python,
                "from . import foo\nfrom .bar import baz\n",
                &deps
            )
            .len(),
            0
        );
    }

    #[test]
    fn python_from_import_flags() {
        let deps = DepIndex {
            python: set(&["requests"]),
            ..Default::default()
        };
        assert_eq!(
            lint(Lang::Python, "from totally_fake_pkg import thing\n", &deps).len(),
            1
        );
    }

    #[test]
    fn python_import_error_guarded_clean() {
        let deps = DepIndex {
            python: set(&["requests"]),
            ..Default::default()
        };
        let src = "try:\n    import simplejson as json\nexcept ImportError:\n    import json\n";
        assert_eq!(lint(Lang::Python, src, &deps).len(), 0);
        let src_bare = "try:\n    import fastapi_auth\nexcept:\n    pass\n";
        assert_eq!(lint(Lang::Python, src_bare, &deps).len(), 0);
        // Sanity: the same import outside a try/except is still flagged.
        assert_eq!(lint(Lang::Python, "import fastapi_auth\n", &deps).len(), 1);
    }

    #[test]
    fn python_empty_deps_skips_entirely() {
        assert_eq!(
            lint(Lang::Python, "import fastapi_auth\n", &DepIndex::empty()).len(),
            0
        );
    }

    // --- TS ---

    #[test]
    fn ts_unresolved_flagged() {
        let deps = DepIndex {
            ts: set(&["react"]),
            ..Default::default()
        };
        assert_eq!(
            lint(Lang::Ts, "import x from 'fastapi-auth-js';\n", &deps).len(),
            1
        );
    }

    #[test]
    fn ts_declared_clean() {
        let deps = DepIndex {
            ts: set(&["react"]),
            ..Default::default()
        };
        assert_eq!(lint(Lang::Ts, "import x from 'react';\n", &deps).len(), 0);
    }

    #[test]
    fn ts_scoped_package_clean() {
        let deps = DepIndex {
            ts: set(&["@babel/core"]),
            ..Default::default()
        };
        assert_eq!(
            lint(Lang::Ts, "import { a } from '@babel/core';\n", &deps).len(),
            0
        );
    }

    #[test]
    fn ts_relative_import_clean() {
        let deps = DepIndex {
            ts: set(&["react"]),
            ..Default::default()
        };
        assert_eq!(
            lint(
                Lang::Ts,
                "import './local';\nimport y from '../local2';\n",
                &deps
            )
            .len(),
            0
        );
    }

    #[test]
    fn ts_node_builtin_clean() {
        let deps = DepIndex {
            ts: set(&["react"]),
            ..Default::default()
        };
        assert_eq!(
            lint(Lang::Ts, "import fs from 'node:fs';\n", &deps).len(),
            0
        );
        assert_eq!(lint(Lang::Ts, "import fs from 'fs';\n", &deps).len(), 0);
    }

    #[test]
    fn ts_path_alias_clean() {
        let deps = DepIndex {
            ts: set(&["react"]),
            ts_path_aliases: set(&["@app"]),
            ..Default::default()
        };
        assert_eq!(
            lint(Lang::Ts, "import { a } from '@app/components';\n", &deps).len(),
            0
        );
    }

    #[test]
    fn ts_empty_deps_skips_entirely() {
        assert_eq!(
            lint(
                Lang::Ts,
                "import x from 'fastapi-auth-js';\n",
                &DepIndex::empty()
            )
            .len(),
            0
        );
    }

    // --- Go ---

    #[test]
    fn go_stdlib_clean() {
        let deps = DepIndex {
            go: set(&["github.com/foo/bar"]),
            ..Default::default()
        };
        assert_eq!(
            lint(Lang::Go, "package main\nimport \"fmt\"\n", &deps).len(),
            0
        );
    }

    #[test]
    fn go_declared_clean() {
        let deps = DepIndex {
            go: set(&["github.com/foo/bar"]),
            ..Default::default()
        };
        assert_eq!(
            lint(
                Lang::Go,
                "package main\nimport \"github.com/foo/bar\"\n",
                &deps
            )
            .len(),
            0
        );
    }

    #[test]
    fn go_declared_subpackage_clean() {
        let deps = DepIndex {
            go: set(&["github.com/foo/bar"]),
            ..Default::default()
        };
        assert_eq!(
            lint(
                Lang::Go,
                "package main\nimport \"github.com/foo/bar/sub\"\n",
                &deps
            )
            .len(),
            0
        );
    }

    #[test]
    fn go_unresolved_flagged() {
        let deps = DepIndex {
            go: set(&["github.com/foo/bar"]),
            ..Default::default()
        };
        assert_eq!(
            lint(
                Lang::Go,
                "package main\nimport \"github.com/totally/fake\"\n",
                &deps
            )
            .len(),
            1
        );
    }

    #[test]
    fn go_grouped_imports() {
        let deps = DepIndex {
            go: set(&["github.com/foo/bar"]),
            ..Default::default()
        };
        let src = "package main\nimport (\n\t\"os\"\n\t\"github.com/totally/fake\"\n)\n";
        assert_eq!(lint(Lang::Go, src, &deps).len(), 1);
    }

    #[test]
    fn go_empty_deps_skips_entirely() {
        assert_eq!(
            lint(
                Lang::Go,
                "package main\nimport \"github.com/totally/fake\"\n",
                &DepIndex::empty()
            )
            .len(),
            0
        );
    }

    // --- Rust ---

    #[test]
    fn rust_builtin_clean() {
        let deps = DepIndex {
            rust: set(&["serde"]),
            ..Default::default()
        };
        assert_eq!(lint(Lang::Rust, "use std::io;\n", &deps).len(), 0);
    }

    #[test]
    fn rust_declared_clean() {
        let deps = DepIndex {
            rust: set(&["serde"]),
            ..Default::default()
        };
        assert_eq!(lint(Lang::Rust, "use serde::Serialize;\n", &deps).len(), 0);
    }

    #[test]
    fn rust_hyphen_underscore_clean() {
        let deps = DepIndex {
            rust: set(&["serde_json"]), // declared as `serde-json` in Cargo.toml, normalized
            ..Default::default()
        };
        assert_eq!(lint(Lang::Rust, "use serde_json::Value;\n", &deps).len(), 0);
    }

    #[test]
    fn rust_crate_self_super_clean() {
        let deps = DepIndex {
            rust: set(&["serde"]),
            ..Default::default()
        };
        assert_eq!(
            lint(
                Lang::Rust,
                "use crate::foo;\nuse self::bar;\nuse super::baz;\n",
                &deps
            )
            .len(),
            0
        );
    }

    #[test]
    fn rust_unresolved_flagged() {
        let deps = DepIndex {
            rust: set(&["serde"]),
            ..Default::default()
        };
        assert_eq!(
            lint(Lang::Rust, "use totally_fake_crate::Thing;\n", &deps).len(),
            1
        );
    }

    #[test]
    fn rust_extern_crate_flagged() {
        let deps = DepIndex {
            rust: set(&["serde"]),
            ..Default::default()
        };
        assert_eq!(
            lint(Lang::Rust, "extern crate totally_fake_crate;\n", &deps).len(),
            1
        );
    }

    #[test]
    fn rust_local_enum_glob_import_clean() {
        let deps = DepIndex {
            rust: set(&["serde"]),
            ..Default::default()
        };
        let src = "enum FastMatchResult { A, B }\nfn f() {\n    use FastMatchResult::*;\n}\n";
        assert_eq!(lint(Lang::Rust, src, &deps).len(), 0);
    }

    #[test]
    fn rust_empty_deps_skips_entirely() {
        assert_eq!(
            lint(
                Lang::Rust,
                "use totally_fake_crate::Thing;\n",
                &DepIndex::empty()
            )
            .len(),
            0
        );
    }

    // --- fixture-dir integration: real manifests under tests/fixtures/imports/<lang>, driven
    // through the actual engine::lint_file pipeline (not the bare `check` fn above). Display
    // paths are stripped of the "tests/fixtures/imports" prefix (mirrors tests/integration.rs)
    // so is_test_path() sees only the logical "<lang>/slop_x.ext" name — SLOP010 is path_gated,
    // and "tests"/"fixtures" are both exempt directory segments (see PLAN §5b), so leaving the
    // real repo-relative path in would silently skip the rule. Documented in the final report.
    fn fixtures_dir(lang: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/imports")
            .join(lang)
    }

    fn lint_fixture(lang: Lang, dir_lang: &str, file: &str) -> Vec<Diagnostic> {
        let dir = fixtures_dir(dir_lang);
        let deps = DepIndex::discover(std::slice::from_ref(&dir));
        let source = std::fs::read_to_string(dir.join(file)).unwrap();
        let settings = crate::engine::Settings {
            enabled: crate::engine::resolve_enabled(&[], &[], true),
            deps: Some(deps),
        };
        crate::engine::lint_file(format!("{dir_lang}/{file}"), &source, lang, &settings)
    }

    #[test]
    fn fixture_python_slop_flags_only_unresolved() {
        let diags = lint_fixture(Lang::Python, "python", "slop_import.py");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP010");
        assert_eq!(diags[0].line, 3);
    }

    #[test]
    fn fixture_python_clean_flags_nothing() {
        assert_eq!(
            lint_fixture(Lang::Python, "python", "clean_import.py").len(),
            0
        );
    }

    #[test]
    fn fixture_node_slop_flags_only_unresolved() {
        let diags = lint_fixture(Lang::Ts, "node", "slop.ts");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 2);
    }

    #[test]
    fn fixture_node_clean_flags_nothing() {
        assert_eq!(lint_fixture(Lang::Ts, "node", "clean.ts").len(), 0);
    }

    #[test]
    fn fixture_go_slop_flags_only_unresolved() {
        let diags = lint_fixture(Lang::Go, "go", "slop.go");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 5);
    }

    #[test]
    fn fixture_go_clean_flags_nothing() {
        assert_eq!(lint_fixture(Lang::Go, "go", "clean.go").len(), 0);
    }

    #[test]
    fn fixture_rust_slop_flags_only_unresolved() {
        let diags = lint_fixture(Lang::Rust, "rust", "slop.rs");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 3);
    }

    #[test]
    fn fixture_rust_clean_flags_nothing() {
        assert_eq!(lint_fixture(Lang::Rust, "rust", "clean.rs").len(), 0);
    }
}
