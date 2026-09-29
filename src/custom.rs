//! User-defined phrase and whole-file rules loaded from `[[custom-rule]]` config entries
//! (house-specific banned words and file limits no one wants to write a Rust module for). Codes
//! are auto-assigned SLOP900, SLOP901, ... in declaration order; `groups::group_of` special-cases the SLOP9 prefix as "custom" the same
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
    // Carries code/name/tier so `Diagnostic::at`/`at_fix` work unmodified; other fields go
    // unused since custom rules dispatch through `engine`'s own pass, not `RULES`, with
    // `files`/`ctx.prose` deciding language scope. cheaper than a second
    // `Diagnostic::at` shape.
    def: RuleDef,
    message: &'static str,
    fix: Option<&'static str>,
    matcher: Matcher,
    files: Option<GlobSet>, // None = every supported lang
}

enum Matcher {
    Phrase(Regex),
    MaxLines(usize),
    Forbid(Regex),
    Require(Regex),
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
    let (matcher, label) = build_matcher(index, c)?;
    let Some(tier) = Tier::parse(&c.tier) else {
        anyhow::bail!(
            "custom-rule[{index}] ({label}): invalid tier {:?}, expected \"A\", \"B\" or \"C\"",
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
                    "custom-rule[{index}] ({label}): invalid files glob {glob_pat:?}: {e}"
                )
            })?;
            builder.add(glob);
        }
        Some(builder.build().map_err(|e| {
            anyhow::anyhow!("custom-rule[{index}] ({label}): invalid files glob set: {e}")
        })?)
    };

    // Config loads once per process, so leaking to `&'static str` is free -- never repeated,
    // freed only at exit. `Cow<'static, str>` is the alternative, but touches every
    // rule module for no benefit.
    let code: &'static str = Box::leak(format!("SLOP{}", 900 + index).into_boxed_str());
    let name: &'static str = Box::leak(format!("custom rule: {label}").into_boxed_str());
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
            // Custom rules are user regexes, not a lexicon -- engine's natlang gate never
            // consults this field for them (see engine::lint_file/lint_prose's 2nd pass).
            natlangs: crate::lang::ALL_NATLANGS,
            default_on: true,
            path_gated: false,
            check: noop_check,
        },
        message,
        fix,
        matcher,
        files,
    })
}

fn compile(index: usize, label: &str, pattern: &str) -> anyhow::Result<Regex> {
    Regex::new(pattern)
        .map_err(|e| anyhow::anyhow!("custom-rule[{index}] ({label}): invalid regex: {e}"))
}

/// Validates the `kind`/predicate combination and returns the matcher plus a label naming the
/// entry in errors and `--list-rules`.
fn build_matcher(index: usize, c: &CustomRuleConfig) -> anyhow::Result<(Matcher, String)> {
    let predicates = [
        ("max-lines", c.max_lines.is_some()),
        ("forbid", c.forbid.is_some()),
        ("require", c.require.is_some()),
    ];
    match c.kind.as_deref().unwrap_or("phrase") {
        "phrase" => {
            let Some(pattern) = &c.pattern else {
                anyhow::bail!("custom-rule[{index}]: kind \"phrase\" requires `pattern`")
            };
            let label = format!("pattern {pattern:?}");
            if let Some((key, _)) = predicates.iter().find(|(_, set)| *set) {
                anyhow::bail!("custom-rule[{index}] ({label}): `{key}` needs kind = \"file\"")
            }
            Ok((Matcher::Phrase(compile(index, &label, pattern)?), label))
        }
        "file" => {
            if c.pattern.is_some() {
                anyhow::bail!("custom-rule[{index}]: kind \"file\" rejects `pattern`; use `forbid`")
            }
            if c.files.is_empty() {
                anyhow::bail!("custom-rule[{index}]: kind \"file\" requires `files`")
            }
            let set: Vec<_> = predicates.iter().filter(|(_, set)| *set).collect();
            if set.len() != 1 {
                anyhow::bail!(
                    "custom-rule[{index}]: kind \"file\" needs exactly one of `max-lines`, `forbid`, `require`, found {}",
                    set.len()
                )
            }
            match (c.max_lines, &c.forbid, &c.require) {
                (Some(0), ..) => anyhow::bail!("custom-rule[{index}]: `max-lines` must be > 0"),
                (Some(n), ..) => Ok((Matcher::MaxLines(n), format!("max-lines {n}"))),
                (_, Some(p), _) => {
                    let label = format!("forbid {p:?}");
                    Ok((Matcher::Forbid(compile(index, &label, p)?), label))
                }
                (_, _, Some(p)) => {
                    let label = format!("require {p:?}");
                    Ok((Matcher::Require(compile(index, &label, p)?), label))
                }
                _ => unreachable!("exactly one predicate is set"),
            }
        }
        other => anyhow::bail!(
            "custom-rule[{index}]: invalid kind {other:?}, expected \"phrase\" or \"file\""
        ),
    }
}

