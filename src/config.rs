use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub select: Vec<String>,
    /// Adds to whatever `select` resolves to (CLI or config), unioned in — never replaces it.
    #[serde(default)]
    pub extend_select: Vec<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
    /// Same union-not-replace relationship to `ignore` that `extend-select` has to `select`.
    #[serde(default)]
    pub extend_ignore: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>, // extra walker excludes (globs)
    #[serde(default)]
    pub check_imports: bool,
    /// Path to a baseline file whose findings are subtracted from every run.
    #[serde(default)]
    pub baseline: Option<std::path::PathBuf>,
    /// glob -> extra codes/groups to ignore for files matching that glob, applied as a post-lint
    /// filter (see cli::run) so it composes with baselines instead of fighting them.
    #[serde(default)]
    pub per_file_ignores: std::collections::BTreeMap<String, Vec<String>>,
    /// `[[custom-rule]]` entries: user-defined banned-phrase rules, compiled by `crate::custom`.
    #[serde(default)]
    pub custom_rule: Vec<CustomRuleConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CustomRuleConfig {
    pub pattern: String,
    pub message: String,
    #[serde(default = "default_tier")]
    pub tier: String,
    #[serde(default)]
    pub fix: Option<String>,
    /// Glob list; empty means every supported language file.
    #[serde(default)]
    pub files: Vec<String>,
}

fn default_tier() -> String {
    "B".to_string()
}

impl Config {
    /// Explicit --config path, else stopslop.toml in cwd, else Default. Missing file = default.
    pub fn discover(explicit: Option<&std::path::Path>, no_config: bool) -> anyhow::Result<Config> {
        if no_config {
            return Ok(Config::default());
        }
        let path = match explicit {
            Some(p) => p.to_path_buf(),
            None => std::path::PathBuf::from("stopslop.toml"),
        };
        if !path.exists() {
            if explicit.is_some() {
                anyhow::bail!("config file not found: {}", path.display());
            }
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("reading config {}: {e}", path.display()))?;
        let cfg: Config = toml::from_str(&text)
            .map_err(|e| anyhow::anyhow!("parsing config {}: {e}", path.display()))?;
        Ok(cfg)
    }
}
