use crate::{
    baseline::{self, Baseline},
    config::Config,
    custom,
    diagnostic::{Diagnostic, Tier},
    engine, git, groups,
    imports_data::DepIndex,
    output,
    registry::RULES,
    walk,
};
use clap::{Parser, ValueEnum};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use update_informer::Check;

#[derive(Parser)]
#[command(name = "stopslop", version, about = "Like Ruff, but for AI slop.")]
pub struct Cli {
    /// Paths to scan (default: current directory).
    pub paths: Vec<PathBuf>,
    /// Lint only files staged in the git index, reading their staged content rather than the
    /// working tree, so a partially staged file is checked as it will be committed.
    #[arg(long, group = "git_scope")]
    pub staged: bool,
    /// Lint only tracked files with staged or unstaged changes against HEAD. Untracked files are
    /// not included.
    #[arg(long, group = "git_scope")]
    pub changed: bool,
    /// Lint only files changed since the merge base with REF, e.g. `--since origin/main` on a PR.
    #[arg(long, value_name = "REF", group = "git_scope")]
    pub since: Option<String>,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
    /// Only run these rule codes/prefixes/groups (resets defaults). Comma-separated or repeated.
    #[arg(long, value_delimiter = ',')]
    pub select: Vec<String>,
    /// Adds these rule codes/prefixes/groups on top of the resolved select set (unioned with the
    /// config's `extend-select`, never replaces it).
    #[arg(long, value_delimiter = ',')]
    pub extend_select: Vec<String>,
    /// Subtract these rule codes/prefixes/groups (replaces the config's `ignore`; use
    /// --extend-ignore to add to it instead). Comma-separated or repeated.
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
    /// Path to a config file (default: the nearest stopslop.toml walking up from the current
    /// directory, else $XDG_CONFIG_HOME/stopslop/stopslop.toml, ~/.config when unset).
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Ignore any project or user-level stopslop.toml (CLI flags only).
    #[arg(long)]
    pub no_config: bool,
    /// Lowest tier that exits 1: "A" (default) fails only on Tier A findings, "B" fails on any
    /// finding. Overrides the config's `fail-on-tier`.
    #[arg(long)]
    pub fail_on_tier: Option<String>,
    /// Report files/lines scanned, wall time and throughput. Text and markdown modes print the
    /// summary to stderr (stdout is unchanged); json and sarif carry it inside the payload as a
    /// `stats` object. "skipped" counts files the walk reached but could not lint (unsupported
    /// extension or unreadable); paths dropped by .gitignore or `exclude` are never walked.
    #[arg(long)]
    pub stats: bool,
    /// Worker threads for the walk; 0 picks automatically.
    #[arg(short = 'j', long = "threads", default_value_t = 0)]
    pub threads: usize,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Format {
    Text,
    Json,
    Sarif,
    Markdown,
}

/// Cap on worker threads per available core, applied to `-j/--threads` before it reaches
/// `ignore::WalkBuilder::threads`. A reviewer once ran `stopslop -j 100000`, which spawned
/// ~12k OS threads and froze the machine; 4 per core is generous headroom over what a
/// directory walk can ever keep busy, while still letting `-j` ask for more than one thread
/// per core on a real multi-core box.
const MAX_THREADS_PER_CORE: usize = 4;

/// `0` keeps meaning "auto" (the walker sizes itself, same as today). A nonzero request above
/// `available * MAX_THREADS_PER_CORE` is silently capped rather than handed to the walker
/// unvalidated -- see `MAX_THREADS_PER_CORE`'s doc comment for the incident this guards against.
fn effective_threads(requested: usize, available: usize) -> usize {
    if requested == 0 {
        return 0;
    }
    requested.min(available.max(1) * MAX_THREADS_PER_CORE)
}

pub fn run(cli: Cli) -> anyhow::Result<i32> {
    let started = std::time::Instant::now();
    // Config is discovered before the --list-rules early-return: custom rules need to appear in
    // that listing, and they only exist once the config is loaded.
    let config = Config::discover(cli.config.as_deref(), cli.no_config)?;
    let custom_rules = custom::load(&config.custom_rule)?;
    // Resolved before any field of `config` is partially moved out below.
    let natlangs = config.natlangs()?;

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
    // Resolved before the walk so a bad spelling is a startup config error, not a surprise after
    // a full scan has already printed its findings.
    let fail_on = match cli.fail_on_tier.or(config.fail_on_tier) {
        Some(s) => Tier::parse(&s).ok_or_else(|| {
            anyhow::anyhow!("invalid fail-on-tier {s:?}, expected \"A\" or \"B\"")
        })?,
        None => Tier::A,
    };
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
    // An empty resolved set from a non-empty select would otherwise lint nothing and exit 0 with
    // only a warning, indistinguishable from a clean run; a partial typo among patterns still
    // just warns.
    if !select.is_empty() && enabled.is_empty() {
        anyhow::bail!(
            "--select matched no rule code (patterns: {})",
            select.join(", ")
        );
    }
    let deps = if check_imports {
        Some(DepIndex::discover(&paths))
    } else {
        None
    };
    let settings = engine::Settings {
        enabled,
        deps,
        custom_rules,
        natlangs,
    };

    let scope = if cli.staged {
        Some(git::Scope::Staged)
    } else if cli.changed {
        Some(git::Scope::Changed)
    } else {
        cli.since.map(git::Scope::Since)
    };
    let available = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    let threads = effective_threads(cli.threads, available);
    if threads != cli.threads {
        eprintln!(
            "stopslop: warning: -j {} exceeds {available} cores x {MAX_THREADS_PER_CORE}; \
             using {threads} threads instead",
            cli.threads,
        );
    }
    let (diags, stats) = std::thread::scope(|warm| {
        engine::prewarm(warm, &settings);
        match scope {
            Some(scope) => {
                let files = git::changed_files(Path::new("."), &scope, &paths)?;
                let staged = matches!(scope, git::Scope::Staged);
                walk::lint_files(&files, &config.exclude, &settings, move |p| {
                    if staged {
                        git::staged_source(Path::new("."), p)
                    } else {
                        std::fs::read_to_string(p)
                    }
                })
            }
            None => walk::lint_paths(&paths, &config.exclude, &settings, threads),
        }
    })?;
    // Applied before baseline so a path-scoped ignore composes with it instead of the two
    // fighting over ownership, and `--write-baseline` doesn't fossilize findings the config
    // already excluded.
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

    // Captured before `stats` is shadowed by the `--stats` Option: the exit code must reflect a
    // panicked file whether or not the user asked for the stats block.
    let panicked = stats.panicked;
    let stats = cli.stats.then(|| stats.with_wall(started.elapsed()));
    let stdout = std::io::stdout();
    let mut w = stdout.lock();
    output::emit(
        cli.format,
        &diags,
        &settings.enabled,
        stats.as_ref(),
        &mut w,
    )?;
    if let (Some(s), Format::Text | Format::Markdown) = (stats, cli.format) {
        eprint!("{}", output::render_stats(&s));
    }
    if matches!(cli.format, Format::Text) {
        update_notice();
    }

    if panicked > 0 {
        eprintln!("stopslop: {panicked} file(s) skipped after a rule panicked");
    }
    Ok(exit_code(&diags, fail_on, panicked))
}

/// One stderr line when crates.io has a newer stable release, checked at most once per 24h
/// (update-informer caches under the platform cache dir). Skipped in CI, on opt-out, and when
/// stderr is not a terminal, so scripts and the pr-comment workflow never see it. Every failure
/// (offline, timeout, unwritable cache) is swallowed: this must never change the exit code.
///
/// The request carries an explicit timeout because this runs AFTER output, on the way out: with
/// the crate's own default, a half-open network held the process open long after the lint the
/// user was waiting on had finished (issue #21, H3).
fn update_notice() {
    let opted_out = ["CI", "STOPSLOP_NO_UPDATE_CHECK", "NO_UPDATE_NOTIFIER"]
        .iter()
        .any(|k| std::env::var_os(k).is_some());
    if opted_out || !std::io::stderr().is_terminal() {
        return;
    }
    let current = env!("CARGO_PKG_VERSION");
    // Two seconds: above a healthy crates.io round trip, below what a hung one used to cost.
    let informer = update_informer::new(update_informer::registry::Crates, "stopslop", current)
        .timeout(std::time::Duration::from_secs(2));
    // The crate reports the newest published version, not crates.io's max_stable_version, so a
    // pre-release is filtered here rather than advertised.
    if let Ok(Some(latest)) = informer.check_version() {
        let latest = latest.semver();
        if latest.pre.is_empty() {
            eprintln!(
                "stopslop: {current} is installed, {latest} is available. \
                 Run `cargo install stopslop` to update."
            );
        }
    }
}

/// 2 if any file was skipped because a rule panicked, else 1 if any finding is at or above
/// `fail_on` in severity, else 0. A crashed rule outranks the findings: the report is
/// incomplete, so a clean-looking 0 would be a lie. Split out of `run` so both contracts are
/// testable without a filesystem walk.
fn exit_code(diags: &[Diagnostic], fail_on: Tier, panicked: u64) -> i32 {
    if panicked > 0 {
        2
    } else if diags.iter().any(|d| d.tier.at_least_as_severe_as(fail_on)) {
        1
    } else {
        0
    }
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

/// `code  group  tier  on-by-default  natlang  name`, grouped-name column included so
/// `--select <group>` is discoverable without reading the README.
fn list_rules(custom_rules: &[custom::CustomRule]) {
    println!(
        "{:<8} {:<10} {:<5} {:<8} {:<10} NAME",
        "CODE", "GROUP", "TIER", "DEFAULT", "NATLANG"
    );
    for r in RULES {
        println!(
            "{:<8} {:<10} {:<5} {:<8} {:<10} {}",
            r.code,
            groups::group_of(r.code),
            match r.tier {
                Tier::A => "A",
                Tier::B => "B",
            },
            if r.default_on { "on" } else { "off" },
            r.natlangs
                .iter()
                .map(|n| n.label())
                .collect::<Vec<_>>()
                .join(", "),
            r.name,
        );
    }
    for cr in custom_rules {
        println!(
            "{:<8} {:<10} {:<5} {:<8} {:<10} {}",
            cr.code(),
            "custom",
            match cr.tier() {
                Tier::A => "A",
                Tier::B => "B",
            },
            "on", // custom rules are always on by default -- the user explicitly wrote them
            "en", // custom rules are user regexes, not a validated lexicon in any one language
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

    fn diag_at(tier: Tier) -> Diagnostic {
        Diagnostic {
            tier,
            ..diag("SLOP017", "a.md")
        }
    }

    #[test]
    fn fail_on_tier_a_ignores_tier_b_findings() {
        assert_eq!(exit_code(&[diag_at(Tier::B)], Tier::A, 0), 0);
        assert_eq!(exit_code(&[diag_at(Tier::A)], Tier::A, 0), 1);
    }

    #[test]
    fn fail_on_tier_b_gates_on_any_finding() {
        assert_eq!(exit_code(&[diag_at(Tier::B)], Tier::B, 0), 1);
        assert_eq!(exit_code(&[diag_at(Tier::A)], Tier::B, 0), 1);
        assert_eq!(exit_code(&[], Tier::B, 0), 0);
    }

    /// A panicked file outranks the findings: the report is incomplete, so exit 2 even when what
    /// did get linted is clean, and even when a Tier A finding would otherwise have said 1.
    #[test]
    fn a_panicked_file_exits_2_whatever_the_findings_say() {
        assert_eq!(exit_code(&[], Tier::A, 1), 2);
        assert_eq!(exit_code(&[diag_at(Tier::A)], Tier::A, 1), 2);
        assert_eq!(exit_code(&[diag_at(Tier::A)], Tier::A, 0), 1);
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

    #[test]
    fn effective_threads_zero_stays_auto() {
        assert_eq!(effective_threads(0, 8), 0);
        assert_eq!(effective_threads(0, 1), 0);
    }

    #[test]
    fn effective_threads_passes_through_a_request_at_or_under_the_cap() {
        assert_eq!(effective_threads(2, 8), 2);
        assert_eq!(effective_threads(32, 8), 32); // exactly at the cap (8 * 4)
    }

    /// The incident this guards against: `-j 100000` on an 8-core machine must not reach
    /// `WalkBuilder::threads` unclamped.
    #[test]
    fn effective_threads_clamps_an_absurd_request() {
        assert_eq!(effective_threads(100_000, 8), 32);
    }

    #[test]
    fn effective_threads_clamps_relative_to_a_single_core() {
        assert_eq!(effective_threads(4, 1), 4);
        assert_eq!(effective_threads(5, 1), 4);
    }

    // --- cli::run integration tests ---
    //
    // `default_cli` sets `no_config: true` so a run is isolated from this repo's own
    // `stopslop.toml`. `fixture_dir` holds one Tier A finding (SLOP012) and one clean file.

    fn default_cli(paths: Vec<PathBuf>) -> Cli {
        Cli {
            paths,
            staged: false,
            changed: false,
            since: None,
            format: Format::Text,
            select: Vec::new(),
            extend_select: Vec::new(),
            ignore: Vec::new(),
            extend_ignore: Vec::new(),
            list_rules: false,
            baseline: None,
            write_baseline: None,
            check_imports: false,
            config: None,
            no_config: true,
            fail_on_tier: None,
            stats: false,
            threads: 0,
        }
    }

    fn fixture_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("slop.md"),
            "The figures were pulled from turn0search0 directly.\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("clean.md"),
            "This release requires v2+1 of the client SDK or newer.\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn run_default_exits_1_on_a_tier_a_finding() {
        let dir = fixture_dir();
        let cli = default_cli(vec![dir.path().to_path_buf()]);
        assert_eq!(run(cli).unwrap(), 1);
    }

    /// SLOP004 is a real code, so it doesn't trip the "matched no rule code" path, but it's
    /// `CODE_LANGS`-only and so can never fire on these `.md` fixtures.
    #[test]
    fn run_select_of_a_rule_that_does_not_fire_exits_0() {
        let dir = fixture_dir();
        let mut cli = default_cli(vec![dir.path().to_path_buf()]);
        cli.select = vec!["SLOP004".to_string()];
        assert_eq!(run(cli).unwrap(), 0);
    }

    #[test]
    fn run_ignore_of_the_firing_rule_exits_0() {
        let dir = fixture_dir();
        let mut cli = default_cli(vec![dir.path().to_path_buf()]);
        cli.ignore = vec!["SLOP012".to_string()];
        assert_eq!(run(cli).unwrap(), 0);
    }

    #[test]
    fn run_write_baseline_then_baseline_filters_the_recorded_finding() {
        let dir = fixture_dir();
        let baseline_path = dir.path().join("baseline.json");

        let mut write_cli = default_cli(vec![dir.path().to_path_buf()]);
        write_cli.write_baseline = Some(baseline_path.clone());
        assert_eq!(run(write_cli).unwrap(), 0);

        let mut second_cli = default_cli(vec![dir.path().to_path_buf()]);
        second_cli.baseline = Some(baseline_path);
        assert_eq!(run(second_cli).unwrap(), 0);
    }

    #[test]
    fn run_format_json_exit_code_matches_text() {
        let dir = fixture_dir();
        let mut cli = default_cli(vec![dir.path().to_path_buf()]);
        cli.format = Format::Json;
        assert_eq!(run(cli).unwrap(), 1);
    }

    /// `no_config` wins over an explicit `--config` path (see `Config::discover`), so an
    /// `ignore = ["SLOP012"]` file placed there never applies and the Tier A finding still
    /// fails the run.
    #[test]
    fn run_no_config_bypasses_an_explicit_config_file() {
        let dir = fixture_dir();
        let config_path = dir.path().join("stopslop.toml");
        std::fs::write(&config_path, "ignore = [\"SLOP012\"]\n").unwrap();

        let mut cli = default_cli(vec![dir.path().to_path_buf()]);
        cli.config = Some(config_path);
        cli.no_config = true;
        assert_eq!(run(cli).unwrap(), 1);
    }

    #[test]
    fn run_select_matching_no_rule_code_is_an_error() {
        let dir = fixture_dir();
        let mut cli = default_cli(vec![dir.path().to_path_buf()]);
        cli.select = vec!["SLOP999".to_string()];
        assert!(run(cli).is_err());
    }
}
