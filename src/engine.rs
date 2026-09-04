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
    pub natlangs: Vec<lang::NatLang>,   // resolved from config; default is every supported language
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

/// Compiles every enabled prose rule's regex statics on its own thread while the caller walks
/// the tree: that one-time compile tax (~24 ms serially) is most of a run over a few files.
/// AST rules are skipped; their compile cost is in the noise next to the walk (issue #21).
pub fn prewarm<'scope>(scope: &'scope std::thread::Scope<'scope, '_>, settings: &Settings) {
    for &rule in RULES {
        if settings.enabled.contains(rule.code) && rule.langs.contains(&Lang::Md) {
            let natlangs = settings.natlangs.clone();
            scope.spawn(move || {
                let one = Settings {
                    enabled: HashSet::from([rule.code]),
                    deps: None,
                    custom_rules: Vec::new(),
                    natlangs,
                };
                lint_file(String::new(), "Warm-up line.\n", Lang::Md, &one);
            });
        }
    }
}

/// The rule-dispatch loop shared by `lint_file`, `lint_prose` and `lint_image`: filter `RULES` by
/// enabled/lang/natlang/path-gating, run each one, then the custom-rule second pass, then
/// `suppress::apply`. Previously pasted identically into `lint_file` and `lint_prose`; folding
/// `suppress::apply` in too is safe because every caller already passes `ctx.comments` as the
/// comment slice it reads (`lint_image`'s is empty, making that call a no-op there -- a binary
/// file has no comment syntax to carry an `ai-slop-ignore` directive in).
fn run_rules(ctx: &LintContext, settings: &Settings) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for &rule in RULES {
        if !settings.enabled.contains(rule.code) {
            continue;
        }
        if !rule.langs.contains(&ctx.lang) {
            continue;
        }
        // No-op under the default (every language); bites only when config sets `language`.
        if !rule.natlangs.iter().any(|n| ctx.natlangs.contains(n)) {
            continue;
        }
        if rule.path_gated && ctx.is_test_path {
            continue;
        }
        (rule.check)(rule, ctx, &mut out);
    }
    for cr in &settings.custom_rules {
        if settings.enabled.contains(cr.code()) {
            crate::custom::check(cr, ctx, &mut out);
        }
    }
    suppress::apply(&mut out, ctx.comments, &ctx.display_path);
    out
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
    // Images go through `lint_image` (bytes in, no tree, no text stream), reached directly from
    // `walk.rs`. This fn's signature is frozen for its existing `&str`-source callers, so it
    // returns empty rather than growing a bytes parameter no code-or-prose caller needs.
    if lang.is_image() {
        return Vec::new();
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
        image: None,
        natlangs: &settings.natlangs,
    };
    run_rules(&ctx, settings)
}

/// Prose langs never build a `NodeIndex`: prose rules scan `ProseDoc::masked` (a byte-preserving
/// stream with code blanked, or for HTML with everything but visible text blanked) instead of an
/// AST. `ctx.index` stays `None`; `ctx.prose` is the only thing prose rules read.
///
/// `natlangs` is run-wide by default (`settings.natlangs`, resolved once from config), but an
/// HTML page can narrow it for itself: `doc.html_lang` is `Some` only when its own `<html lang>`
/// names a language the run already has enabled, in which case the diagnostics for this ONE file
/// are computed as if only that language were configured. A declared language the config
/// excludes leaves the run-wide set unchanged -- config is explicit, the hint only ever narrows a
/// set the config already allows, it never adds a language config turned off. Under the default
/// union this makes `<html lang="pt-BR">` skip that file's English-only rules (SLOP014, SLOP032,
/// ...) while `ALL_NATLANGS` rules (SLOP018's em dash, SLOP033's sentence length, ...) and
/// bilingual rules run exactly as before -- and it feeds SLOP033's Portuguese sentence-length cap
/// (`sentence_length::OVERLONG_WORDS_PT_BR`), since that only activates when the file's own
/// `natlangs` resolves to Portuguese alone.
fn lint_prose(
    display_path: String,
    source: &str,
    lang: Lang,
    settings: &Settings,
) -> Vec<Diagnostic> {
    let doc = match lang {
        Lang::Html => crate::prose::ProseDoc::parse_html(source),
        _ => crate::prose::ProseDoc::parse(source),
    };
    let narrowed;
    let natlangs: &[lang::NatLang] = match doc.html_lang {
        Some(nl) if settings.natlangs.contains(&nl) => {
            narrowed = [nl];
            &narrowed
        }
        _ => &settings.natlangs,
    };
    let is_test = paths::is_test_path(&display_path);
    let ctx = LintContext {
        display_path,
        source,
        index: None,
        lang,
        comments: &doc.ignore_comments, // only used by suppress::apply; prose rules read ctx.prose
        strings: &doc.attr_values,      // HTML attributes; empty for the Markdown family
        is_test_path: is_test,
        is_stub_file: false,
        deps: None,
        prose: Some(&doc),
        image: None,
        natlangs,
    };
    run_rules(&ctx, settings)
}

