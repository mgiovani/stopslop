use crate::{config::Config, diagnostic::Tier, engine, imports_data::DepIndex, output, walk};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "stopslop", version, about = "Like Ruff, but for AI slop.")]
pub struct Cli {
    /// Paths to scan (default: current directory).
    pub paths: Vec<PathBuf>,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
    /// Only run these rule codes/prefixes (resets defaults). Comma-separated or repeated.
    #[arg(long, value_delimiter = ',')]
    pub select: Vec<String>,
    /// Subtract these rule codes/prefixes.
    #[arg(long, value_delimiter = ',')]
    pub ignore: Vec<String>,
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
