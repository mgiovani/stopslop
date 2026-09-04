use crate::lang::NatLang;
use serde::Deserialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

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
    pub baseline: Option<PathBuf>,
    /// glob -> extra codes/groups to ignore for files matching that glob, applied as a post-lint
    /// filter (see cli::run) so it composes with baselines instead of fighting them.
    #[serde(default)]
    pub per_file_ignores: std::collections::BTreeMap<String, Vec<String>>,
    /// Lowest tier that fails the run. `"A"` (the default) is the historical behaviour: Tier B
    /// and Tier C findings print but never affect the exit code. `"B"` adds Tier B to the exit-1
    /// path, which is what a project wants when it cares about one warn-only rule enough to gate
    /// on it, and `"C"` puts every finding there.
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

const FILE_NAME: &str = "stopslop.toml";

impl Config {
    /// Explicit --config path, else the nearest stopslop.toml walking up from cwd, else the
    /// user-level file under $XDG_CONFIG_HOME (default ~/.config). Never merged: one source or
    /// none, so a repo that ships its own file is never affected by what sits in ~/.config.
    pub fn discover(explicit: Option<&Path>, no_config: bool) -> anyhow::Result<Config> {
        if no_config {
            return Ok(Config::default());
        }
        let path = match explicit {
            Some(p) if !p.exists() => anyhow::bail!("config file not found: {}", p.display()),
            Some(p) => p.to_path_buf(),
            None => {
                let found = find_config(
                    &std::env::current_dir()?,
                    std::env::var_os("XDG_CONFIG_HOME"),
                    std::env::var_os("HOME"),
                );
                match found {
                    Some(p) => p,
                    None => return Ok(Config::default()),
                }
            }
        };
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

/// The walk stops at the filesystem root, not the git root: stopslop runs on plain directories
/// and extracted CI artifacts too, where a git-root stop would silently find nothing.
fn find_config(
    start: &Path,
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|dir| dir.join(FILE_NAME))
        .find(|p| p.is_file())
        .or_else(|| {
            user_config_dir(xdg_config_home, home)
                .map(|dir| dir.join("stopslop").join(FILE_NAME))
                .filter(|p| p.is_file())
        })
}

/// XDG base-dir spec: an unset *or empty* XDG_CONFIG_HOME means $HOME/.config, on every
/// platform including macOS (Ruff made the same call).
fn user_config_dir(xdg_config_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let non_empty = |v: Option<OsString>| v.filter(|s| !s.is_empty()).map(PathBuf::from);
    non_empty(xdg_config_home).or_else(|| non_empty(home).map(|h| h.join(".config")))
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

    /// `[[custom-rule]]`, `[per-file-ignores]`, and `language` are independent top-level tables;
    /// this checks they parse together in one document rather than each in isolation.
    #[test]
    fn parses_custom_rule_per_file_ignores_and_language_together() {
        let cfg = parse(
            r#"
language = ["en", "pt-BR"]

[per-file-ignores]
"docs/**" = ["SLOP012", "rhetoric"]

[[custom-rule]]
pattern = "synergy"
message = "banned word: synergy"
tier = "B"
"#,
        );
        assert_eq!(cfg.natlangs().unwrap(), vec![NatLang::En, NatLang::PtBr]);
        assert_eq!(
            cfg.per_file_ignores.get("docs/**"),
            Some(&vec!["SLOP012".to_string(), "rhetoric".to_string()])
        );
        assert_eq!(cfg.custom_rule.len(), 1);
        assert_eq!(cfg.custom_rule[0].pattern, "synergy");
        assert_eq!(cfg.custom_rule[0].message, "banned word: synergy");
        assert_eq!(cfg.custom_rule[0].tier, "B");
    }

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "").unwrap();
    }

    fn os(s: &str) -> Option<OsString> {
        Some(OsString::from(s))
    }

    #[test]
    fn find_config_walks_up_to_the_nearest_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join(FILE_NAME);
        touch(&root);
        let start = tmp.path().join("a/b/c");
        std::fs::create_dir_all(&start).unwrap();
        assert_eq!(find_config(&start, None, None), Some(root));
    }

    #[test]
    fn find_config_prefers_the_nearest_ancestor() {
        let tmp = tempfile::tempdir().unwrap();
        touch(&tmp.path().join(FILE_NAME));
        let nearer = tmp.path().join("a").join(FILE_NAME);
        touch(&nearer);
        let start = tmp.path().join("a/b");
        std::fs::create_dir_all(&start).unwrap();
        assert_eq!(find_config(&start, None, None), Some(nearer));
    }

    #[test]
    fn find_config_falls_back_to_xdg_config_home() {
        let project = tempfile::tempdir().unwrap();
        let xdg = tempfile::tempdir().unwrap();
        let user = xdg.path().join("stopslop").join(FILE_NAME);
        touch(&user);
        let found = find_config(project.path(), os(xdg.path().to_str().unwrap()), None);
        assert_eq!(found, Some(user));
    }

    #[test]
    fn find_config_project_wins_over_user_level() {
        let project = tempfile::tempdir().unwrap();
        let xdg = tempfile::tempdir().unwrap();
        touch(&xdg.path().join("stopslop").join(FILE_NAME));
        let own = project.path().join(FILE_NAME);
        touch(&own);
        let found = find_config(project.path(), os(xdg.path().to_str().unwrap()), None);
        assert_eq!(found, Some(own));
    }

    #[test]
    fn find_config_uses_home_dot_config_when_xdg_is_empty() {
        let project = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let user = home.path().join(".config/stopslop").join(FILE_NAME);
        touch(&user);
        let found = find_config(project.path(), os(""), os(home.path().to_str().unwrap()));
        assert_eq!(found, Some(user));
    }

    #[test]
    fn user_config_dir_needs_a_non_empty_xdg_or_home() {
        assert_eq!(user_config_dir(None, None), None);
        assert_eq!(user_config_dir(os(""), os("")), None);
        assert_eq!(
            user_config_dir(os("/x"), os("/h")),
            Some(PathBuf::from("/x"))
        );
        assert_eq!(
            user_config_dir(None, os("/h")),
            Some(PathBuf::from("/h/.config"))
        );
    }

    #[test]
    fn find_config_ignores_a_directory_named_like_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(FILE_NAME)).unwrap();
        assert_eq!(find_config(tmp.path(), None, None), None);
    }

    #[test]
    fn find_config_returns_none_when_nothing_exists() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(find_config(tmp.path(), None, None), None);
    }

    #[test]
    fn discover_no_config_skips_every_lookup() {
        let missing = Path::new("/definitely/not/here/stopslop.toml");
        let cfg = Config::discover(Some(missing), true).unwrap();
        assert!(cfg.select.is_empty() && cfg.custom_rule.is_empty());
    }

    #[test]
    fn discover_reads_and_parses_the_explicit_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join(FILE_NAME);
        std::fs::write(&file, "select = [\"SLOP001\"]\nfail-on-tier = \"B\"\n").unwrap();
        let cfg = Config::discover(Some(&file), false).unwrap();
        assert_eq!(cfg.select, vec!["SLOP001"]);
        assert_eq!(cfg.fail_on_tier.as_deref(), Some("B"));
    }

    #[test]
    fn discover_explicit_missing_path_is_an_error() {
        let missing = Path::new("/definitely/not/here/stopslop.toml");
        let err = Config::discover(Some(missing), false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("config file not found"), "{err}");
    }
}