/// Byte-oriented entry point for `Lang::Image`, reached directly from `walk.rs` (never through
/// `lint_file`, which returns empty for this lang instead). `source` is the empty string because
/// an image has no text stream to speak of; image rules read `ctx.image` and nothing else.
pub fn lint_image(display_path: String, bytes: &[u8], settings: &Settings) -> Vec<Diagnostic> {
    let Some(doc) = crate::image::ImageDoc::parse(bytes) else {
        return Vec::new();
    };
    let is_test = paths::is_test_path(&display_path);
    let ctx = LintContext {
        display_path,
        source: "",
        index: None,
        lang: Lang::Image,
        comments: &[],
        strings: &[],
        is_test_path: is_test,
        is_stub_file: false,
        deps: None,
        prose: None,
        image: Some(&doc),
        natlangs: &settings.natlangs,
    };
    run_rules(&ctx, settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prewarm_runs_every_rule_on_the_warm_up_document() {
        let settings = Settings {
            enabled: RULES.iter().map(|r| r.code).collect(),
            deps: None,
            custom_rules: Vec::new(),
            natlangs: lang::ALL_NATLANGS.to_vec(),
        };
        std::thread::scope(|scope| prewarm(scope, &settings));
    }

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

    /// `language = "pt-BR"` gates out an English-lexicon rule while a lexicon-free rule keeps
    /// firing on the same document; the default runs both.
    #[test]
    fn natlangs_setting_gates_english_only_rules() {
        let enabled = resolve_enabled(
            &["SLOP018".to_string(), "SLOP023".to_string()],
            &[],
            &[],
            &[],
            &[],
            false,
        );
        let settings = |natlangs: Vec<lang::NatLang>| Settings {
            enabled: enabled.clone(),
            deps: None,
            custom_rules: Vec::new(),
            natlangs,
        };
        let src = "It's not about speed, it's about accuracy \u{2014} both matter.\n";
        let codes = |s: &Settings| {
            let mut v: Vec<&str> = lint_file("f.md".to_string(), src, Lang::Md, s)
                .into_iter()
                .map(|d| d.code)
                .collect();
            v.sort_unstable();
            v
        };
        assert_eq!(
            codes(&settings(lang::ALL_NATLANGS.to_vec())),
            vec!["SLOP018", "SLOP023"]
        );
        assert_eq!(codes(&settings(vec![lang::NatLang::PtBr])), vec!["SLOP018"]);
    }

    /// `doc.html_lang` narrows `ctx.natlangs` for one HTML file, independent of the run-wide
    /// setting: SLOP032 (`natlangs: &[NatLang::En]`) is still genuinely English-only after phase
    /// 3 (unlike SLOP018's em dash or SLOP033's sentence length, both `ALL_NATLANGS` and thus
    /// unaffected by this narrowing either way), so it's the rule that demonstrates the outer
    /// `rule.natlangs ∩ ctx.natlangs` gate actually failing shut.
    #[test]
    fn html_lang_narrows_natlangs_per_file_but_config_wins() {
        let enabled = resolve_enabled(&["SLOP032".to_string()], &[], &[], &[], &[], false);
        let body = |lang_attr: &str| {
            format!(
                "<html lang=\"{lang_attr}\"><body><p>A real-time view helps here. \
                 The real-time system stays in sync. Another real-time check follows.</p>\
                 </body></html>\n"
            )
        };
        let settings = |natlangs: Vec<lang::NatLang>| Settings {
            enabled: enabled.clone(),
            deps: None,
            custom_rules: Vec::new(),
            natlangs,
        };
        let fires = |src: &str, s: &Settings| {
            !lint_file("f.html".to_string(), src, Lang::Html, s).is_empty()
        };

        // Default union narrows to Portuguese for this file -> SLOP032's English-only gate fails.
        assert!(!fires(
            &body("pt-BR"),
            &settings(lang::ALL_NATLANGS.to_vec())
        ));
        // Declaring English narrows to English -> the rule's own gate passes and it fires.
        assert!(fires(&body("en"), &settings(lang::ALL_NATLANGS.to_vec())));
        // Config restricted to English excludes the page's declared Portuguese, so the hint
        // can't narrow into a language the config already turned off -- the run-wide set (just
        // English) is used unchanged, and the rule still fires.
        assert!(fires(&body("pt-BR"), &settings(vec![lang::NatLang::En])));
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
            natlangs: lang::ALL_NATLANGS.to_vec(),
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
