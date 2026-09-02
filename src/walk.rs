use crate::{diagnostic::Diagnostic, engine, engine::Settings, lang::Lang};
use ignore::{overrides::OverrideBuilder, WalkBuilder, WalkState};
use std::path::PathBuf;
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
        self.lines_per_sec = (self.lines as f64 / self.wall_secs.max(f64::EPSILON)) as u64;
        self
    }
}

pub fn lint_paths(
    roots: &[PathBuf],
    exclude: &[String],
    settings: &Settings,
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
    if !exclude.is_empty() {
        let mut ov = OverrideBuilder::new(&cwd);
        for glob in exclude {
            ov.add(&format!("!{glob}"))?;
        }
        builder.overrides(ov.build()?);
    }

    let diags: Mutex<Vec<Diagnostic>> = Mutex::new(Vec::new());
    // Atomics, not the diags Mutex: the walk is parallel and one relaxed add per file is cheaper
    // than taking the diag lock that already runs per file.
    let files = AtomicU64::new(0);
    let skipped = AtomicU64::new(0);
    let lines = AtomicU64::new(0);
    builder.build_parallel().run(|| {
        Box::new(|entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return WalkState::Continue,
            };
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                return WalkState::Continue;
            }
            let path = entry.path();
            let lang = match Lang::from_path(path) {
                Some(l) => l,
                None => {
                    skipped.fetch_add(1, Ordering::Relaxed);
                    return WalkState::Continue;
                }
            };
            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("stopslop: skipping {}: {e}", path.display());
                    skipped.fetch_add(1, Ordering::Relaxed);
                    return WalkState::Continue;
                }
            };
            files.fetch_add(1, Ordering::Relaxed);
            lines.fetch_add(source.lines().count() as u64, Ordering::Relaxed);
            let display_path = display_path(path, &cwd);
            let mut found = engine::lint_file(display_path, &source, lang, settings);
            let mut guard = diags.lock().unwrap();
            guard.append(&mut found);
            WalkState::Continue
        })
    });

    let mut diags = diags.into_inner().unwrap();
    diags.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    let stats = Stats {
        files: files.into_inner(),
        skipped: skipped.into_inner(),
        lines: lines.into_inner(),
        ..Default::default()
    };
    Ok((diags, stats))
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
        };
        let once = lint_paths(std::slice::from_ref(&dir), &[], &settings)
            .unwrap()
            .0;
        let twice = lint_paths(&[dir.clone(), dir], &[], &settings).unwrap().0;
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
    fn stats_count_linted_and_skipped_files() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/go");
        let settings = Settings {
            enabled: resolve_enabled(&[], &[], &[], &[], &[], false),
            deps: None,
            custom_rules: Vec::new(),
        };
        let (_, stats) = lint_paths(std::slice::from_ref(&dir), &[], &settings).unwrap();

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
}
