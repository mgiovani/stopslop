//! Generic marker harness. Walks tests/fixtures/{typescript,python,go,rust}/** and, per file,
//! diffs the rule engine's findings against inline `expect:`/`expect-line:` markers (§8 of PLAN).

use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use stopslop::{lint_file, resolve_enabled, Lang, Settings, ALL_NATLANGS};

const LANG_DIRS: &[&str] = &["typescript", "python", "go", "rust", "markdown", "html"];

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
            enabled: resolve_enabled(&["SLOP".to_string()], &[], &[], &[], &[], false),
            deps: None,
            custom_rules: Vec::new(),
            natlangs: ALL_NATLANGS.to_vec(),
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

/// Declared `(rule, lang)` pairs that intentionally have no fixture exercising them. A rule that
/// merely lacks a language-specific AST arm does not belong here when a shared path still fires
/// for that language (SLOP037 reaches Rust through its email-regex check, see
/// fixtures/rust/slop_reinvent.rs). Every entry is checked in both directions below.
const UNEXERCISED_LANGS: &[(&str, Lang)] = &[];

// ponytail: Tsx shares every dispatch arm with Ts and prose rules never dispatch on lang, so a
// witness in one member covers the family. A no-op for a single member inside a family goes unseen.
// HTML is its own family: it reaches the rules through a different masking path, so a Markdown
// witness proves nothing about it.
fn family(lang: Lang) -> Lang {
    match lang {
        Lang::Tsx => Lang::Ts,
        Lang::Html => Lang::Html,
        l if l.is_prose() => Lang::Md,
        l => l,
    }
}

#[test]
fn every_declared_lang_has_a_fixture_witness() {
    let root = fixtures_root();

    let mut witnessed: HashSet<(String, Lang)> = HashSet::new();
    for lang_dir in LANG_DIRS {
        let dir = root.join(lang_dir);
        let mut files = Vec::new();
        collect_files(&dir, &mut files);
        for path in files {
            let Some(lang) = Lang::from_path(&path) else {
                continue;
            };
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
            for (_, code) in parse_markers(&source) {
                witnessed.insert((code, family(lang)));
            }
        }
    }

    let enabled = resolve_enabled(&["SLOP".to_string()], &[], &[], &[], &[], false);

    let mut missing = Vec::new();
    for rule in stopslop::registry::RULES {
        // SLOP010 is opt-in; its languages are covered by unit tests in src/rules/imports.rs.
        if !enabled.contains(rule.code) {
            continue;
        }
        for &lang in rule.langs {
            let witnessed_here = witnessed.contains(&(rule.code.to_string(), family(lang)));
            let excepted = UNEXERCISED_LANGS.contains(&(rule.code, lang));
            if !witnessed_here && !excepted {
                missing.push(format!(
                    "{} declares {lang:?} but no fixture under tests/fixtures/ exercises it; \
                     add a slop_* fixture or list it in UNEXERCISED_LANGS",
                    rule.code
                ));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));

    for &(code, lang) in UNEXERCISED_LANGS {
        let rule = stopslop::registry::RULES
            .iter()
            .find(|r| r.code == code)
            .unwrap_or_else(|| panic!("UNEXERCISED_LANGS lists unknown rule {code}"));
        assert!(
            rule.langs.contains(&lang),
            "{code} does not declare {lang:?}; drop the UNEXERCISED_LANGS entry"
        );
        assert!(
            !witnessed.contains(&(code.to_string(), family(lang))),
            "{code} is exercised for {lang:?}; drop the UNEXERCISED_LANGS entry"
        );
    }
}
