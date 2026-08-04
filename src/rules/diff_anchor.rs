use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose::{first_byte_per_line, ProseDoc};
use crate::registry::RuleDef;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP036",
    name: "Diff-anchored documentation",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: false,
    path_gated: false,
    check,
};

/// Documentation that narrates a change instead of describing the current state.
///
/// The catalog's `no longer (uses|needs|requires|supports)` alternative is deliberately DROPPED:
/// it can't be implemented deterministically. Unlike every other alternative here, "no longer
/// requires X" is indistinguishable by regex from an ordinary PRESENT-TENSE capability
/// description ("a traffic spike no longer requires an emergency deploy") that never narrates a
/// change at all -- it just describes what's true now. Concretely, it false-positives on real,
/// non-slop prose elsewhere in this repo's own fixture corpus (clean_hedging.md), with no nearby
/// textual signal (no "before"/"previously"/"used to") to disambiguate the two readings.
static DIFF_ANCHOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b((?:was|were) (?:added|introduced|removed|renamed|replaced|changed|updated) (?:to|in|for|with)|this (?:replaces|supersedes|deprecates) the (?:old|previous|former|legacy)|previously,? (?:this|it|the \w+) (?:was|used|had|would)|we(?:'ve| have) (?:changed|updated|switched|migrated|moved) (?:this|it|the)|now uses \w+ instead of|used to (?:be|use|have|require)|in the old (?:version|implementation|code))\b")
        .unwrap()
});

/// Headings under which diff-anchored narration is expected and exempt.
const EXEMPT_HEADINGS: &[&str] = &[
    "changelog",
    "release notes",
    "migration",
    "migration guide",
    "upgrading",
    "what's new",
    "breaking changes",
    "deprecations",
];

/// Basenames (uppercased) that exempt the whole file.
const EXEMPT_FILE_PREFIXES: &[&str] = &[
    "CHANGELOG",
    "RELEASE",
    "NEWS",
    "HISTORY",
    "MIGRATION",
    "UPGRADING",
];

fn file_exempt(display_path: &str) -> bool {
    let base = Path::new(display_path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("")
        .to_uppercase();
    EXEMPT_FILE_PREFIXES.iter().any(|p| base.starts_with(p))
}

/// True if the nearest heading at or before `byte` is one of the exempt "this document narrates
/// change on purpose" section titles.
fn under_exempt_heading(doc: &ProseDoc, byte: usize) -> bool {
    doc.headings
        .iter()
        .filter(|h| h.byte_start <= byte)
        .max_by_key(|h| h.byte_start)
        .is_some_and(|h| EXEMPT_HEADINGS.contains(&h.text.trim().to_lowercase().as_str()))
}

/// Scans the masked prose stream for diff-anchored narration (headings in scope, frontmatter
/// and URLs skipped). The whole file is exempt when its basename looks like a changelog/release
/// note; a match is exempt when it falls under an exempt heading (a changelog embedded in
/// ordinary docs). One diagnostic per matching line.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    if file_exempt(&ctx.display_path) {
        return;
    }

    let bytes = DIFF_ANCHOR_RE
        .find_iter(&doc.masked)
        .map(|m| m.start())
        .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte))
        .filter(|&byte| !under_exempt_heading(doc, byte));
    let by_line = first_byte_per_line(doc, bytes);
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            "documentation narrates a change instead of describing current behavior",
            "describe what the code does now",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::ProseDoc;

    fn diagnostics_for_path(src: &str, display_path: &str) -> Vec<Diagnostic> {
        let doc = ProseDoc::parse(src);
        let ctx = LintContext {
            display_path: display_path.to_string(),
            source: src,
            tree: None,
            lang: Lang::Md,
            comments: &doc.ignore_comments,
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: Some(&doc),
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        diagnostics_for_path(src, "test.md")
    }

    #[test]
    fn flags_was_added_to() {
        let src = "The retry option was added to the client last quarter.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP036");
    }

    #[test]
    fn clean_no_longer_requires_is_deliberately_unclaimed() {
        // "no longer requires" is dropped from the panel (see the static's doc comment): it
        // reads just as naturally as a present-tense capability description as it does diff
        // narration, and this rule must stay silent on it either way.
        let src = "A traffic spike no longer requires an emergency deploy.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_used_to_be() {
        let src = "The endpoint used to be synchronous before this release.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn clean_present_tense_description() {
        let src = "The client retries once on a timeout and logs the failure.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn file_exempt_by_changelog_basename() {
        let src = "The retry option was added to the client last quarter.\n";
        assert!(diagnostics_for_path(src, "docs/CHANGELOG.md").is_empty());
        assert!(diagnostics_for_path(src, "release-notes/RELEASE.md").is_empty());
    }

    #[test]
    fn exempt_under_changelog_heading() {
        let src = "# Changelog\n\nThe retry option was added to the client last quarter.\n\n# Usage\n\nThe client retries once on a timeout.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_when_outside_the_exempt_heading() {
        let src = "# Changelog\n\nThe retry option was added to the client last quarter.\n\n# Usage\n\nThe endpoint used to require manual configuration.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 7);
    }
}
