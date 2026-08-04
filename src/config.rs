use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub select: Vec<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>, // extra walker excludes (globs)
    #[serde(default)]
    pub check_imports: bool,
    /// Path to a baseline file whose findings are subtracted from every run.
    #[serde(default)]
    pub baseline: Option<std::path::PathBuf>,
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
