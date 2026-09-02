use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, Lang};
use crate::registry::RuleDef;
use tree_sitter::Node;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP038",
    name: "Dependency with a stdlib equivalent",
    tier: Tier::B,
    langs: &[Lang::Ts, Lang::Tsx],
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: true,
    check,
};

fn fix_for(name: &str) -> Option<&'static str> {
    match name {
        "moment" => Some("use `Intl.DateTimeFormat`"),
        "uuid" => Some("use `crypto.randomUUID()`"),
        "node-fetch" | "isomorphic-fetch" | "cross-fetch" => {
            Some("use the global `fetch` (Node 18+)")
        }
        "dotenv" => Some("use `node --env-file=.env`"),
        "left-pad" | "pad-left" => Some("use `String.prototype.padStart`"),
        "object-assign" => Some("use `Object.assign`"),
        "is-array" => Some("use `Array.isArray`"),
        "querystring" => Some("use `URLSearchParams`"),
        "request" => Some("use the global `fetch`"),
        "mkdirp" => Some("use `fs.mkdir` with `{ recursive: true }`"),
        "rimraf" => Some("use `fs.rm` with `{ recursive: true, force: true }`"),
        _ => None,
    }
}

/// Leading package segment: `moment/locale/x` -> `moment`, `@scope/pkg/sub` -> `@scope/pkg`.
/// None of the mapped names above are scoped, but a relative/subpath specifier should still
/// resolve to the same leading segment as its bare form.
fn package_name(spec: &str) -> &str {
    if let Some(rest) = spec.strip_prefix('@') {
        match rest.find('/') {
            Some(i) => &spec[..i + 2],
            None => spec,
        }
    } else {
        spec.split('/').next().unwrap_or(spec)
    }
}

fn flag(
    rule: &'static RuleDef,
    ctx: &LintContext,
    node: Node,
    spec: &str,
    out: &mut Vec<Diagnostic>,
) {
    let name = package_name(spec);
    let Some(fix) = fix_for(name) else {
        return;
    };
    let (line, col) = ctx.pos(&node);
    out.push(Diagnostic::at_fix(
        rule,
        ctx,
        line,
        col,
        format!("`{name}` duplicates a feature the platform now provides"),
        fix,
    ));
}

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    for node in ctx.nodes(&["import_statement"]) {
        let Some(source) = node.child_by_field_name("source") else {
            continue;
        };
        let raw = ctx.node_text(&source);
        let spec = raw.trim_matches(|c| c == '\'' || c == '"' || c == '`');
        flag(rule, ctx, node, spec, out);
    }
    for node in ctx.nodes(&["call_expression"]) {
        let Some(func) = node.child_by_field_name("function") else {
            continue;
        };
        if ctx.node_text(&func) != "require" {
            continue;
        }
        let Some(args) = node.child_by_field_name("arguments") else {
            continue;
        };
        let mut cursor = args.walk();
        let named: Vec<Node> = args.named_children(&mut cursor).collect();
        if named.len() != 1 || named[0].kind() != "string" {
            continue;
        }
        let raw = ctx.node_text(&named[0]);
        let spec = raw.trim_matches(|c| c == '\'' || c == '"' || c == '`');
        flag(rule, ctx, node, spec, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
    use tree_sitter::Parser;

    fn lint(src: &str) -> Vec<Diagnostic> {
        let mut p = Parser::new();
        p.set_language(&crate::lang::ts_language(Lang::Ts)).unwrap();
        let tree = p.parse(src, None).unwrap();
        let (comments, strings, index) = context::extract(&tree, src, Lang::Ts);
        let ctx = LintContext {
            display_path: "t".into(),
            source: src,
            index: Some(&index),
            lang: Lang::Ts,
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
    fn moment_import_flagged() {
        assert_eq!(lint("import moment from 'moment';\n").len(), 1);
    }

    #[test]
    fn moment_subpath_flagged() {
        assert_eq!(lint("import 'moment/locale/pt-br';\n").len(), 1);
    }

    #[test]
    fn uuid_require_flagged() {
        let diags = lint("const { v4 } = require('uuid');\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].fix.as_deref(), Some("use `crypto.randomUUID()`"));
    }

    /// A non-`require` call first, then the flaggable `require`: a `continue` mistakenly turned
    /// into `return` would drop the second call too.
    #[test]
    fn non_require_call_first_uuid_require_still_flagged() {
        assert_eq!(
            lint("foo(bar);\nconst { v4 } = require('uuid');\n").len(),
            1
        );
    }

    #[test]
    fn react_import_clean() {
        assert_eq!(lint("import React from 'react';\n").len(), 0);
    }

    #[test]
    fn relative_import_clean() {
        assert_eq!(lint("import { helper } from './helper';\n").len(), 0);
    }

    #[test]
    fn all_mapped_packages_flagged() {
        let names = [
            "moment",
            "uuid",
            "node-fetch",
            "isomorphic-fetch",
            "cross-fetch",
            "dotenv",
            "left-pad",
            "pad-left",
            "object-assign",
            "is-array",
            "querystring",
            "request",
            "mkdirp",
            "rimraf",
        ];
        for name in names {
            let src = format!("import x from '{name}';\n");
            assert_eq!(lint(&src).len(), 1, "expected a finding for '{name}'");
        }
    }
}
