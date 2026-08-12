use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose::first_byte_per_line;
use crate::prose_words::REASONING_CHAIN_FRAGMENT;
use crate::registry::RuleDef;
use crate::rules::fragmentation;
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
/// this exact phrasing has no legitimate reason to appear in finished, edited prose. The
/// speculative-gap-filling family below (`maintains a low profile`, `not publicly available`, ...)
/// is the shape a model falls back on when it has no real biographical fact to report.
static RE_ANYWHERE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bas an? (ai|large) language model\b|\bas an ai (assistant|model)\b|\bas of my (last|latest|most recent) (knowledge|training) (update|cutoff|data)\b|\b(up to|as of) my last training update\b|\bmy (knowledge|training) cutoff\b|\bI (do not|don'?t) have (access to|the ability to browse) (real-?time|the internet|current)\b|\bI (cannot|can'?t) browse the internet\b|\bwhile specific details (are|remain) (limited|scarce|unavailable)\b|\bin the (provided|available) (search results|sources)\b|\bbased on (the )?available information\b|\bI'?m (sorry|unable)[, ].{0,40}\b(cannot|can'?t|unable to) (assist|help|provide|generate)\b|\bI cannot generate content that\b|\bI'?m unable to assist with that request\b|\breviewer note\b|\bi hope this message finds you well\b|\bthank you for your review\b|\bplease find (our|the) revised\b|\bwe remain committed to creating content that aligns with\b|\bmaintains? a low profile\b|\bkeeps? (his|her|their) personal (life|details) private\b|\bprefers? to stay out of the spotlight\b|\blikely (grew up|studied|began|started)\b|\bnot publicly available\b").unwrap()
});

/// Reasoning-chain leakage: chain-of-thought scaffolding left behind in a deliverable ("let's
/// think about this", "step 1:", ...). Matched anywhere, same as `RE_ANYWHERE` above -- this
/// phrasing has no legitimate reason to survive into finished prose either. Shared with
/// `rules::preamble` (SLOP002, code comments) via `prose_words::REASONING_CHAIN_FRAGMENT` so the
/// phrase list can't drift between the two consumers.
static RE_REASONING_CHAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?i)\b(?:{REASONING_CHAIN_FRAGMENT})")).unwrap());

/// Acknowledgment loops ("you're asking about X", "to answer your question, ..."). PARAGRAPH-
/// INITIAL only, checked separately below via `fragmentation::paragraph_blocks`: a wrapped
/// continuation line that happens to start with this phrasing mid-paragraph is ordinary English,
/// but the very first line of a paragraph is where a chat-turn acknowledgment actually lands.
static RE_ACK_LOOP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:you(?:'|\u{2019})re asking (?:about|for)|to (?:answer|address) your question)\b",
    )
    .unwrap()
});

/// Group C (conversational openers). LINE-INITIAL anchor only: a paragraph that legitimately
/// opens with "Sure, ..." mid-document is ordinary English; residue only when it's literally
/// the first thing on the line (chat-turn register bleeding through).
static RE_OPENER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^[ \t]*(certainly|sure|absolutely|of course|great question|excellent question)[!,.]|^[ \t]*(you'?re absolutely right|that'?s an excellent (?:point|question)|happy to help)\b").unwrap()
});

/// Group D (closers). END-OF-LINE/end-of-paragraph anchor: "feel free to reach out to a
/// maintainer" mid-sentence is normal CONTRIBUTING-doc prose; only the chat-closer form --
/// the phrase trailing off the end of the line -- is residue.
static RE_CLOSER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)\b(i hope this helps|hope this helps|let me know if you (need|have|'?d like)|feel free to (reach out|ask)|don'?t hesitate to ask|would you like me to|is there anything else)\b(?:[!.]|\s*$)").unwrap()
});

/// Scope: headings in scope, frontmatter in scope, URLs/link text in scope -- only code (already
/// blanked in `doc.masked`) is excluded. One diagnostic per matching line: track the first
/// (minimum-byte) match per line across all groups (the anywhere/opener/closer regexes plus the
/// paragraph-initial acknowledgment-loop check), then emit once per line in line order.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let re_bytes = [
        &*RE_ANYWHERE,
        &*RE_REASONING_CHAIN,
        &*RE_OPENER,
        &*RE_CLOSER,
    ]
    .into_iter()
    .flat_map(|re| re.find_iter(&doc.masked).map(|m| m.start()));
    let ack_bytes: Vec<usize> = fragmentation::paragraph_blocks(doc)
        .iter()
        .filter(|b| RE_ACK_LOOP.is_match(&b.text))
        .map(|b| b.first_byte)
        .collect();
    let by_line = first_byte_per_line(doc, re_bytes.chain(ack_bytes));
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            "unedited assistant-response residue in prose",
            "delete the sentence",
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
    fn diagnostic_carries_a_fix_hint() {
        let diags = diagnostics_for("Reviewer note: please check the config.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].fix.as_deref(), Some("delete the sentence"));
    }

    #[test]
    fn flags_new_opener_markers() {
        let diags = diagnostics_for(
            "Excellent question! Let's look at the logs.\n\nThat's an excellent point about retries.\n\nHappy to help with the migration.\n",
        );
        assert_eq!(diags.len(), 3);
        assert!(diags.iter().all(|d| d.code == "SLOP011"));
    }

    #[test]
    fn flags_speculative_gap_filling_markers() {
        let cases = [
            "The author maintains a low profile outside of work.\n",
            "She keeps her personal life private from the press.\n",
            "He prefers to stay out of the spotlight entirely.\n",
            "The engineer likely grew up in the same region as the team.\n",
            "A verified birth date is not publicly available for this person.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP011");
        }
    }

    #[test]
    fn flags_reasoning_chain_leakage_markers() {
        let cases = [
            "Let's think about this differently before shipping the fix.\n",
            "Let\u{2019}s think about this differently before shipping the fix.\n",
            "Let me think about the right way to phrase this.\n",
            "Thinking through this carefully before writing the final answer.\n",
            "Step 1: parse the config file into a struct.\n",
            "Breaking this down into smaller pieces makes it easier to review.\n",
            "First, let's consider what happens when the queue backs up.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP011");
        }
    }

    #[test]
    fn flags_paragraph_initial_acknowledgment_loop() {
        let cases = [
            "You're asking about the retry budget, so here is how it works.\n",
            "You\u{2019}re asking for a walkthrough of the deploy pipeline.\n",
            "To answer your question, the timeout defaults to thirty seconds.\n",
            "To address your question directly, caching is enabled by default.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP011");
        }
    }

    #[test]
    fn ignores_mid_paragraph_acknowledgment_loop() {
        // Same phrasing, but not the first line of its paragraph -- ordinary English, not a
        // chat-turn acknowledgment bleeding through.
        let src = "The config file controls most of the runtime behavior.\nYou're asking about the retry budget specifically here.\n";
        assert!(diagnostics_for(src).is_empty());
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
