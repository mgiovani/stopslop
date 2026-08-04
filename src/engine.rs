use crate::{
    context::{self, LintContext},
    diagnostic::Diagnostic,
    imports_data::DepIndex,
    lang::{self, Lang},
    paths,
    registry::RULES,
    suppress,
};
use std::collections::HashSet;
use tree_sitter::Parser;

pub struct Settings {
    pub enabled: HashSet<&'static str>, // rule codes to run
    pub deps: Option<DepIndex>,         // Some under --check-imports
}

/// select resets base; ignore subtracts; a pattern matches by exact code OR prefix ("SLOP" = all).
pub fn resolve_enabled(
    select: &[String],
    ignore: &[String],
    check_imports: bool,
) -> HashSet<&'static str> {
    // Group names expand to their member codes first, so everything downstream only ever sees
    // codes and prefixes -- `--select rhetoric` and `--select SLOP014,SLOP017,...` are one path.
    let (select, ignore) = (
        &crate::groups::expand(select),
        &crate::groups::expand(ignore),
    );
    let m = |code: &str, pats: &[String]| {
        pats.iter()
            .any(|p| code == p || code.starts_with(p.as_str()))
    };
    for p in unmatched_patterns(select) {
        eprintln!("stopslop: warning: --select pattern '{p}' matched no rule code");
    }
    for p in unmatched_patterns(ignore) {
        eprintln!("stopslop: warning: --ignore pattern '{p}' matched no rule code");
    }
    let mut set: HashSet<&'static str> = if select.is_empty() {
        RULES
            .iter()
            .filter(|r| r.default_on)
            .map(|r| r.code)
            .collect()
    } else {
        RULES
            .iter()
            .filter(|r| m(r.code, select))
            .map(|r| r.code)
            .collect()
    };
    if check_imports {
        set.insert("SLOP010");
    } else {
        set.remove("SLOP010");
    }
    for r in RULES {
        if m(r.code, ignore) {
            set.remove(r.code);
        }
    }
    set
}

/// Patterns that match no rule code — a typo (e.g. "SLOP0001") would otherwise silently
/// match nothing and the CLI's whole enabled set could end up empty with no diagnostic.
fn unmatched_patterns(pats: &[String]) -> Vec<&String> {
    pats.iter()
        .filter(|p| {
            !RULES
                .iter()
                .any(|r| r.code == p.as_str() || r.code.starts_with(p.as_str()))
        })
        .collect()
}

pub fn lint_file(
    display_path: String,
    source: &str,
    lang: Lang,
    settings: &Settings,
) -> Vec<Diagnostic> {
    if lang.is_prose() {
        return lint_prose(display_path, source, lang, settings);
    }
    let mut parser = Parser::new();
    if parser.set_language(&lang::ts_language(lang)).is_err() {
        return Vec::new();
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => {
            eprintln!("stopslop: parse failed, skipping {display_path}");
            return Vec::new();
        }
    };
    // NOTE: a tree WITH error nodes is normal and expected (stray fences etc.) — DO NOT skip it.
    let (comments, strings) = context::extract(&tree, source, lang);
    let is_test = paths::is_test_path(&display_path);
    let is_stub = display_path.ends_with(".pyi");
    let ctx = LintContext {
        display_path,
        source,
        tree: Some(&tree),
        lang,
        comments: &comments,
        strings: &strings,
        is_test_path: is_test,
        is_stub_file: is_stub,
        deps: settings.deps.as_ref(),
        prose: None,
    };
    let mut out = Vec::new();
    for &rule in RULES {
        if !settings.enabled.contains(rule.code) {
            continue;
        }
        if !rule.langs.contains(&lang) {
            continue;
        }
        if rule.path_gated && ctx.is_test_path {
            continue;
        }
        (rule.check)(rule, &ctx, &mut out);
    }
    suppress::apply(&mut out, &comments);
    out
}

/// Prose langs (.md/.mdx/.txt/.rst) skip tree-sitter entirely: there is no grammar for them, and
/// prose rules scan `ProseDoc::masked` (a byte-preserving fenced/inline-code-blanked stream) instead
/// of an AST. `ctx.tree` stays `None`; `ctx.prose` is the only thing prose rules read.
fn lint_prose(
    display_path: String,
    source: &str,
    lang: Lang,
    settings: &Settings,
) -> Vec<Diagnostic> {
    let doc = crate::prose::ProseDoc::parse(source);
    let is_test = paths::is_test_path(&display_path);
    let ctx = LintContext {
        display_path,
        source,
        tree: None,
        lang,
        comments: &doc.ignore_comments, // only used by suppress::apply; prose rules read ctx.prose
        strings: &[],
        is_test_path: is_test,
        is_stub_file: false,
        deps: None,
        prose: Some(&doc),
    };
    let mut out = Vec::new();
    for &rule in RULES {
        if !settings.enabled.contains(rule.code) {
            continue;
        }
        if !rule.langs.contains(&lang) {
            continue;
        }
        if rule.path_gated && ctx.is_test_path {
            continue;
        }
        (rule.check)(rule, &ctx, &mut out);
    }
    suppress::apply(&mut out, ctx.comments);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unmatched_patterns_flags_typo() {
        assert_eq!(
            unmatched_patterns(&["SLOP0001".to_string()]),
            vec!["SLOP0001"]
        );
        assert!(unmatched_patterns(&["SLOP001".to_string()]).is_empty());
        assert!(unmatched_patterns(&["SLOP".to_string()]).is_empty()); // prefix match
    }
}
