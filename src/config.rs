use crate::lang::NatLang;
use serde::Deserialize;

/// Accepts `language = "pt-BR"` or `language = ["en", "pt-BR"]` in TOML without a wrapper table.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

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
    /// Lowest tier that fails the run. `"A"` (the default) is the historical behaviour: Tier B
    /// findings print but never affect the exit code. `"B"` puts every finding on the exit-1 path,
    /// which is what a project wants when it cares about one warn-only rule enough to gate on it.
    /// `None` (absent) means `"A"`. Kept optional rather than defaulted to a string so the
    /// `#[derive(Default)]` used for the no-config-file case can't produce an unparseable `""`.
    #[serde(default)]
    pub fail_on_tier: Option<String>,
    /// `[[custom-rule]]` entries: user-defined banned-phrase rules, compiled by `crate::custom`.
    #[serde(default)]
    pub custom_rule: Vec<CustomRuleConfig>,
    /// Restricts linting to these natural languages (BCP-47-ish tags; see `NatLang::from_tag`).
    /// Absent or empty means every supported language -- the union default described in
    /// `NatLang`'s doc comment.
    #[serde(default)]
    pub language: Option<OneOrMany>,
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

    /// Resolves `language` into the concrete set to lint. Empty or absent means every supported
    /// language; an unrecognized tag is a startup config error (matches `fail_on_tier`'s
    /// fail-fast-on-typo shape) rather than a silent no-op.
    pub fn natlangs(&self) -> anyhow::Result<Vec<NatLang>> {
        let tags: Vec<String> = match &self.language {
            None => Vec::new(),
            Some(OneOrMany::One(s)) => vec![s.clone()],
            Some(OneOrMany::Many(v)) => v.clone(),
        };
        if tags.is_empty() {
            return Ok(crate::lang::ALL_NATLANGS.to_vec());
        }
        let mut out: Vec<NatLang> = Vec::new();
        for tag in tags {
            let nl = NatLang::from_tag(&tag).ok_or_else(|| {
                anyhow::anyhow!("unknown language '{tag}' in config; supported: en, pt-BR")
            })?;
            if !out.contains(&nl) {
                out.push(nl);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml_src: &str) -> Config {
        toml::from_str(toml_src).unwrap()
    }

    #[test]
    fn natlangs_absent_means_every_language() {
        assert_eq!(
            Config::default().natlangs().unwrap(),
            crate::lang::ALL_NATLANGS
        );
    }

    #[test]
    fn natlangs_accepts_a_single_string() {
        let cfg = parse(r#"language = "pt-BR""#);
        assert_eq!(cfg.natlangs().unwrap(), vec![NatLang::PtBr]);
    }

    #[test]
    fn natlangs_accepts_an_array() {
        let cfg = parse(r#"language = ["en", "pt-BR"]"#);
        assert_eq!(cfg.natlangs().unwrap(), vec![NatLang::En, NatLang::PtBr]);
    }

    #[test]
    fn natlangs_rejects_unknown_tag() {
        let cfg = parse(r#"language = "fr""#);
        let err = cfg.natlangs().unwrap_err().to_string();
        assert!(err.contains("unknown language 'fr'"), "{err}");
        assert!(err.contains("en, pt-BR"), "{err}");
    }

    #[test]
    fn natlangs_dedupes_preserving_first_occurrence_order() {
        let cfg = parse(r#"language = ["pt-BR", "en", "pt", "en-US"]"#);
        assert_eq!(cfg.natlangs().unwrap(), vec![NatLang::PtBr, NatLang::En]);
    }
}
