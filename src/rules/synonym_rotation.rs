use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PARAGRAPH_LANGS};
use crate::registry::RuleDef;
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP034",
    name: "Synonym rotation across a closed concept set",
    tier: Tier::B,
    langs: PARAGRAPH_LANGS,
    natlangs: &[NatLang::En],
    default_on: true,
    path_gated: false,
    check,
};

struct Member {
    name: &'static str,
    re: Regex,
}

/// Ten closed concept sets, each member word-bounded/case-insensitive and stem-tolerant where
/// sensible (verb inflections; bare adjectives left uninflected). The catalog's "use / utilize /
/// leverage / employ" set is narrowed to "use / employ": "utilize" and "leverage" are already
/// matched bare by prose_words.rs's VOCAB_TIER2 (SLOP016) -- keeping them here would double-flag
/// the same span under two rules.
static SETS: LazyLock<Vec<Vec<Member>>> = LazyLock::new(|| {
    fn set(members: &[(&'static str, &str)]) -> Vec<Member> {
        members
            .iter()
            .map(|&(name, pat)| Member {
                name,
                // `pat` may itself be a top-level alternation (e.g. "run(?:s|ning)?|ran"), so
                // the boundary anchors must wrap the whole thing in a non-capturing group --
                // `(?-u:\b){pat}(?-u:\b)` would only bind `(?-u:\b)` to the first alternative.
                re: Regex::new(&format!(r"(?i)(?-u:\b)(?:{pat})(?-u:\b)")).unwrap(),
            })
            .collect()
    }
    vec![
        set(&[
            ("check", r"check(?:s|ed|ing)?"),
            ("verify", r"verif(?:y|ies|ied|ying)"),
            ("confirm", r"confirm(?:s|ed|ing)?"),
            ("validate", r"validat(?:e|es|ed|ing)"),
        ]),
        set(&[
            ("config", r"config"),
            ("configuration", r"configurations?"),
            ("settings", r"settings"),
        ]),
        set(&[
            ("delete", r"delet(?:e|es|ed|ing)"),
            ("remove", r"remov(?:e|es|ed|ing)"),
        ]),
        set(&[
            ("run", r"run(?:s|ning)?|ran"),
            ("execute", r"execut(?:e|es|ed|ing)"),
            ("invoke", r"invok(?:e|es|ed|ing)"),
            ("launch", r"launch(?:es|ed|ing)?"),
        ]),
        set(&[
            ("show", r"show(?:s|ed|ing|n)?"),
            ("display", r"display(?:s|ed|ing)?"),
        ]),
        // "utilize"/"leverage" dropped: see module doc comment above.
        set(&[
            ("use", r"us(?:e|es|ed|ing)"),
            ("employ", r"employ(?:s|ed|ing)?"),
        ]),
        set(&[
            ("fast", r"fast"),
            ("quick", r"quick(?:ly)?"),
            ("rapid", r"rapid(?:ly)?"),
            ("speedy", r"speedy"),
        ]),
        set(&[
            ("start", r"start(?:s|ed|ing)?"),
            ("begin", r"begin(?:s|ning)?|began|begun"),
            ("commence", r"commenc(?:e|es|ed|ing)"),
            ("initiate", r"initiat(?:e|es|ed|ing)"),
        ]),
        set(&[
            ("create", r"creat(?:e|es|ed|ing)"),
            ("generate", r"generat(?:e|es|ed|ing)"),
            ("produce", r"produc(?:e|es|ed|ing)"),
        ]),
        set(&[
            ("change", r"chang(?:e|es|ed|ing)"),
            ("modify", r"modif(?:y|ies|ied|ying)"),
            ("alter", r"alter(?:s|ed|ing)?"),
            ("adjust", r"adjust(?:s|ed|ing)?"),
        ]),
    ]
});

/// For each closed concept set, counts occurrences per member within one SECTION's running prose.
/// A member "qualifies" once it occurs `>= 2` times there. Fires at most one diagnostic per set,
/// only when two or more distinct members qualify in the same section, anchored at the first
/// occurrence of the second-seen qualifying member (chronologically, by first occurrence).
///
/// Scope is a section rather than the whole file because a document that enumerates
/// differently-named things gets its "competing" words from unrelated entries -- a skill catalog
/// flagged `generate, create` where the two words described two different skills. Two scope
/// decisions, and both are load-bearing:
///
/// - **Sections, not paragraphs.** Genuine rotation is one author drifting across a passage; the
///   rule's own fixture spreads `check` x2 and `verify` x2 over four consecutive paragraphs.
///   Paragraph scoping would delete that true positive.
/// - **Only `fragmentation::paragraph_blocks`.** That helper already drops headings, list items,
///   tables, rules, link-reference definitions, and comment lines -- "bullet lists, tables, and
///   headings must never be treated as sentences", per its own doc comment. Both known false
///   positives were list items, and a 17-bullet catalog under one heading is not fixed by section
///   scoping alone. This also subsumes the separate "skip tokens in headings" concern.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    let blocks = super::fragmentation::paragraph_blocks(doc);
    if blocks.is_empty() {
        return;
    }
    let heading_starts: Vec<usize> = doc.headings.iter().map(|h| h.byte_start).collect();
    let section_of = |byte: usize| heading_starts.partition_point(|&s| s <= byte);
    let in_prose = |byte: usize| {
        blocks
            .iter()
            .any(|b| byte >= b.first_byte && byte < b.end_byte)
    };

    for set in SETS.iter() {
        // section -> [(member name, first byte in that section)]
        let mut by_section: BTreeMap<usize, Vec<(&str, usize)>> = BTreeMap::new();
        for member in set {
            let mut per_section: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
            for m in member.re.find_iter(&doc.masked) {
                let byte = m.start();
                if doc.in_frontmatter(byte) || doc.in_url(byte) || !in_prose(byte) {
                    continue;
                }
                per_section.entry(section_of(byte)).or_insert((0, byte)).0 += 1;
            }
            for (section, (count, first_byte)) in per_section {
                if count >= 2 {
                    by_section
                        .entry(section)
                        .or_default()
                        .push((member.name, first_byte));
                }
            }
        }
        // At most one diagnostic per set, as before: the earliest section that rotates.
        let Some(mut qualifying) = by_section.into_values().find(|members| members.len() >= 2)
        else {
            continue;
        };
        qualifying.sort_by_key(|&(_, byte)| byte);
        let first_member = qualifying[0].0;
        let anchor_byte = qualifying[1].1;
        let names: Vec<&str> = qualifying.iter().map(|&(name, _)| name).collect();
        let (line, col) = doc.line_col(anchor_byte);
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            format!(
                "synonym rotation across a closed concept set: {}",
                names.join(", ")
            ),
            format!("pick one term and use it throughout: {first_member}"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::prose::ProseDoc;

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        let doc = ProseDoc::parse(src);
        let ctx = LintContext {
            display_path: "test.md".to_string(),
            source: src,
            index: None,
            lang: Lang::Md,
            comments: &doc.ignore_comments,
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: Some(&doc),
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn flags_rotation_between_two_qualifying_members() {
        let src = "Check the response. Check it twice. Now verify the response. Verify it again.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP034");
        assert!(diags[0].message.contains("check"));
        assert!(diags[0].message.contains("verify"));
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("pick one term and use it throughout: check")
        );
    }

    /// A catalog of differently-named things: the competing words describe different entries, so
    /// they are not one author rotating terms. Both are real strings from a 130-document corpus.
    #[test]
    fn list_items_are_not_pooled_together() {
        let catalog = "## Development\n\n- **ci-generate**: Generate a production-ready CI/CD pipeline config\n- **docs-check**: Check documentation against the codebase and report drift\n- **db-migrate**: Create, validate, and manage database migrations across any framework\n- **test-suite**: Generate test suites by analyzing coverage gaps\n";
        assert!(diagnostics_for(catalog).is_empty());

        let changelog = "## Security\n\n- Phase 3: Parallel Vulnerability Scanning\n  - Agent 1: Access Control & Authentication (A01, A07)\n  - Agent 2: Configuration & Insecure Design (A02, A06)\n  - Agent 3: Injection & Data Integrity (A05, A08)\n\nThe config file is read once at startup.\n";
        assert!(diagnostics_for(changelog).is_empty());
    }

    /// Rotation split across two sections is two authors' vocabularies, not one drifting.
    #[test]
    fn sections_are_counted_separately() {
        let src = "## Setup\n\nCheck the response. Check it twice.\n\n## Teardown\n\nNow verify the response. Verify it again.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// The rule's own fixture spreads the two members over four consecutive paragraphs under one
    /// heading -- paragraph scoping would have deleted this true positive.
    #[test]
    fn rotation_pools_across_paragraphs_within_one_section() {
        let src = "# Deployment Checklist\n\nCheck the health endpoint before promoting.\n\nCheck it again after five minutes.\n\nNow verify the endpoint reports steady latency.\n\nVerify it once more before closing.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP034");
    }

    #[test]
    fn anchors_at_second_seen_member_first_occurrence() {
        // "check" is seen first (word 1), "verify" second (first appears at word 3); anchor
        // must land on that second member's first occurrence, not the very first word.
        let src = "Check it. Verify it. Check it again. Verify it again.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        let verify_byte = src.find("Verify").unwrap();
        let doc = ProseDoc::parse(src);
        let (line, col) = doc.line_col(verify_byte);
        assert_eq!((diags[0].line, diags[0].col), (line, col));
    }

    #[test]
    fn clean_single_member_repeated_alone() {
        // Only one member of the set qualifies; a lone repeated word choice is not rotation.
        let src = "Check the config. Check it again. Check it once more.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_second_member_appears_only_once() {
        // "verify" only appears once, so it never qualifies (needs >= 2).
        let src = "Check the response. Check it twice. Now verify the response once.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_claim_utilize_or_leverage() {
        // Both belong to VOCAB_TIER2 (SLOP016) already; this rule's "use/employ" set must stay
        // silent on them even when repeated.
        let src = "We utilize the cache. We utilize it again. We leverage the queue. We leverage it again.\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
