//! A rule can declare `natlangs: &[.., NatLang::PtBr, ..]` for a lexicon that was never actually
//! tuned on Portuguese -- the enum compiles either way. This harness makes the declaration prove
//! itself: every PtBr-declaring rule needs a pt-BR fixture that fires it, and every pt-BR fixture
//! marker needs a rule that actually declares PtBr, so a stray marker can't hide a missing
//! declaration either. Copies `parse_markers`/`collect_files` from `tests/integration.rs` (a
//! separate test crate, so importing isn't an option) rather than growing the frozen harness.

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use stopslop::lang::CODE_LANGS;
use stopslop::registry::{RuleDef, RULES};
use stopslop::{resolve_enabled, Lang, NatLang};

fn pt_br_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/markdown/pt-br")
}

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

fn pt_br_witnessed_codes() -> HashSet<String> {
    let mut files = Vec::new();
    collect_files(&pt_br_dir(), &mut files);
    let mut witnessed = HashSet::new();
    for path in files {
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        for (_, code) in parse_markers(&source) {
            witnessed.insert(code);
        }
    }
    witnessed
}

#[test]
fn every_ptbr_rule_has_a_pt_br_fixture_witness() {
    let enabled = resolve_enabled(&["SLOP".to_string()], &[], &[], &[], &[], false);
    let witnessed = pt_br_witnessed_codes();

    let mut missing = Vec::new();
    for rule in RULES {
        if !enabled.contains(rule.code) {
            continue;
        }
        let is_prose = rule.langs.iter().any(|l| l.is_prose());
        let is_ptbr = rule.natlangs.contains(&NatLang::PtBr);
        if is_prose && is_ptbr && !witnessed.contains(rule.code) {
            missing.push(format!(
                "{} declares PtBr but no fixture under tests/fixtures/markdown/pt-br/ exercises \
                 it; add a slop_* fixture",
                rule.code
            ));
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

/// Code-lang fixture dirs this witness scans. Mirrors `LANG_DIRS`'s code-only members in
/// tests/integration.rs (a separate test crate, so importing isn't an option -- see this file's
/// top doc comment).
const CODE_LANG_DIRS: &[&str] = &["typescript", "python", "go", "rust"];

fn code_fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Tsx shares every dispatch arm with Ts (same reasoning as `family` in tests/integration.rs), so
/// a witness in either member proves the pair; every other code lang stands alone.
fn code_family(lang: Lang) -> Lang {
    match lang {
        Lang::Tsx => Lang::Ts,
        other => other,
    }
}

fn is_pt_br_fixture_name(path: &Path) -> bool {
    static NAME_RE: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"^slop_.*_pt_br\.[A-Za-z0-9]+$").unwrap());
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| NAME_RE.is_match(n))
}

/// rule code -> code-language families a `slop_*_pt_br.<ext>` fixture actually exercises.
fn pt_br_code_witnessed_families() -> HashMap<String, HashSet<Lang>> {
    let root = code_fixtures_root();
    let mut witnessed: HashMap<String, HashSet<Lang>> = HashMap::new();
    for dir in CODE_LANG_DIRS {
        let mut files = Vec::new();
        collect_files(&root.join(dir), &mut files);
        for path in files {
            let Some(lang) = Lang::from_path(&path) else {
                continue;
            };
            if !is_pt_br_fixture_name(&path) {
                continue;
            }
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
            for (_, code) in parse_markers(&source) {
                witnessed.entry(code).or_default().insert(code_family(lang));
            }
        }
    }
    witnessed
}

/// The code-language rules that read natural language out of comments and carry a Portuguese
/// panel. Every other code rule declares `ALL_NATLANGS` because it matches AST shape, not
/// words, and a Portuguese fixture would prove nothing about it. Add a rule here in the PR that
/// gives its panel Portuguese entries; the reverse check below fails when a `*_pt_br` fixture
/// names a rule that is not listed.
const PT_BR_LEXICON_CODE_RULES: &[&str] = &["SLOP001", "SLOP002", "SLOP004", "SLOP009", "SLOP042"];

/// The code-lang twin of `every_ptbr_rule_has_a_pt_br_fixture_witness`: each rule in
/// `PT_BR_LEXICON_CODE_RULES` needs a hand-written `slop_*_pt_br.<ext>` fixture (with an
/// `expect:`/`expect-line:` marker for that rule) per declared code-language family, so the
/// PtBr declaration proves itself there too, not only on the prose side the other test covers.
#[test]
fn every_ptbr_code_rule_has_a_pt_br_fixture_witness_per_family() {
    let enabled = resolve_enabled(&["SLOP".to_string()], &[], &[], &[], &[], false);
    let witnessed = pt_br_code_witnessed_families();

    let unlisted: Vec<&String> = witnessed
        .keys()
        .filter(|code| !PT_BR_LEXICON_CODE_RULES.contains(&code.as_str()))
        .collect();
    assert!(
        unlisted.is_empty(),
        "pt-BR code fixtures exist for rules outside PT_BR_LEXICON_CODE_RULES: {unlisted:?}"
    );

    let mut missing = Vec::new();
    for rule in RULES {
        if !enabled.contains(rule.code) || !PT_BR_LEXICON_CODE_RULES.contains(&rule.code) {
            continue;
        }
        assert!(
            rule.natlangs.contains(&NatLang::PtBr),
            "{} is listed as a pt-BR lexicon rule but does not declare NatLang::PtBr",
            rule.code
        );
        let families: HashSet<Lang> = rule
            .langs
            .iter()
            .copied()
            .filter(|l| CODE_LANGS.contains(l))
            .map(code_family)
            .collect();
        if families.is_empty() {
            continue; // no code-lang exposure; the prose witness test above covers this rule
        }
        let have = witnessed.get(rule.code);
        for lang in &families {
            if !have.is_some_and(|h| h.contains(lang)) {
                missing.push(format!(
                    "{} declares {lang:?} and PtBr but no tests/fixtures/ fixture named \
                     slop_*_pt_br.<ext> exercises it for that family",
                    rule.code
                ));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
fn no_pt_br_fixture_witnesses_an_english_only_rule() {
    let by_code: HashMap<&str, &'static RuleDef> = RULES.iter().map(|r| (r.code, *r)).collect();

    let mut wrong = Vec::new();
    for code in pt_br_witnessed_codes() {
        let Some(rule) = by_code.get(code.as_str()) else {
            continue; // unknown/custom code: not this test's concern
        };
        if !rule.natlangs.contains(&NatLang::PtBr) {
            wrong.push(format!(
                "tests/fixtures/markdown/pt-br/ has a marker for {code}, but its RuleDef does \
                 not declare NatLang::PtBr; add it there before witnessing it here"
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
