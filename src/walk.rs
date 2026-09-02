use crate::{diagnostic::Diagnostic, engine, engine::Settings, lang::Lang};
use ignore::{overrides::Override, overrides::OverrideBuilder, WalkBuilder, WalkState};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Debug, Default, Clone, Copy, serde::Serialize)]
pub struct Stats {
    pub files: u64,
    pub skipped: u64,
    pub lines: u64,
    pub wall_secs: f64,
    pub lines_per_sec: u64,
}

impl Stats {
    pub fn with_wall(mut self, wall: std::time::Duration) -> Self {
        self.wall_secs = wall.as_secs_f64();
        self.lines_per_sec = if self.wall_secs > 0.0 {
            (self.lines as f64 / self.wall_secs) as u64
        } else {
            0
        };
        self
    }
}

/// Per-file body shared by `lint_paths` (parallel walk) and `lint_files` (fixed list).
#[derive(Default)]
struct Accumulator {
    diags: Mutex<Vec<Diagnostic>>,
    // Atomics, not the diags Mutex: the walk is parallel and one relaxed add per file is cheaper
    // than taking the diag lock that already runs per file.
    files: AtomicU64,
    skipped: AtomicU64,
    lines: AtomicU64,
}

impl Accumulator {
    fn lint(
        &self,
        path: &Path,
        cwd: &Path,
        settings: &Settings,
        read: impl FnOnce(&Path) -> std::io::Result<String>,
    ) {
        let lang = match Lang::from_path(path) {
            Some(l) => l,
            None => {
                self.skipped.fetch_add(1, Ordering::Relaxed);
                return;
            }
        };
        let source = match read(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("stopslop: skipping {}: {e}", path.display());
                self.skipped.fetch_add(1, Ordering::Relaxed);
                return;
            }
        };
        self.files.fetch_add(1, Ordering::Relaxed);
        self.lines
            .fetch_add(source.lines().count() as u64, Ordering::Relaxed);
        let mut found = engine::lint_file(display_path(path, cwd), &source, lang, settings);
        self.diags.lock().unwrap().append(&mut found);
    }

    fn finish(self) -> (Vec<Diagnostic>, Stats) {
        let mut diags = self.diags.into_inner().unwrap();
        diags.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
        let stats = Stats {
            files: self.files.into_inner(),
            skipped: self.skipped.into_inner(),
            lines: self.lines.into_inner(),
            ..Default::default()
        };
        (diags, stats)
    }
}

/// `[glob]` -> negated-glob override matching config `exclude` entries, shared by both entry
/// points so config excludes apply the same way to a walked root or a git-selected file list.
fn exclude_override(cwd: &Path, exclude: &[String]) -> anyhow::Result<Option<Override>> {
    if exclude.is_empty() {
        return Ok(None);
    }
    let mut ov = OverrideBuilder::new(cwd);
    for glob in exclude {
        ov.add(&format!("!{glob}"))?;
    }
    Ok(Some(ov.build()?))
}

pub fn lint_paths(
    roots: &[PathBuf],
    exclude: &[String],
    settings: &Settings,
    threads: usize,
) -> anyhow::Result<(Vec<Diagnostic>, Stats)> {
    let cwd = std::env::current_dir()?;
    for root in roots {
        if !root.exists() {
            anyhow::bail!("path not found: {}", root.display());
        }
    }
    // Dedup roots (canonicalized) so passing the same path twice doesn't double-report.
    let mut seen = std::collections::HashSet::new();
    let roots: Vec<&PathBuf> = roots
        .iter()
        .filter(|r| seen.insert(r.canonicalize().unwrap_or_else(|_| (*r).clone())))
        .collect();
    let mut builder = WalkBuilder::new(roots[0]);
    for root in &roots[1..] {
        builder.add(root);
    }
    builder.hidden(false).git_ignore(true).parents(true);
    builder.threads(threads);
    if let Some(ov) = exclude_override(&cwd, exclude)? {
        builder.overrides(ov);
    }

    let acc = Accumulator::default();
    builder.build_parallel().run(|| {
        Box::new(|entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return WalkState::Continue,
            };
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                return WalkState::Continue;
            }
            acc.lint(entry.path(), &cwd, settings, |p| std::fs::read_to_string(p));
            WalkState::Continue
        })
    });

    Ok(acc.finish())
}

/// Sequential entry point over a fixed file list (from `git::changed_files`), rather than a
/// walked directory tree. A diff-sized file list is small; `lint_paths` above is the parallel
/// shape to copy if a pre-commit ever needs to lint thousands of files at once.
pub fn lint_files(
    files: &[PathBuf],
    exclude: &[String],
    settings: &Settings,
    read: impl Fn(&Path) -> std::io::Result<String>,
) -> anyhow::Result<(Vec<Diagnostic>, Stats)> {
    let cwd = std::env::current_dir()?;
    let ov = exclude_override(&cwd, exclude)?;
    let acc = Accumulator::default();
    for path in files {
        // Config `exclude` globs keep applying to git-selected files, same as a walked root.
        if ov
            .as_ref()
            .is_some_and(|ov| ov.matched(path, false).is_ignore())
        {
            continue;
        }
        acc.lint(path, &cwd, settings, &read);
    }
    Ok(acc.finish())
}

