//! Generic marker harness. Walks tests/fixtures/{typescript,python,go,rust}/** and, per file,
//! diffs the rule engine's findings against inline `expect:`/`expect-line:` markers (§8 of PLAN).
//! This file is WP0-owned and frozen after landing — WP1-5 only add fixture files.

use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use stopslop::{lint_file, resolve_enabled, Lang, Settings};

const LANG_DIRS: &[&str] = &["typescript", "python", "go", "rust", "markdown"];

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Recursively collect every file under `dir`.
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Parse `expect: SLOPNNN [SLOPMMM ...]` (same line) and `expect-line: NNN SLOPXXX` (anywhere)
/// markers out of raw source text.
fn parse_markers(source: &str) -> HashSet<(usize, String)> {
    let code_re = Regex::new(r"SLOP\d+").unwrap();
    let expect_line_re = Regex::new(r"expect-line:\s*(\d+)\s+(SLOP\d+)").unwrap();
    let mut expected = HashSet::new();

    for (idx, line) in source.lines().enumerate() {
        let line_no = idx + 1;
        if let Some(pos) = line.find("expect:") {
            for m in code_re.find_iter(&line[pos..]) {
                expected.insert((line_no, m.as_str().to_string()));
            }
        }
    }
    for caps in expect_line_re.captures_iter(source) {
        let line_no: usize = caps[1].parse().unwrap();
        expected.insert((line_no, caps[2].to_string()));
    }
    expected
}

#[test]
fn fixtures_match_markers() {
    let root = fixtures_root();

    let mut checked = 0;
    for lang_dir in LANG_DIRS {
        // Fixtures exercise ALL rule codes (incl. Tier B, default_on=false) so opt-in
        // density/style rules' `expect:`/`expect-line:` markers actually fire under this
        // harness. SLOP010 stays off: resolve_enabled drops it when check_imports == false.
        let settings = Settings {
            enabled: resolve_enabled(&["SLOP".to_string()], &[], false),
            deps: None,
        };
        let dir = root.join(lang_dir);
        let mut files = Vec::new();
        collect_files(&dir, &mut files);
        for path in files {
            let Some(lang) = Lang::from_path(&path) else {
                continue;
            };
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
            let display = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");

            let actual: HashSet<(usize, String)> =
                lint_file(display.clone(), &source, lang, &settings)
                    .into_iter()
                    .map(|d| (d.line, d.code.to_string()))
                    .collect();
            let expected = parse_markers(&source);

            assert_eq!(actual, expected, "mismatch for {display}");
            checked += 1;
        }
    }
    // Passes trivially with empty fixture dirs (WP0 lands alone).
    let _ = checked;
}
