use crate::{
    baseline::{self, Baseline},
    config::Config,
    custom,
    diagnostic::{Diagnostic, Tier},
    engine, groups,
    imports_data::DepIndex,
    output,
    registry::RULES,
    walk,
};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "stopslop", version, about = "Like Ruff, but for AI slop.")]
pub struct Cli {
    /// Paths to scan (default: current directory).
    pub paths: Vec<PathBuf>,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
    /// Only run these rule codes/prefixes/groups (resets defaults). Comma-separated or repeated.
    #[arg(long, value_delimiter = ',')]
    pub select: Vec<String>,
    /// Adds these rule codes/prefixes/groups on top of the resolved select set (unioned with the
    /// config's `extend-select`, never replaces it).
    #[arg(long, value_delimiter = ',')]
    pub extend_select: Vec<String>,
    /// Subtract these rule codes/prefixes/groups.
    #[arg(long, value_delimiter = ',')]
    pub ignore: Vec<String>,
    /// Subtracts these rule codes/prefixes/groups last, after select/extend-select (unioned with
    /// the config's `extend-ignore`).
    #[arg(long, value_delimiter = ',')]
    pub extend_ignore: Vec<String>,
    /// Print every rule with its group and tier, then exit.
    #[arg(long)]
    pub list_rules: bool,
    /// Subtract findings recorded in a baseline file, so only new findings are reported.
    /// Bare `--baseline` uses `.stopslop-baseline.json`; `--baseline=PATH` picks another file.
    // require_equals is load-bearing: without it, `--baseline .` would eat the `.` scan path as
    // the baseline filename instead of leaving it as a positional.
    #[arg(long, num_args = 0..=1, require_equals = true, default_missing_value = baseline::DEFAULT_PATH)]
    pub baseline: Option<PathBuf>,
    /// Record the current findings as the baseline and exit 0. Same `=PATH` form as --baseline.
    #[arg(long, num_args = 0..=1, require_equals = true, default_missing_value = baseline::DEFAULT_PATH)]
    pub write_baseline: Option<PathBuf>,
    /// Enable SLOP010 (package-import resolution).
    #[arg(long)]
    pub check_imports: bool,
    /// Path to a config file (default: stopslop.toml in the current directory, if present).
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Ignore any stopslop.toml (CLI flags only).
    #[arg(long)]
    pub no_config: bool,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Format {
    Text,
    Json,
    Sarif,
    Markdown,
}

pub fn run(cli: Cli) -> anyhow::Result<i32> {
    // Config is discovered before the --list-rules early-return: custom rules need to appear in
    // that listing, and they only exist once the config is loaded.
    let config = Config::discover(cli.config.as_deref(), cli.no_config)?;
    let custom_rules = custom::load(&config.custom_rule)?;

    if cli.list_rules {
        list_rules(&custom_rules);
        return Ok(0);
    }

    let select = if !cli.select.is_empty() {
        cli.select
    } else {
        config.select
    };
    let ignore = if !cli.ignore.is_empty() {
        cli.ignore
    } else {
        config.ignore
    };
    // Unlike select/ignore, extend-select and extend-ignore are additive from BOTH sources at
    // once (Ruff semantics): CLI doesn't replace config here, it adds on top of it.
    let extend_select: Vec<String> = config
        .extend_select
        .into_iter()
        .chain(cli.extend_select)
        .collect();
    let extend_ignore: Vec<String> = config
        .extend_ignore
        .into_iter()
        .chain(cli.extend_ignore)
        .collect();
    let check_imports = cli.check_imports || config.check_imports;
    let paths = if cli.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        cli.paths
    };

    let custom_codes: Vec<&'static str> =
        custom_rules.iter().map(custom::CustomRule::code).collect();
    let enabled = engine::resolve_enabled(
        &select,
        &extend_select,
        &ignore,
        &extend_ignore,
        &custom_codes,
        check_imports,
    );
    let deps = if check_imports {
        Some(DepIndex::discover(&paths))
    } else {
        None
    };
    let settings = engine::Settings {
        enabled,
        deps,
        custom_rules,
    };

    let diags = walk::lint_paths(&paths, &config.exclude, &settings)?;
    // Applied right after the walk, before either baseline step: a path-scoped ignore composes
    // with a baseline (both subtract findings) instead of the two fighting over which one "owns"
    // a finding, and a fresh `--write-baseline` shouldn't fossilize findings the config already
    // declared irrelevant for that path.
    let diags = apply_per_file_ignores(diags, &config.per_file_ignores)?;

    if let Some(path) = cli.write_baseline {
        let n = Baseline::write(&path, &diags)?;
        eprintln!("stopslop: recorded {n} findings in {}", path.display());
        return Ok(0);
    }

    // CLI wins over config, and an explicit empty `baseline = ""` in config means "no baseline".
    let baseline_path = cli
        .baseline
        .or_else(|| config.baseline.filter(|p| !p.as_os_str().is_empty()));
    let diags = match baseline_path {
        Some(path) => Baseline::load(&path)?.filter(diags),
        None => diags,
    };