fn matches_path(rule: &CustomRule, display_path: &str) -> bool {
    let path = crate::paths::strip_dot_slash(display_path);
    rule.files.as_ref().is_none_or(|g| g.is_match(path))
}

/// Phrase rules: prose langs scan `ProseDoc::masked` (fenced/inline code already blanked), same
/// as every built-in prose rule, so a custom phrase inside a code fence never fires. Code langs
/// scan `ctx.comments`/`ctx.strings`, same idiom as `rules::placeholder`/`rules::type_escape`:
/// one diagnostic per matching node, anchored at the node's own start. File rules read the raw
/// `ctx.source` and yield at most one diagnostic per file.
pub fn check(rule: &CustomRule, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    if !matches_path(rule, &ctx.display_path) {
        return;
    }
    match &rule.matcher {
        Matcher::Phrase(re) => check_phrase(rule, re, ctx, out),
        Matcher::MaxLines(n) => {
            if ctx.source.lines().count() > *n {
                push(rule, ctx, n + 1, 1, out);
            }
        }
        Matcher::Forbid(re) => {
            if let Some(m) = re.find(ctx.source) {
                let (line, col) = line_col(ctx.source, m.start());
                push(rule, ctx, line, col, out);
            }
        }
        Matcher::Require(re) => {
            if !re.is_match(ctx.source) {
                push(rule, ctx, 1, 1, out);
            }
        }
    }
}

fn check_phrase(rule: &CustomRule, re: &Regex, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    if let Some(doc) = ctx.prose {
        for m in re.find_iter(&doc.masked) {
            let (line, col) = doc.line_col(m.start());
            push(rule, ctx, line, col, out);
        }
        return;
    }
    for node in ctx.comments.iter().chain(ctx.strings.iter()) {
        if re.is_match(node.text) {
            push(rule, ctx, node.line, node.col, out);
        }
    }
}

