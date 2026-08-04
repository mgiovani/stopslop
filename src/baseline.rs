//! Baseline support: adopt stopslop on a codebase that already has findings without having to fix
//! them all first. `--write-baseline` records today's findings; `--baseline` then subtracts them
//! from every later run, so CI fails only on findings that are NEW. The count can go down or stay
//! flat on its own; going up takes a deliberate baseline rewrite.
//!
//! Findings are matched by fingerprint, not by line number, so unrelated edits above a finding
//! don't resurrect it.

use crate::diagnostic::Diagnostic;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const DEFAULT_PATH: &str = ".stopslop-baseline.json";

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Baseline {
    /// fingerprint -> how many occurrences were accepted. Counted, not a bare set: most rules
    /// emit a fixed message, so every occurrence of one rule in one file shares a fingerprint. A
    /// set would let a single accepted entry grandfather an unlimited number of NEW findings in
    /// that file. With counts, N accepted occurrences absorb exactly N -- the (N+1)th is reported.
    /// `BTreeMap` keeps the file key-sorted so it diffs cleanly in review.
    accepted: BTreeMap<String, usize>,
}

/// `code|path|message with every digit replaced by '#'`.
///
/// The line and column are deliberately absent: a finding that moved because someone added an
/// import above it is the same finding. Digits are normalized out for the same reason -- a density
/// rule's "7 occurrences vs threshold 5" would otherwise un-baseline itself the moment the count
/// shifted by one, which is exactly the noise a baseline exists to absorb.
pub fn fingerprint(d: &Diagnostic) -> String {
    let message: String = d
        .message
        .chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .collect();
    format!("{}|{}|{}", d.code, d.path, message)
}

impl Baseline {
    pub fn load(path: &Path) -> anyhow::Result<Baseline> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!(
                "reading baseline {}: {e} (create one with --write-baseline)",
                path.display()
            )
        })?;
        serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("parsing baseline {}: {e}", path.display()))
    }

    /// Writes `diags` as the accepted set. Returns the total number of findings recorded.
    pub fn write(path: &Path, diags: &[Diagnostic]) -> anyhow::Result<usize> {
        let mut accepted: BTreeMap<String, usize> = BTreeMap::new();
        for d in diags {
            *accepted.entry(fingerprint(d)).or_insert(0) += 1;
        }
        let text = serde_json::to_string_pretty(&Baseline { accepted })?;
        std::fs::write(path, text + "\n")
            .map_err(|e| anyhow::anyhow!("writing baseline {}: {e}", path.display()))?;
        Ok(diags.len())
    }

    /// Drops up to the accepted count of each fingerprint, keeping the rest.
    ///
    /// Findings arrive sorted by (path, line, col), so the ones absorbed are the earliest in the
    /// file and the surplus reported is the tail. Which specific occurrence gets reported is
    /// arbitrary when the message can't distinguish them; the count is what matters, and it
    /// ratchets: fix one and the budget shrinks, add one and it surfaces.
    pub fn filter(&self, diags: Vec<Diagnostic>) -> Vec<Diagnostic> {
        let mut remaining = self.accepted.clone();
        diags
            .into_iter()
            .filter(|d| match remaining.get_mut(&fingerprint(d)) {
                Some(n) if *n > 0 => {
                    *n -= 1;
                    false
                }
                _ => true,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Tier;

    fn diag(code: &'static str, path: &str, line: usize, message: &str) -> Diagnostic {
        Diagnostic {
            code,
            name: "test",
            tier: Tier::A,
            path: path.to_string(),
            line,
            col: 1,
            message: message.to_string(),
            fix: None,
        }
    }

    #[test]
    fn fingerprint_ignores_line_and_digits() {
        let a = diag(
            "SLOP015",
            "a.md",
            3,
            "high density (7 occurrences vs threshold 5)",
        );
        let b = diag(
            "SLOP015",
            "a.md",
            91,
            "high density (8 occurrences vs threshold 5)",
        );
        assert_eq!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn fingerprint_separates_code_path_and_message() {
        let a = diag("SLOP014", "a.md", 1, "cliché");
        assert_ne!(
            fingerprint(&a),
            fingerprint(&diag("SLOP014", "b.md", 1, "cliché"))
        );
        assert_ne!(
            fingerprint(&a),
            fingerprint(&diag("SLOP016", "a.md", 1, "cliché"))
        );
    }

    fn baseline_of(diags: &[Diagnostic]) -> Baseline {
        let mut accepted = BTreeMap::new();
        for d in diags {
            *accepted.entry(fingerprint(d)).or_insert(0) += 1;
        }
        Baseline { accepted }
    }

    #[test]
    fn filter_drops_accepted_and_keeps_new() {
        let baseline = baseline_of(&[diag("SLOP014", "a.md", 1, "stock cliché")]);
        let kept = baseline.filter(vec![
            diag("SLOP014", "a.md", 40, "stock cliché"), // same finding, moved
            diag("SLOP014", "b.md", 1, "stock cliché"),  // new file
        ]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].path, "b.md");
    }

    /// The regression that motivated counting: most rules emit one fixed message, so every
    /// occurrence in a file shares a fingerprint. One accepted occurrence must absorb exactly one.
    #[test]
    fn accepted_count_bounds_how_many_are_absorbed() {
        let d = diag("SLOP025", "a.md", 3, "unsourced weasel attribution");
        let baseline = baseline_of(std::slice::from_ref(&d));
        let kept = baseline.filter(vec![
            d.clone(),
            diag("SLOP025", "a.md", 9, "unsourced weasel attribution"),
        ]);
        assert_eq!(
            kept.len(),
            1,
            "a second, NEW occurrence must still be reported"
        );
    }

    /// Fixing one of two accepted occurrences must not report the survivor.
    #[test]
    fn removing_one_occurrence_reports_nothing() {
        let d = diag("SLOP025", "a.md", 3, "unsourced weasel attribution");
        let baseline = baseline_of(&[d.clone(), d.clone()]);
        assert!(baseline.filter(vec![d]).is_empty());
    }

    #[test]
    fn write_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("baseline.json");
        let d = diag("SLOP014", "a.md", 1, "stock cliché");
        let n = Baseline::write(&path, &[d.clone(), d.clone()]).unwrap();
        assert_eq!(n, 2);
        let loaded = Baseline::load(&path).unwrap();
        assert!(loaded.filter(vec![d.clone(), d.clone()]).is_empty());
        assert_eq!(loaded.filter(vec![d.clone(), d.clone(), d]).len(), 1);
    }
}