fn display_path(path: &std::path::Path, cwd: &std::path::Path) -> String {
    let rel = path.strip_prefix(cwd).unwrap_or(path);
    rel.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::resolve_enabled;

    /// Passing the same root twice must not duplicate findings.
    #[test]
    fn duplicate_root_does_not_duplicate_findings() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/go");
        let settings = Settings {
            enabled: resolve_enabled(&[], &[], &[], &[], &[], false),
            deps: None,
            custom_rules: Vec::new(),
            natlangs: crate::lang::ALL_NATLANGS.to_vec(),
        };
        let once = lint_paths(std::slice::from_ref(&dir), &[], &settings, 0)
            .unwrap()
            .0;
        let twice = lint_paths(&[dir.clone(), dir], &[], &settings, 0)
            .unwrap()
            .0;
        assert!(!once.is_empty(), "fixture dir should produce findings");
        assert_eq!(
            once.len(),
            twice.len(),
            "duplicate root must not duplicate findings"
        );
    }

    /// Recursively collects `.go` files under `dir`, mirroring what the `ignore` walk should see
    /// (the fixture dir isn't gitignored) so the test has an independent expected count.
    fn go_files(dir: &std::path::Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                out.extend(go_files(&path));
            } else if path.extension().is_some_and(|e| e == "go") {
                out.push(path);
            }
        }
        out
    }

    #[test]
    fn with_wall_computes_rate_and_reports_zero_for_zero_duration() {
        let stats = Stats {
            lines: 100,
            ..Default::default()
        };
        assert_eq!(
            stats
                .with_wall(std::time::Duration::from_millis(500))
                .lines_per_sec,
            200
        );
        assert_eq!(stats.with_wall(std::time::Duration::ZERO).lines_per_sec, 0);
    }

    #[test]
    fn stats_count_linted_and_skipped_files() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/go");
        let settings = Settings {
            enabled: resolve_enabled(&[], &[], &[], &[], &[], false),
            deps: None,
            custom_rules: Vec::new(),
            natlangs: crate::lang::ALL_NATLANGS.to_vec(),
        };
        let (_, stats) = lint_paths(std::slice::from_ref(&dir), &[], &settings, 0).unwrap();

        let expected = go_files(&dir);
        let expected_lines: u64 = expected
            .iter()
            .map(|p| std::fs::read_to_string(p).unwrap().lines().count() as u64)
            .sum();

        assert_eq!(stats.files, expected.len() as u64);
        assert!(stats.skipped >= 1, "the fixture dir's .gitkeep has no Lang");
        assert_eq!(stats.lines, expected_lines);
        assert!(stats.lines > 0);
    }

    fn go_settings() -> Settings {
        Settings {
            enabled: resolve_enabled(&[], &[], &[], &[], &[], false),
            deps: None,
            custom_rules: Vec::new(),
            natlangs: crate::lang::ALL_NATLANGS.to_vec(),
        }
    }

    #[test]
    fn lint_files_matches_lint_paths() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/go");
        let settings = go_settings();
        let files = go_files(&dir);

        let (from_files, _) =
            lint_files(&files, &[], &settings, |p| std::fs::read_to_string(p)).unwrap();
        let (from_paths, _) = lint_paths(std::slice::from_ref(&dir), &[], &settings, 0).unwrap();

        assert!(
            !from_paths.is_empty(),
            "fixture dir should produce findings"
        );
        // Diagnostic has no PartialEq; comparing Debug output is equivalent and avoids adding one
        // just for this test.
        assert_eq!(format!("{from_files:?}"), format!("{from_paths:?}"));
    }

    #[test]
    fn lint_files_applies_exclude() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/go");
        let settings = go_settings();
        let files = go_files(&dir);

        let (diags, stats) = lint_files(&files, &["**/*.go".to_string()], &settings, |p| {
            std::fs::read_to_string(p)
        })
        .unwrap();

        assert!(diags.is_empty());
        assert_eq!(stats.files, 0);
    }

    #[test]
    fn lint_files_read_error_is_skipped_not_failed() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/go");
        let settings = go_settings();
        let files = go_files(&dir);

        let (diags, stats) = lint_files(&files, &[], &settings, |_| {
            Err(std::io::Error::other("boom"))
        })
        .unwrap();

        assert!(diags.is_empty());
        assert_eq!(stats.skipped, files.len() as u64);
    }
}