/// 1-based line and char column of a byte offset.
fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, col)
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
            pattern: Some(pattern.to_string()),
            message: message.to_string(),
            tier: "B".to_string(),
            ..Default::default()
        }
    }

    fn file_cfg(files: &[&str]) -> CustomRuleConfig {
        CustomRuleConfig {
            kind: Some("file".to_string()),
            message: "m".to_string(),
            tier: "B".to_string(),
            files: files.iter().map(|f| f.to_string()).collect(),
            ..Default::default()
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
        c.tier = "X".to_string();
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
            index: None,
            lang: Lang::Md,
            comments: &doc.comments,
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: Some(doc),
            image: None,
            natlangs: crate::lang::ALL_NATLANGS,
        }
    }

    #[test]
    fn prose_scan_hits_masked_stream_and_skips_fences() {
        let rules = load(&[cfg(r"(?i)(?-u:\b)synergy(?-u:\b)", "banned word: synergy")]).unwrap();
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
        let mut c = cfg(r"(?i)(?-u:\b)synergy(?-u:\b)", "banned word: synergy");
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
            let mut c = cfg(r"(?i)(?-u:\b)synergy(?-u:\b)", "banned word: synergy");
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
        let mut c = cfg(r"(?i)(?-u:\b)synergy(?-u:\b)", "banned word: synergy");
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

    fn code_ctx<'a>(source: &'a str, path: &str) -> LintContext<'a> {
        LintContext {
            display_path: path.to_string(),
            source,
            index: None,
            lang: Lang::Python,
            comments: &[],
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: None,
            image: None,
            natlangs: crate::lang::ALL_NATLANGS,
        }
    }

    fn run(c: CustomRuleConfig, source: &str, path: &str) -> Vec<(usize, usize)> {
        let rules = load(&[c]).unwrap();
        let mut out = Vec::new();
        check(&rules[0], &code_ctx(source, path), &mut out);
        out.iter().map(|d| (d.line, d.col)).collect()
    }

    #[test]
    fn file_rule_config_errors_name_the_entry() {
        let mut both = file_cfg(&["**/*.py"]);
        both.max_lines = Some(3);
        both.forbid = Some("x".into());
        let mut with_pattern = file_cfg(&["**/*.py"]);
        with_pattern.forbid = Some("x".into());
        with_pattern.pattern = Some("y".into());
        let mut bad_regex = file_cfg(&["**/*.py"]);
        bad_regex.require = Some("(".into());
        let mut zero = file_cfg(&["**/*.py"]);
        zero.max_lines = Some(0);
        let mut no_files = file_cfg(&[]);
        no_files.max_lines = Some(3);
        let mut phrase_with_predicate = cfg("x", "m");
        phrase_with_predicate.max_lines = Some(3);
        let mut bad_kind = cfg("x", "m");
        bad_kind.kind = Some("line".into());
        let phrase_missing = CustomRuleConfig {
            message: "m".into(),
            tier: "B".into(),
            ..Default::default()
        };
        for c in [
            file_cfg(&["**/*.py"]),
            both,
            with_pattern,
            bad_regex,
            zero,
            no_files,
            phrase_with_predicate,
            bad_kind,
            phrase_missing,
        ] {
            let err = err_string(load(&[cfg("ok", "m"), c]));
            assert!(err.contains("custom-rule[1]"), "{err}");
        }
    }

    #[test]
    fn max_lines_anchors_at_first_line_over_the_limit() {
        let mut c = file_cfg(&["**/*.py"]);
        c.max_lines = Some(3);
        assert!(run(c.clone(), "a\nb\nc\n", "x.py").is_empty());
        assert_eq!(run(c.clone(), "a\nb\nc\nd\n", "x.py"), vec![(4, 1)]);
        assert_eq!(run(c, "a\nb\nc\nd\ne", "x.py"), vec![(4, 1)]);
    }

    #[test]
    fn forbid_anchors_at_first_match_and_empty_file_is_clean() {
        let mut c = file_cfg(&["**/__init__.py"]);
        c.forbid = Some(r"\S".into());
        assert!(run(c.clone(), "", "pkg/__init__.py").is_empty());
        assert_eq!(
            run(c.clone(), "\n  x = 1\ny = 2\n", "pkg/__init__.py"),
            vec![(2, 3)]
        );
        assert!(run(c, "x = 1\n", "pkg/mod.py").is_empty());
    }

    #[test]
    fn require_anchors_at_line_one_when_missing() {
        let mut c = file_cfg(&["**/test_*.py"]);
        c.require = Some(r"(?-u:\b)assert(?-u:\b)".into());
        assert_eq!(
            run(c.clone(), "def test_a():\n    pass\n", "test_a.py"),
            vec![(1, 1)]
        );
        assert!(run(c, "def test_a():\n    assert 1\n", "test_a.py").is_empty());
    }

    #[test]
    fn max_lines_fingerprint_survives_growth() {
        let mut c = file_cfg(&["**/*.py"]);
        c.max_lines = Some(3);
        let rules = load(&[c]).unwrap();
        let fp = |lines: usize| {
            let src = "x\n".repeat(lines);
            let mut out = Vec::new();
            check(&rules[0], &code_ctx(&src, "x.py"), &mut out);
            crate::baseline::fingerprint(&out[0])
        };
        assert_eq!(fp(320), fp(340));
    }
}