    let stdout = std::io::stdout();
    let mut w = stdout.lock();
    output::emit(cli.format, &diags, &settings.enabled, &mut w)?;

    let code = if diags.iter().any(|d| d.tier == Tier::A) {
        1
    } else {
        0
    };
    Ok(code)
}

/// `[per-file-ignores]` post-filter: a glob whose value list (codes and/or group names, expanded
/// through `groups::expand`) matches a diagnostic's code drops that diagnostic for paths matching
/// the glob. An invalid glob is a config error (exit 2), never a silent skip.
fn apply_per_file_ignores(
    diags: Vec<Diagnostic>,
    per_file_ignores: &std::collections::BTreeMap<String, Vec<String>>,
) -> anyhow::Result<Vec<Diagnostic>> {
    if per_file_ignores.is_empty() {
        return Ok(diags);
    }
    let mut compiled = Vec::with_capacity(per_file_ignores.len());
    for (glob_pat, codes) in per_file_ignores {
        let glob = globset::Glob::new(crate::paths::strip_dot_slash(glob_pat))
            .map_err(|e| anyhow::anyhow!("per-file-ignores {glob_pat:?}: invalid glob: {e}"))?;
        compiled.push((glob.compile_matcher(), groups::expand(codes)));
    }
    Ok(diags
        .into_iter()
        .filter(|d| {
            let path = crate::paths::strip_dot_slash(&d.path);
            !compiled
                .iter()
                .any(|(g, codes)| g.is_match(path) && codes.iter().any(|c| c == d.code))
        })
        .collect())
}

/// `code  group  tier  on-by-default  name`, grouped-name column included so `--select <group>`
/// is discoverable without reading the README.
fn list_rules(custom_rules: &[custom::CustomRule]) {
    println!(
        "{:<8} {:<10} {:<5} {:<8} NAME",
        "CODE", "GROUP", "TIER", "DEFAULT"
    );
    for r in RULES {
        println!(
            "{:<8} {:<10} {:<5} {:<8} {}",
            r.code,
            groups::group_of(r.code),
            match r.tier {
                Tier::A => "A",
                Tier::B => "B",
            },
            if r.default_on { "on" } else { "off" },
            r.name,
        );
    }
    for cr in custom_rules {
        println!(
            "{:<8} {:<10} {:<5} {:<8} {}",
            cr.code(),
            "custom",
            match cr.tier() {
                Tier::A => "A",
                Tier::B => "B",
            },
            "on", // custom rules are always on by default -- the user explicitly wrote them
            cr.name(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Tier;
    use std::collections::BTreeMap;

    fn diag(code: &'static str, path: &str) -> Diagnostic {
        Diagnostic {
            code,
            name: "test",
            tier: Tier::A,
            path: path.to_string(),
            line: 1,
            col: 1,
            message: "test".into(),
            fix: None,
        }
    }

    #[test]
    fn per_file_ignores_matches_glob_and_leaves_other_paths_alone() {
        let mut ignores = BTreeMap::new();
        ignores.insert("docs/**".to_string(), vec!["SLOP036".to_string()]);
        let diags = vec![diag("SLOP036", "docs/a.md"), diag("SLOP036", "src/a.md")];
        let kept = apply_per_file_ignores(diags, &ignores).unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].path, "src/a.md");
    }

    /// The value list accepts group names, expanded via `groups::expand` (here "rhetoric" covers
    /// SLOP036 -- see groups.rs).
    #[test]
    fn per_file_ignores_value_list_accepts_group_names() {
        let mut ignores = BTreeMap::new();
        ignores.insert("docs/**".to_string(), vec!["rhetoric".to_string()]);
        let diags = vec![diag("SLOP036", "docs/a.md")];
        let kept = apply_per_file_ignores(diags, &ignores).unwrap();
        assert!(kept.is_empty());
    }

    /// Scanning `.` (the default) prefixes every display path with `./`; scanning `docs` does not.
    /// The regression this guards: `docs/**` matched under one invocation and silently nothing
    /// under the other, so a per-file-ignore looked simply broken depending on how you invoked it.
    #[test]
    fn per_file_ignores_glob_ignores_dot_slash_on_either_side() {
        for (glob, path) in [
            ("docs/**", "./docs/a.md"),
            ("./docs/**", "docs/a.md"),
            ("./docs/**", "./docs/a.md"),
            ("docs/**", "docs/a.md"),
        ] {
            let mut ignores = BTreeMap::new();
            ignores.insert(glob.to_string(), vec!["SLOP036".to_string()]);
            let kept = apply_per_file_ignores(vec![diag("SLOP036", path)], &ignores).unwrap();
            assert!(kept.is_empty(), "glob {glob:?} should match path {path:?}");
        }
    }

    #[test]
    fn per_file_ignores_invalid_glob_is_a_config_error() {
        let mut ignores = BTreeMap::new();
        ignores.insert("[".to_string(), vec!["SLOP036".to_string()]);
        let err = apply_per_file_ignores(vec![], &ignores).unwrap_err();
        assert!(err.to_string().contains("invalid glob"));
    }
}
