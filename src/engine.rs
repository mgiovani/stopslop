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
    pub custom_rules: Vec<crate::custom::CustomRule>, // [[custom-rule]] entries, run as a 2nd pass
}

/// select resets base; extend-select unions on top of it (config AND CLI, never replacing);
/// ignore/extend-ignore subtract last. A pattern matches by exact code OR prefix ("SLOP" = all).
/// Mirrors Ruff's select/extend-select/ignore/extend-ignore composition.
pub fn resolve_enabled(
    select: &[String],
    extend_select: &[String],
    ignore: &[String],
    extend_ignore: &[String],
    custom_codes: &[&'static str],
    check_imports: bool,
) -> HashSet<&'static str> {
    // Groups expand to member codes first, so downstream only sees codes/prefixes.
    // `groups::expand`'s `ALL` only knows the static `RULES` table, so custom codes are
    // appended here for that one selector.
    let expand = |pats: &[String]| -> Vec<String> {
        let mut out = crate::groups::expand(pats);
        if pats.iter().any(|p| p == "ALL") {
            out.extend(custom_codes.iter().map(|c| c.to_string()));
        }
        out
    };
    let select = &expand(select);
    let extend_select = &expand(extend_select);
    let ignore = &expand(ignore);
    let extend_ignore = &expand(extend_ignore);

    let m = |code: &str, pats: &[String]| {
        pats.iter()
            .any(|p| code == p || code.starts_with(p.as_str()))
    };
    for (label, pats) in [
        ("--select", select),
        ("extend-select", extend_select),
        ("--ignore", ignore),
        ("extend-ignore", extend_ignore),
    ] {
        for p in unmatched_patterns(pats, custom_codes) {
            eprintln!("stopslop: warning: {label} pattern '{p}' matched no rule code");
        }
    }

    let all_codes = || {
        RULES
            .iter()
            .map(|r| r.code)
            .chain(custom_codes.iter().copied())
    };

    let mut set: HashSet<&'static str> = if select.is_empty() {
        // Custom rules are on by default -- the user explicitly wrote them.
        RULES
            .iter()
            .filter(|r| r.default_on)
            .map(|r| r.code)
            .chain(custom_codes.iter().copied())
            .collect()
    } else {
        all_codes().filter(|&c| m(c, select)).collect()
    };
    set.extend(all_codes().filter(|&c| m(c, extend_select)));

    if check_imports {
        set.insert("SLOP010");
    } else {
        set.remove("SLOP010");
    }

    for c in all_codes() {
        if m(c, ignore) || m(c, extend_ignore) {
            set.remove(c);
        }
    }
    set
}

