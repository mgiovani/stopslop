use crate::{diagnostic::Diagnostic, engine, engine::Settings, lang::Lang};
use ignore::{overrides::OverrideBuilder, WalkBuilder, WalkState};
use std::path::PathBuf;
use std::sync::Mutex;

pub fn lint_paths(
    roots: &[PathBuf],
    exclude: &[String],
    settings: &Settings,
) -> anyhow::Result<Vec<Diagnostic>> {
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
                None => return WalkState::Continue,
            };
            let source = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("stopslop: skipping {}: {e}", path.display());
                    return WalkState::Continue;
                }
            };
            let display_path = display_path(path, &cwd);
            let mut found = engine::lint_file(display_path, &source, lang, settings);
            let mut guard = diags.lock().unwrap();
            guard.append(&mut found);
            WalkState::Continue
        })
    });

    let mut diags = diags.into_inner().unwrap();
    diags.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    Ok(diags)
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
            enabled: resolve_enabled(&[], &[], false),
            deps: None,
        };
        let once = lint_paths(std::slice::from_ref(&dir), &[], &settings).unwrap();
        let twice = lint_paths(&[dir.clone(), dir], &[], &settings).unwrap();
        assert!(!once.is_empty(), "fixture dir should produce findings");
        assert_eq!(
            once.len(),
            twice.len(),
            "duplicate root must not duplicate findings"
        );
    }
}
