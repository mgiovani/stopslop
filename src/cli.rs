use crate::{
    baseline::{self, Baseline},
    config::Config,
    diagnostic::Tier,
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
    /// Subtract these rule codes/prefixes/groups.
    #[arg(long, value_delimiter = ',')]
    pub ignore: Vec<String>,
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
}

pub fn run(cli: Cli) -> anyhow::Result<i32> {
    if cli.list_rules {
        list_rules();
        return Ok(0);
    }
    let config = Config::discover(cli.config.as_deref(), cli.no_config)?;

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
    let check_imports = cli.check_imports || config.check_imports;
    let paths = if cli.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        cli.paths
    };

    let enabled = engine::resolve_enabled(&select, &ignore, check_imports);
    let deps = if check_imports {
        Some(DepIndex::discover(&paths))
    } else {
        None
    };
    let settings = engine::Settings { enabled, deps };

    let diags = walk::lint_paths(&paths, &config.exclude, &settings)?;

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

/// `code  group  tier  on-by-default  name`, grouped-name column included so `--select <group>`
/// is discoverable without reading the README.
fn list_rules() {
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
}
