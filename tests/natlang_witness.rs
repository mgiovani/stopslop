//! A rule can declare `natlangs: &[.., NatLang::PtBr, ..]` for a lexicon that was never actually
//! tuned on Portuguese -- the enum compiles either way. This harness makes the declaration prove
//! itself: every PtBr-declaring rule needs a pt-BR fixture that fires it, and every pt-BR fixture
//! marker needs a rule that actually declares PtBr, so a stray marker can't hide a missing
//! declaration either. Copies `parse_markers`/`collect_files` from `tests/integration.rs` (a
//! separate test crate, so importing isn't an option) rather than growing the frozen harness.

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use stopslop::registry::{RuleDef, RULES};
use stopslop::{resolve_enabled, NatLang};

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
