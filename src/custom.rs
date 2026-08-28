//! User-defined phrase rules loaded from `[[custom-rule]]` config entries (house-specific banned
//! words no one wants to write a Rust module for). Codes are auto-assigned SLOP900, SLOP901, ...
//! in declaration order; `groups::group_of` special-cases the SLOP9 prefix as "custom" the same
//! way Wave 1 special-cased `ALL` -- these codes deliberately never join the static `GROUPS`
//! table, whose `groups_partition_every_rule` test requires every member to also be in `RULES`.
//!
//! Custom rules can't live in the static `RULES` slice: `RuleDef.check` is a plain `fn` pointer,
//! and each custom rule carries its own compiled `Regex` a fn pointer can't close over. They run
//! as a second pass in `engine::lint_file`/`lint_prose`, after the static rule loop.

use crate::config::CustomRuleConfig;
use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::registry::RuleDef;
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::Regex;

pub struct CustomRule {
    // Carries code/name/tier so `Diagnostic::at`/`at_fix` (which only read those three fields)
    // work unmodified. `langs`/`default_on`/`path_gated`/`check` are never read: custom rules are
    // dispatched by `engine`'s dedicated second pass, not the `RULES` loop, and language scope is
    // decided by `files`/`ctx.prose` below instead. ponytail: a dummy `RuleDef` is smaller than
    // teaching `Diagnostic::at` a second, `CustomRule`-shaped input for three fields.
    def: RuleDef,
    message: &'static str,
    fix: Option<&'static str>,
    pattern: Regex,
    files: Option<GlobSet>, // None = every supported lang
}

impl CustomRule {
    pub fn code(&self) -> &'static str {
        self.def.code
    }
    pub fn name(&self) -> &'static str {
        self.def.name
    }
    pub fn tier(&self) -> Tier {
        self.def.tier
    }
}

