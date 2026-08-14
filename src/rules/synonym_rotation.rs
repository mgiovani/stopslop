use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP034",
    name: "Synonym rotation across a closed concept set",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
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
                // `\b{pat}\b` would only bind `\b` to the first alternative.
                re: Regex::new(&format!(r"(?i)\b(?:{pat})\b")).unwrap(),
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

/// For each closed concept set, counts in-scope occurrences per member (frontmatter and URLs
/// skipped). A member "qualifies" once it occurs `>= 2` times. Fires at most one diagnostic per
/// set, only when two or more distinct members qualify, anchored at the first occurrence of the
/// second-seen qualifying member (chronologically, by first occurrence).
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    for set in SETS.iter() {
        let mut qualifying: Vec<(&str, usize)> = Vec::new(); // (name, first_byte)
        for member in set {
            let mut count = 0usize;
            let mut first_byte = None;
            for m in member.re.find_iter(&doc.masked) {
                let byte = m.start();
                if doc.in_frontmatter(byte) || doc.in_url(byte) {
                    continue;
                }
                count += 1;
                first_byte.get_or_insert(byte);
            }
            if count >= 2 {
                qualifying.push((member.name, first_byte.unwrap()));
            }
        }
        if qualifying.len() < 2 {
            continue;
        }
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
    use crate::prose::ProseDoc;

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        let doc = ProseDoc::parse(src);
        let ctx = LintContext {
            display_path: "test.md".to_string(),
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