/// Patterns that match no rule code (static or custom) — a typo (e.g. "SLOP0001") would
/// otherwise silently match nothing and the CLI's whole enabled set could end up empty with no
/// diagnostic.
fn unmatched_patterns<'a>(pats: &'a [String], custom_codes: &[&'static str]) -> Vec<&'a String> {
    pats.iter()
        .filter(|p| {
            !RULES
                .iter()
                .any(|r| r.code == p.as_str() || r.code.starts_with(p.as_str()))
                && !custom_codes
                    .iter()
                    .any(|c| *c == p.as_str() || c.starts_with(p.as_str()))
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
    let (comments, strings, index) = context::extract(&tree, source, lang);
    let is_test = paths::is_test_path(&display_path);
    let is_stub = display_path.ends_with(".pyi");
    let ctx = LintContext {
        display_path,
        source,
        index: Some(&index),
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
    for cr in &settings.custom_rules {
        if settings.enabled.contains(cr.code()) {
            crate::custom::check(cr, &ctx, &mut out);
        }
    }
    suppress::apply(&mut out, &comments, &ctx.display_path);
    out
}

/// Prose langs (.md/.mdx/.txt/.rst) skip tree-sitter entirely: there is no grammar for them, and
/// prose rules scan `ProseDoc::masked` (a byte-preserving fenced/inline-code-blanked stream) instead
/// of an AST. `ctx.index` stays `None`; `ctx.prose` is the only thing prose rules read.
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
        index: None,
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
    for cr in &settings.custom_rules {
        if settings.enabled.contains(cr.code()) {
            crate::custom::check(cr, &ctx, &mut out);
        }
    }
    suppress::apply(&mut out, ctx.comments, &ctx.display_path);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unmatched_patterns_flags_typo() {
        assert_eq!(
            unmatched_patterns(&["SLOP0001".to_string()], &[]),
            vec!["SLOP0001"]
        );
        assert!(unmatched_patterns(&["SLOP001".to_string()], &[]).is_empty());
        assert!(unmatched_patterns(&["SLOP".to_string()], &[]).is_empty()); // prefix match
    }

    #[test]
    fn unmatched_patterns_recognizes_custom_codes() {
        assert!(unmatched_patterns(&["SLOP900".to_string()], &["SLOP900"]).is_empty());
    }

    /// `resolve_enabled` expands groups (incl. `ALL`) before `unmatched_patterns` ever runs, so
    /// `--select ALL` must not itself trip the "matched no rule code" warning.
    #[test]
    fn select_all_expands_before_unmatched_check() {
        let expanded = crate::groups::expand(&["ALL".to_string()]);
        assert!(unmatched_patterns(&expanded, &[]).is_empty());
    }

    /// extend-select unions on top of an explicit select rather than replacing it; extend-ignore
    /// subtracts last, after both select and extend-select are unioned in.
    #[test]
    fn extend_select_unions_and_extend_ignore_subtracts_last() {
        let set = resolve_enabled(
            &["SLOP001".to_string()],
            &["SLOP015".to_string()], // extend-select: default_on=false, opt-in density rule
            &[],
            &["SLOP001".to_string()], // extend-ignore removes what select just added
            &[],
            false,
        );
        assert!(!set.contains("SLOP001"), "extend-ignore must subtract last");
        assert!(set.contains("SLOP015"), "extend-select must union in");
    }

    /// Custom codes participate in select/ignore/extend-* uniformly, on top of static rules.
    #[test]
    fn resolve_enabled_includes_custom_codes_by_default_and_honors_ignore() {
        let custom = ["SLOP900", "SLOP901"];
        let all_on = resolve_enabled(&[], &[], &[], &[], &custom, false);
        assert!(all_on.contains("SLOP900") && all_on.contains("SLOP901"));

        let one_ignored = resolve_enabled(&[], &[], &["SLOP900".to_string()], &[], &custom, false);
        assert!(!one_ignored.contains("SLOP900"));
        assert!(one_ignored.contains("SLOP901"));
    }

    /// End-to-end: a custom rule runs through `lint_file`'s second pass and its finding is still
    /// suppressible with a rule-scoped `ai-slop-ignore: SLOP900` comment, same as any built-in
    /// rule -- `suppress::apply` runs after both passes and only ever looks at `Diagnostic.code`.
    #[test]
    fn custom_rule_finding_is_suppressible_by_code() {
        let custom_cfg = crate::config::CustomRuleConfig {
            pattern: r"(?i)(?-u:\b)synergy(?-u:\b)".to_string(),
            message: "banned word: synergy".to_string(),
            tier: "B".to_string(),
            fix: None,
            files: Vec::new(),
        };
        let custom_rules = crate::custom::load(&[custom_cfg]).unwrap();
        let custom_codes: Vec<&'static str> = custom_rules
            .iter()
            .map(crate::custom::CustomRule::code)
            .collect();
        let enabled = resolve_enabled(&[], &[], &[], &[], &custom_codes, false);
        let settings = Settings {
            enabled,
            deps: None,
            custom_rules,
        };

        let unsuppressed = "We need synergy here.\n";
        let diags = lint_file("f.md".to_string(), unsuppressed, Lang::Md, &settings);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP900");

        let suppressed = "We need synergy here. <!-- ai-slop-ignore: SLOP900 -->\n";
        let diags = lint_file("f.md".to_string(), suppressed, Lang::Md, &settings);
        assert!(diags.is_empty());
    }
}