fn noop_check(_: &'static RuleDef, _: &LintContext, _: &mut Vec<Diagnostic>) {}

/// Compiles every `[[custom-rule]]` entry. An invalid regex, `tier`, or `files` glob is a config
/// error (exit 2 via `anyhow`) naming the offending entry's index and pattern -- never a silent
/// skip, since a silently-dropped house rule is worse than a startup failure.
pub fn load(configs: &[CustomRuleConfig]) -> anyhow::Result<Vec<CustomRule>> {
    configs
        .iter()
        .enumerate()
        .map(|(i, c)| build_one(i, c))
        .collect()
}

fn build_one(index: usize, c: &CustomRuleConfig) -> anyhow::Result<CustomRule> {
    let pattern = Regex::new(&c.pattern).map_err(|e| {
        anyhow::anyhow!(
            "custom-rule[{index}] (pattern {:?}): invalid regex: {e}",
            c.pattern
        )
    })?;
    let Some(tier) = Tier::parse(&c.tier) else {
        anyhow::bail!(
            "custom-rule[{index}] (pattern {:?}): invalid tier {:?}, expected \"A\" or \"B\"",
            c.pattern,
            c.tier
        )
    };
    let files = if c.files.is_empty() {
        None
    } else {
        let mut builder = GlobSetBuilder::new();
        for glob_pat in &c.files {
            let glob = Glob::new(crate::paths::strip_dot_slash(glob_pat)).map_err(|e| {
                anyhow::anyhow!(
                    "custom-rule[{index}] (pattern {:?}): invalid files glob {glob_pat:?}: {e}",
                    c.pattern
                )
            })?;
            builder.add(glob);
        }
        Some(builder.build().map_err(|e| {
            anyhow::anyhow!(
                "custom-rule[{index}] (pattern {:?}): invalid files glob set: {e}",
                c.pattern
            )
        })?)
    };

    // Config loads exactly once per process, so leaking these `String`s to `&'static str` is the
    // whole cost -- never repeated, never freed until exit. ponytail: the alternative is
    // `Diagnostic::code`/`.name` becoming `Cow<'static, str>`, which touches every rule module and
    // every test helper's `Diagnostic { .. }` literal for no runtime benefit.
    let code: &'static str = Box::leak(format!("SLOP{}", 900 + index).into_boxed_str());
    let name: &'static str = Box::leak(format!("custom rule: {}", c.pattern).into_boxed_str());
    let message: &'static str = Box::leak(c.message.clone().into_boxed_str());
    let fix: Option<&'static str> = c
        .fix
        .clone()
        .map(|f| -> &'static str { Box::leak(f.into_boxed_str()) });

    Ok(CustomRule {
        def: RuleDef {
            code,
            name,
            tier,
            langs: &[],
            default_on: true,
            path_gated: false,
            check: noop_check,
        },
        message,
        fix,
        pattern,
        files,
    })
}

fn matches_path(rule: &CustomRule, display_path: &str) -> bool {
    let path = crate::paths::strip_dot_slash(display_path);
    rule.files.as_ref().is_none_or(|g| g.is_match(path))
}

/// Prose langs scan `ProseDoc::masked` (fenced/inline code already blanked), same as every
/// built-in prose rule, so a custom phrase inside a code fence never fires. Code langs scan
/// `ctx.comments`/`ctx.strings`, same idiom as `rules::placeholder`/`rules::type_escape`: one
/// diagnostic per matching node, anchored at the node's own start.
pub fn check(rule: &CustomRule, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    if !matches_path(rule, &ctx.display_path) {
        return;
    }
    if let Some(doc) = ctx.prose {
        for m in rule.pattern.find_iter(&doc.masked) {
            let (line, col) = doc.line_col(m.start());
            push(rule, ctx, line, col, out);
        }
        return;
    }
    for node in ctx.comments.iter().chain(ctx.strings.iter()) {
        if rule.pattern.is_match(node.text) {
            push(rule, ctx, node.line, node.col, out);
        }
    }
}

fn push(rule: &CustomRule, ctx: &LintContext, line: usize, col: usize, out: &mut Vec<Diagnostic>) {
    out.push(match rule.fix {
        Some(fix) => Diagnostic::at_fix(&rule.def, ctx, line, col, rule.message, fix),
        None => Diagnostic::at(&rule.def, ctx, line, col, rule.message),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;

    fn cfg(pattern: &str, message: &str) -> CustomRuleConfig {
        CustomRuleConfig {
            pattern: pattern.to_string(),
            message: message.to_string(),
            tier: "B".to_string(),
            fix: None,
            files: Vec::new(),
        }
    }

    #[test]
    fn codes_assigned_in_declaration_order() {
        let rules = load(&[cfg("a", "m1"), cfg("b", "m2"), cfg("c", "m3")]).unwrap();
        assert_eq!(
            rules.iter().map(CustomRule::code).collect::<Vec<_>>(),
            vec!["SLOP900", "SLOP901", "SLOP902"]
        );
    }

    // `CustomRule` deliberately doesn't derive `Debug` (its `Regex`/`GlobSet` fields would drag
    // that requirement everywhere for no reason), so tests can't use `unwrap_err`/`expect_err`
    // (both require `T: Debug`). This sidesteps the bound instead of adding a derive just for it.
    fn err_string(r: anyhow::Result<Vec<CustomRule>>) -> String {
        match r {
            Ok(_) => panic!("expected a config error"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn bad_regex_is_a_config_error() {
        let err = err_string(load(&[cfg("(unclosed", "m")]));
        assert!(err.contains("custom-rule[0]"));
    }

    #[test]
    fn bad_tier_is_a_config_error() {
        let mut c = cfg("x", "m");
        c.tier = "C".to_string();
        let err = err_string(load(&[c]));
        assert!(err.contains("invalid tier"));
    }

    #[test]
    fn bad_files_glob_is_a_config_error() {
        let mut c = cfg("x", "m");
        c.files = vec!["[".to_string()];
        let err = err_string(load(&[c]));
        assert!(err.contains("invalid files glob"));
    }

    fn prose_ctx<'a>(doc: &'a crate::prose::ProseDoc<'a>, path: &str) -> LintContext<'a> {
        LintContext {
            display_path: path.to_string(),
            source: "",
            tree: None,
            lang: Lang::Md,
            comments: &doc.ignore_comments,
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: Some(doc),
        }
    }

    #[test]
    fn prose_scan_hits_masked_stream_and_skips_fences() {
        let rules = load(&[cfg(r"(?i)\bsynergy\b", "banned word: synergy")]).unwrap();
        let src = "We need synergy here.\n\n```\nsynergy in code\n```\n";
        let doc = crate::prose::ProseDoc::parse(src);
        let ctx = prose_ctx(&doc, "f.md");
        let mut out = Vec::new();
        check(&rules[0], &ctx, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].code, "SLOP900");
        assert_eq!(out[0].line, 1);
    }

    #[test]
    fn files_glob_restricts_which_paths_are_scanned() {
        let mut c = cfg(r"(?i)\bsynergy\b", "banned word: synergy");
        c.files = vec!["docs/**".to_string()];
        let rules = load(&[c]).unwrap();
        let doc = crate::prose::ProseDoc::parse("synergy\n");
        let miss = prose_ctx(&doc, "other/f.md");
        let mut out = Vec::new();
        check(&rules[0], &miss, &mut out);
        assert!(out.is_empty());

        let hit = prose_ctx(&doc, "docs/f.md");
        check(&rules[0], &hit, &mut out);
        assert_eq!(out.len(), 1);
    }

    /// Same normalization `[per-file-ignores]` gets: scanning `.` prefixes display paths with
    /// `./`, and a `files` glob that only worked for one spelling of the scan target reads as
    /// the rule simply not firing.
    #[test]
    fn files_glob_ignores_dot_slash_on_either_side() {
        for (glob, path) in [
            ("docs/**", "./docs/f.md"),
            ("./docs/**", "docs/f.md"),
            ("./docs/**", "./docs/f.md"),
        ] {
            let mut c = cfg(r"(?i)\bsynergy\b", "banned word: synergy");
            c.files = vec![glob.to_string()];
            let rules = load(&[c]).unwrap();
            let doc = crate::prose::ProseDoc::parse("synergy\n");
            let mut out = Vec::new();
            check(&rules[0], &prose_ctx(&doc, path), &mut out);
            assert_eq!(out.len(), 1, "glob {glob:?} should match path {path:?}");
        }
    }

    #[test]
    fn fix_hint_is_emitted_when_configured() {
        let mut c = cfg(r"(?i)\bsynergy\b", "banned word: synergy");
        c.fix = Some("say what the teams actually do".to_string());
        let rules = load(&[c]).unwrap();
        let doc = crate::prose::ProseDoc::parse("synergy\n");
        let ctx = prose_ctx(&doc, "f.md");
        let mut out = Vec::new();
        check(&rules[0], &ctx, &mut out);
        assert_eq!(
            out[0].fix.as_deref(),
            Some("say what the teams actually do")
        );
    }
}
