use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
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
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En],
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
    Regex::new(r"(?i)(?-u:\b)as an? (ai|large) language model(?-u:\b)|(?-u:\b)as an ai (assistant|model)(?-u:\b)|(?-u:\b)as of my (last|latest|most recent) (knowledge|training) (update|cutoff|data)(?-u:\b)|(?-u:\b)(up to|as of) my last training update(?-u:\b)|(?-u:\b)my (knowledge|training) cutoff(?-u:\b)|(?-u:\b)I (do not|don'?t) have (access to|the ability to browse) (real-?time|the internet|current)(?-u:\b)|(?-u:\b)I (cannot|can'?t) browse the internet(?-u:\b)|(?-u:\b)while specific details (are|remain) (limited|scarce|unavailable)(?-u:\b)|(?-u:\b)in the (provided|available) (search results|sources)(?-u:\b)|(?-u:\b)based on (the )?available information(?-u:\b)|(?-u:\b)I'?m (sorry|unable)[, ].{0,40}(?-u:\b)(cannot|can'?t|unable to) (assist|help|provide|generate)(?-u:\b)|(?-u:\b)I cannot generate content that(?-u:\b)|(?-u:\b)I'?m unable to assist with that request(?-u:\b)|(?-u:\b)reviewer note(?-u:\b)|(?-u:\b)i hope this message finds you well(?-u:\b)|(?-u:\b)thank you for your review(?-u:\b)|(?-u:\b)please find (our|the) revised(?-u:\b)|(?-u:\b)we remain committed to creating content that aligns with(?-u:\b)|(?-u:\b)maintains? a low profile(?-u:\b)|(?-u:\b)keeps? (his|her|their) personal (life|details) private(?-u:\b)|(?-u:\b)prefers? to stay out of the spotlight(?-u:\b)|(?-u:\b)likely (grew up|studied|began|started)(?-u:\b)|(?-u:\b)not publicly available(?-u:\b)").unwrap()
});

/// Reasoning-chain leakage: chain-of-thought scaffolding left behind in a deliverable ("let's
/// think about this", "breaking this down", ...). Matched anywhere, same as `RE_ANYWHERE` above --
/// this phrasing has no legitimate reason to survive into finished prose either. Shared with
/// `rules::preamble` (SLOP002, code comments) via `prose_words::REASONING_CHAIN_FRAGMENT` so the
/// phrase list can't drift between the two consumers. `step 1:` is the one member that isn't
/// shared -- it has a legitimate reading, so it needs position context (see `RE_NUMBERED_STEP`).
static RE_REASONING_CHAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?i)(?-u:\b)(?:{REASONING_CHAIN_FRAGMENT})")).unwrap());

/// `Step N:` -- NOT line-initial. Numbered procedural headings and bold lead-ins (`## Step 1:
/// Detect the framework`, `**Step 1: Mine the conversation.**`, `- Step 1: open the file`) are
/// standard technical writing, not chat residue; every hit of the unanchored form across a
/// 130-document corpus was one of those. The residue reading is the phrase surfacing *inside* a
/// sentence ("...so, step 1: we parse the args"), where no author would number a step.
///
/// A markdown structure marker is *required* (`+`, not `*`): the exemption is for `Step N:` that
/// heads a section or list item, so a bare line-initial `Step 1: parse the config` -- which heads
/// nothing -- stays residue. `(?m)` makes `^` a line anchor.
static RE_NUMBERED_STEP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^[ \t]{0,3}(?:#{1,6}[ \t]+|[-*+][ \t]+|\d+\.[ \t]+|\*\*)+step \d+:").unwrap()
});

/// The residue form: `Step N:` with real text before it on the same line.
static RE_MIDLINE_STEP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?-u:\b)step \d+:").unwrap());

/// Acknowledgment loops ("you're asking about X", "to answer your question, ..."). PARAGRAPH-
/// INITIAL only, checked separately below via `fragmentation::paragraph_blocks`: a wrapped
/// continuation line that happens to start with this phrasing mid-paragraph is ordinary English,
/// but the very first line of a paragraph is where a chat-turn acknowledgment actually lands.
static RE_ACK_LOOP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:you(?:'|\u{2019})re asking (?:about|for)|to (?:answer|address) your question)(?-u:\b)",
    )
    .unwrap()
});

/// Group C (conversational openers). LINE-INITIAL anchor only: a paragraph that legitimately
/// opens with "Sure, ..." mid-document is ordinary English; residue only when it's literally
/// the first thing on the line (chat-turn register bleeding through).
static RE_OPENER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)^[ \t]*(certainly|sure|absolutely|of course|great question|excellent question)[!,.]|^[ \t]*(you'?re absolutely right|that'?s an excellent (?:point|question)|happy to help)(?-u:\b)").unwrap()
});

/// Group D (closers). END-OF-LINE/end-of-paragraph anchor: "feel free to reach out to a
/// maintainer" mid-sentence is normal CONTRIBUTING-doc prose; only the chat-closer form --
/// the phrase trailing off the end of the line -- is residue.
static RE_CLOSER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?im)(?-u:\b)(i hope this helps|hope this helps|let me know if you (need|have|'?d like)|feel free to (reach out|ask)|don'?t hesitate to ask|would you like me to|is there anything else)(?-u:\b)(?:[!.]|\s*$)").unwrap()
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
    // A `Step N:` is structure, not residue, when it opens its line behind markdown markers.
    // Both regexes end at the same byte there, so matching end offsets is what distinguishes
    // the two readings.
    let structural_ends: std::collections::HashSet<usize> = RE_NUMBERED_STEP
        .find_iter(&doc.masked)
        .map(|m| m.end())
        .collect();
    let step_bytes: Vec<usize> = RE_MIDLINE_STEP
        .find_iter(&doc.masked)
        .filter(|m| !structural_ends.contains(&m.end()))
        .map(|m| m.start())
        .collect();
    let ack_bytes: Vec<usize> = fragmentation::paragraph_blocks(doc)
        .iter()
        .filter(|b| RE_ACK_LOOP.is_match(&b.text))
        .map(|b| b.first_byte)
        .collect();
    let by_line = first_byte_per_line(doc, re_bytes.chain(ack_bytes).chain(step_bytes));
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

    /// Every `Step N:` hit across a 130-document corpus was one of these -- numbered procedural
    /// writing, not chat residue.
    #[test]
    fn numbered_step_as_structure_is_not_residue() {
        for src in [
            "## Step 1: Detect the framework\n",
            "### Step 1: Load Eval Cases\n",
            "**Step 1: Mine the conversation.** Read what the user already said.\n",
            "- Step 1: open the file\n",
            "1. Step 1: open the file\n",
            "- **Step 2: Verify.** Re-run the suite.\n",
            "# Step 1: Navigate to form\n",
        ] {
            assert!(
                diagnostics_for(src).is_empty(),
                "structure flagged as residue: {src:?}"
            );
        }
    }

    #[test]
    fn numbered_step_mid_sentence_is_still_residue() {
        let diags =
            diagnostics_for("The parser is straightforward, so step 1: we read the args.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP011");
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
