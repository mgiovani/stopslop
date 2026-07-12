use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose::first_byte_per_line;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP011",
    name: "Assistant-response residue in prose",
    tier: Tier::A,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: true,
    path_gated: false,
    check,
};

/// Groups A/B/E (self-ID / knowledge-cutoff disclaimer / speculative gap-filling, refusal
/// boilerplate, reviewer-submission leakage). Anchored anywhere in the masked prose stream --
/// this exact phrasing has no legitimate reason to appear in finished, edited prose.
static RE_ANYWHERE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bas an? (ai|large) language model\b|\bas an ai (assistant|model)\b|\bas of my (last|latest|most recent) (knowledge|training) (update|cutoff|data)\b|\b(up to|as of) my last training update\b|\bmy (knowledge|training) cutoff\b|\bI (do not|don'?t) have (access to|the ability to browse) (real-?time|the internet|current)\b|\bI (cannot|can'?t) browse the internet\b|\bwhile specific details (are|remain) (limited|scarce|unavailable)\b|\bin the (provided|available) (search results|sources)\b|\bbased on (the )?available information\b|\bI'?m (sorry|unable)[, ].{0,40}\b(cannot|can'?t|unable to) (assist|help|provide|generate)\b|\bI cannot generate content that\b|\bI'?m unable to assist with that request\b|\breviewer note\b|\bi hope this message finds you well\b|\bthank you for your review\b|\bplease find (our|the) revised\b|\bwe remain committed to creating content that aligns with\b").unwrap()
});

/// Group C (conversational openers). LINE-INITIAL anchor only: a paragraph that legitimately
/// opens with "Sure, ..." mid-document is ordinary English; residue only when it's literally
/// the first thing on the line (chat-turn register bleeding through).
static RE_OPENER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^[ \t]*(certainly|sure|absolutely|of course|great question)[!,.]|^[ \t]*you'?re absolutely right\b").unwrap()
});

/// Group D (closers). END-OF-LINE/end-of-paragraph anchor: "feel free to reach out to a
/// maintainer" mid-sentence is normal CONTRIBUTING-doc prose; only the chat-closer form --
/// the phrase trailing off the end of the line -- is residue.
static RE_CLOSER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)\b(i hope this helps|hope this helps|let me know if you (need|have|'?d like)|feel free to (reach out|ask)|don'?t hesitate to ask|would you like me to|is there anything else)\b(?:[!.]|\s*$)").unwrap()
});

/// Scope: headings in scope, frontmatter in scope, URLs/link text in scope -- only code (already
/// blanked in `doc.masked`) is excluded. One diagnostic per matching line: track the first
/// (minimum-byte) match per line across all three groups, then emit once per line in line order.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let bytes = [&*RE_ANYWHERE, &*RE_OPENER, &*RE_CLOSER]
        .into_iter()
        .flat_map(|re| re.find_iter(&doc.masked).map(|m| m.start()));
    let by_line = first_byte_per_line(doc, bytes);
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "unedited assistant-response residue in prose",
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
    fn flags_self_id_disclaimer() {
        let diags =
            diagnostics_for("As an AI language model, I don't have access to real-time data.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP011");
    }

    #[test]
    fn flags_refusal_boilerplate() {
        let diags = diagnostics_for("I'm sorry, but I cannot provide that here.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_reviewer_leakage() {
        let diags = diagnostics_for("Reviewer note: please check the config.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_line_initial_opener() {
        let diags = diagnostics_for("Certainly! Here is the updated table.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn ignores_mid_sentence_opener() {
        let diags = diagnostics_for(
            "Our release process is smooth, and sure enough, tests catch regressions.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_end_of_line_closer() {
        let diags = diagnostics_for("If anything's unclear, feel free to reach out!\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn ignores_mid_sentence_closer() {
        let diags = diagnostics_for(
            "Feel free to reach out to a maintainer before starting a large change.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn skips_code_fence() {
        let diags = diagnostics_for(
            "Body text.\n```\nAs an AI language model, I have no opinions.\n```\nMore text here.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn dedupes_multiple_matches_per_line() {
        let diags = diagnostics_for(
            "As an AI language model, I don't have access to real-time information.\n",
        );
        assert_eq!(diags.len(), 1);
    }
}
