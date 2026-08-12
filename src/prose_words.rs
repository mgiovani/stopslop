//! Bulky phrase/word panels for the prose density rules (SLOP014-017), as `LazyLock<Regex>`
//! statics, same convention as `rules::preamble`. Each panel has exactly one consumer (see
//! WP-FOUNDATION spec §0 C5) — this file is a tidy home for the bulky panels, not deduplication.

use regex::Regex;
use std::sync::LazyLock;

// SLOP014 — cliché phrases (case-insensitive). Consumer: rules::cliche.
pub static CLICHE_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(in (today'?s|this|the) (ever-evolving|ever-changing|fast-paced|dynamic|digital|rapidly[ -]changing|modern) (world|landscape|era|age)|in the (ever-)?evolving landscape of|in an era where|in the realm of|unlock(ing)? the (full )?(potential|power) of|harness(ing)? the (full )?power of|embark(s|ed|ing)? (on|upon) (a|this|our|your) journey|navigat(e|es|ing) the (complexities|complexity|landscape|challenges|world) of|(stands?|serves?) as a testament to|is a testament to|a testament to|tapestr(y|ies) of|a treasure trove of|can(not|'?t) be overstated|the power of\s+\w+\s+cannot be|game[ -]changer|this changes everything|this is huge|paradigm shift|a beacon of|the holy grail of)\b").unwrap()
});
// NOTE: "when it comes to" is deliberately OMITTED from the default set (catalog: borderline-
// common, opt-down member). Do not add it.

// SLOP015 — hedging / filler phrases (case-insensitive). Consumer: rules::hedging.
// Used with OCCURRENCE COUNTING: iterate find_iter, tally per-phrase (lowercased matched text)
// for the "repeats >= 2" branch and total N for the density branch.
pub static HEDGE_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(it'?s (important|worth|crucial|essential|interesting) (to note|noting|mentioning|to mention|to point out) that|it is (important|worth) (to note|noting|mentioning|to remember|to understand) that|it should be noted that|it'?s (also )?worth (noting|mentioning)|plays? an? (vital|crucial|significant|pivotal|key|important|central) (role|part) in|in conclusion|in summary|to sum up|to summarize|at the end of the day|a (wealth|plethora) of|needless to say|that (being|said) said|first and foremost|last but not least)\b").unwrap()
});

// SLOP016 — vocab marker panel. Consumer: rules::vocabulary.
// TIER-1 weight 2 (distinctive). "paramount" moved here from TIER2 (it was listed for TIER1 in
// the catalog, but was already present in TIER2 from an earlier pass -- a word can only ever be
// in ONE tier, since `vocabulary.rs`'s check() would otherwise double-count a single occurrence
// by matching it in both `find_iter` passes). "ever-evolving" is NOT re-added: it was already
// present in this exact panel from the start.
pub static VOCAB_TIER1: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(delv(e|es|ed|ing)|underscor(e|es|ed|ing)|showcas(e|es|ed|ing)|meticulous(ly)?|intricate|intricac(y|ies)|commendable|tapestr(y|ies)|testament|boast(s|ed|ing)?|bolster(s|ed|ing)?|garner(s|ed|ing)?|interplay|elucidat(e|es|ed|ing)|unveil(s|ed|ing)?|indelible|quintessential|multifaceted|groundbreaking|seamless(ly)?|holistic|transformative|spearhead(s|ed|ing)?|exemplif(y|ies|ied|ying)|underpin(s|ned|ning)?|myriad|plethora|nuanced|resonat(e|es|ed|ing)|captivat(e|es|ed|ing)|paradigm|synerg(y|ies)|burgeoning|veritable|aforementioned|beacon(s)?|supercharg(e|es|ed|ing)|ever-evolving|interconnected|paramount|noteworthy|emblematic|evocative|poignant)\b").unwrap()
});
// TIER-2 weight 1 (common). "utiliz(e|es|ed|ing)" is NOT re-added: it was already present in this
// exact panel from the start. "effortlessly" lives HERE (not in FILLER_ADVERBS/SLOP027) -- see
// filler.rs's doc comment for why the two panels stay disjoint on that word.
pub static VOCAB_TIER2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(comprehensive|crucial|robust|leverag(e|es|ed|ing)|foster(s|ed|ing)?|enhanc(e|es|ed|ing)|elevat(e|es|ed|ing)|streamlin(e|es|ed|ing)|facilitat(e|es|ed|ing)|encompass(es|ed|ing)?|navigat(e|es|ed|ing)|amplif(y|ies|ied|ying)|empower(s|ed|ing)?|notably|particularly|additionally|moreover|furthermore|consequently|nevertheless|nonetheless|pivotal|vibrant|landscape|realm|profound(ly)?|dynamic|integral|cohesive|vital|essential|invaluable|ubiquitous|pertinent|salient|valuable|enduring|discerning|advancement(s)?|revolutionary|unprecedented|cutting-edge|versatile|intuitive|keen|adept|utiliz(e|es|ed|ing)|harness(es|ed|ing)?|effortlessly|performant|imperative)\b").unwrap()
});

// SLOP017 — parallelism sub-patterns. Consumer: rules::parallelism.
// Each list item allows up to 4 space-separated words (bounded, so no runaway matching across a
// whole paragraph): the most common AI "trichotomy" phrasing is short multi-word items ("solo
// developers, growing startups, or established enterprises"), not just single adjectives.
pub static RULE_OF_THREE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:\w+\s+){0,3}\w+,\s+(?:\w+\s+){0,3}\w+,\s+(?:and|or)\s+(?:\w+\s+){0,3}\w+\b")
        .unwrap()
});
pub static NEGATIVE_PARALLELISM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\bnot only\b[^.?!\n]{0,80}?\bbut(\s+also)?\b|\bnot just\b[^.?!\n]{0,60}?\b(but|it'?s)\b|\bit'?s not (just|only)\b[^.?!\n]{0,60}?\bit'?s\b|\bnot an? \w+[^,.?!\n]{0,40},\s*but\b)").unwrap()
});
pub static TRAILING_PARTICIPLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r",\s+(highlighting|underscoring|emphasizing|showcasing|reflecting|symbolizing|fostering|cultivating|contributing to|reinforcing|solidifying|cementing|reaffirming|underlining|exemplifying|demonstrating|signaling|embodying|encapsulating|marking|ensuring|encompassing|enabling|allowing|resulting in|leading to|paving the way for|making it possible|making it easier)\b[^.?!\n]*[.?!]").unwrap()
});

// SLOP027 — empty filler phrases (case-insensitive). Consumer: rules::filler.
// "needless to say" is deliberately OMITTED: it's already in HEDGE_PHRASES (SLOP015) above.
// From the catalog's later addition list, "with regard to", "in terms of", and "the fact of the
// matter is" are NOT re-added: all three were already present in this exact panel from the start.
pub static FILLER_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(when it comes to|at its core|in the age of|in the world of|the reality is|the truth is|in terms of|with regard to|with respect to|in order to|going forward|in this article|in this post|let'?s dive in|let'?s take a look|as we'?ve seen|as mentioned earlier|it goes without saying|for all intents and purposes|the fact of the matter is|out of the box|under the hood|gracefully handles|subsequent to)\b").unwrap()
});

// SLOP027 — filler adverbs. Consumer: rules::filler. Position-gated (capture group 1 is the
// adverb itself): only counts a sentence-initial adverb (leading markdown list/quote/heading
// markers allowed before it) or one directly after a
// copula ("is/are/was/were/be/being/been" or a contracted "'s"/"'re"). This is what keeps
// "simply" in "the simply typed lambda calculus" (mid-sentence, no copula before it) from
// counting, while still catching "It's simply a wrapper" / "Simply put, ...".
// "easily" is the one new addition from the catalog's later pass: "simply"/"just"/"literally"/
// "essentially"/"arguably" were already present in this exact panel from the start, and
// "effortlessly" deliberately lives in `VOCAB_TIER2` (SLOP016) instead, not here -- both panels
// count single words, so the same word can only own one of them (see `prose_words`'s VOCAB_TIER2
// comment).
pub static FILLER_ADVERBS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?im)(?:^[ \t>*_#-]*|[.!?]["')\]]?[ \t]+|\b(?:is|are|was|were|be|being|been)[ \t]+|'(?:s|re)[ \t]+)(just|literally|honestly|simply|actually|truly|fundamentally|importantly|crucially|inherently|inevitably|basically|essentially|arguably|undoubtedly|obviously|clearly|easily)\b"#).unwrap()
});

// SLOP002 / SLOP011 — reasoning-chain scaffolding left behind from a model's chain-of-thought,
// shared between `rules::preamble` (code comments) and `rules::residue` (prose). A pattern
// FRAGMENT, not a compiled `Regex`: the two consumers anchor it differently (preamble.rs anchors
// to a comment-marker line start; residue.rs matches anywhere in the masked prose stream), so
// each compiles its own `Regex` around this shared alternation instead of sharing a `LazyLock`.
// Both ASCII `'` and U+2019 `'` are covered for the two "let's ..." members.
pub const REASONING_CHAIN_FRAGMENT: &str =
    "let(?:'|\u{2019})s think|let me think|thinking through this|step 1:|breaking this down|first, let(?:'|\u{2019})s consider";

// SLOP015 — adjacent hedge stack (case-insensitive). Consumer: rules::hedging. Checked
// independent of the density gate: two hedge words stacked back to back is a defect on a SINGLE
// occurrence, unlike the rest of the panel above which only means something in aggregate.
pub static ADJACENT_HEDGE_STACK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:could|may|might) (?:potentially|possibly|perhaps)\b").unwrap()
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cliche_phrases_compiles_and_matches() {
        assert!(CLICHE_PHRASES.is_match("In today's fast-paced world, teams must adapt."));
        assert!(!CLICHE_PHRASES.is_match("The API returns a 404 when the resource is missing."));
    }

    #[test]
    fn hedge_phrases_compiles_and_matches() {
        assert!(HEDGE_PHRASES.is_match("It's important to note that caching helps."));
        assert!(!HEDGE_PHRASES.is_match("The cache expires after 60 seconds."));
    }

    #[test]
    fn vocab_tier1_compiles_and_matches() {
        assert!(VOCAB_TIER1.is_match("This section delves into the details."));
        assert!(!VOCAB_TIER1.is_match("This section covers the details."));
    }

    #[test]
    fn vocab_tier2_compiles_and_matches() {
        assert!(VOCAB_TIER2.is_match("A comprehensive test suite catches regressions."));
        assert!(!VOCAB_TIER2.is_match("A test suite catches regressions."));
    }

    #[test]
    fn vocab_new_markers_compile_and_match() {
        assert!(VOCAB_TIER1.is_match("The dashboard is a beacon of clarity for the team."));
        assert!(VOCAB_TIER1.is_match("This release will supercharge your workflow."));
        assert!(VOCAB_TIER1.is_match("Built for an ever-evolving product roadmap."));
        assert!(VOCAB_TIER2.is_match("Utilize the cache to cut latency."));
        assert!(VOCAB_TIER2.is_match("Harness the queue to smooth out bursts."));
        // "paramount" moved to TIER1 (see the panel's doc comment); it must not also match TIER2,
        // or a single occurrence would double-count in vocabulary.rs's weighted total.
        assert!(VOCAB_TIER1.is_match("Uptime is of paramount concern here."));
        assert!(!VOCAB_TIER2.is_match("Uptime is of paramount concern here."));
    }

    #[test]
    fn vocab_tier1_second_pass_markers_compile_and_match() {
        assert!(VOCAB_TIER1.is_match("Every service here is deeply interconnected."));
        assert!(VOCAB_TIER1.is_match("This is a noteworthy improvement over last quarter."));
        assert!(VOCAB_TIER1.is_match("The mascot is emblematic of the team's culture."));
        assert!(VOCAB_TIER1.is_match("The essay is an evocative account of the outage."));
        assert!(VOCAB_TIER1.is_match("The farewell message struck a poignant note."));
        assert!(!VOCAB_TIER1.is_match("Every service here is well connected to the network."));
    }

    #[test]
    fn vocab_tier2_second_pass_markers_compile_and_match() {
        assert!(VOCAB_TIER2.is_match("The service handles retries effortlessly."));
        assert!(VOCAB_TIER2.is_match("The new index is far more performant than the old one."));
        assert!(VOCAB_TIER2.is_match("Reviewing the diff before merge is imperative."));
        assert!(!VOCAB_TIER2.is_match("The service handles retries without much trouble."));
    }

    #[test]
    fn cliche_phrases_new_markers_compile_and_match() {
        assert!(CLICHE_PHRASES.is_match("This release is a real game changer for the team."));
        assert!(CLICHE_PHRASES.is_match("Our new game-changer feature ships today."));
        assert!(CLICHE_PHRASES.is_match("Honestly, this changes everything about deployment."));
        assert!(CLICHE_PHRASES.is_match("This is huge for anyone running large clusters."));
        assert!(CLICHE_PHRASES.is_match("The new API represents a paradigm shift for us."));
        assert!(CLICHE_PHRASES.is_match("The library is a beacon of stability in the ecosystem."));
        assert!(CLICHE_PHRASES.is_match("This tool is the holy grail of log parsing."));
    }

    #[test]
    fn filler_phrases_compiles_and_matches() {
        assert!(FILLER_PHRASES.is_match("When it comes to caching, defaults matter."));
        assert!(FILLER_PHRASES.is_match("In order to scale, shard the database."));
        assert!(!FILLER_PHRASES.is_match("The cache expires after 60 seconds."));
        // Already covered by HEDGE_PHRASES (SLOP015); deliberately excluded from this panel.
        assert!(!FILLER_PHRASES.is_match("Needless to say, tests still pass."));
    }

    #[test]
    fn filler_adverbs_compiles_and_matches() {
        assert!(FILLER_ADVERBS.is_match("Basically, the retry loop never terminates."));
        assert!(FILLER_ADVERBS.is_match("It's simply a thin wrapper around the client."));
        assert!(FILLER_ADVERBS.is_match("This is obviously the wrong approach."));
        // Mid-sentence, no copula before it: a term, not filler.
        assert!(!FILLER_ADVERBS.is_match("The simply typed lambda calculus models computation."));
        assert!(!FILLER_ADVERBS.is_match("It ships with robust error handling, not just logging."));
    }

    #[test]
    fn filler_phrases_second_pass_markers_compile_and_match() {
        assert!(FILLER_PHRASES.is_match("This client works out of the box with no setup."));
        assert!(FILLER_PHRASES.is_match("Under the hood, the client batches every request."));
        assert!(FILLER_PHRASES.is_match("The wrapper gracefully handles a dropped connection."));
        assert!(FILLER_PHRASES.is_match("The job runs subsequent to the nightly backup."));
        assert!(!FILLER_PHRASES.is_match("The box ships with a padded interior lining."));
    }

    #[test]
    fn filler_adverbs_second_pass_marker_compiles_and_matches() {
        assert!(FILLER_ADVERBS.is_match("This is easily the simplest fix available."));
        assert!(!FILLER_ADVERBS.is_match("The team shipped the change easily ahead of schedule."));
    }

    #[test]
    fn rule_of_three_compiles_and_matches() {
        assert!(RULE_OF_THREE.is_match("clear, concise, and correct"));
        assert!(!RULE_OF_THREE.is_match("clear and correct"));
    }

    #[test]
    fn rule_of_three_matches_multi_word_list_items() {
        assert!(RULE_OF_THREE.is_match(
            "designed for solo developers, growing startups, or established enterprises"
        ));
    }

    #[test]
    fn negative_parallelism_compiles_and_matches() {
        assert!(NEGATIVE_PARALLELISM.is_match("not only fast but also simple"));
        assert!(!NEGATIVE_PARALLELISM.is_match("fast and simple"));
    }

    #[test]
    fn trailing_participle_compiles_and_matches() {
        assert!(TRAILING_PARTICIPLE.is_match(", underscoring its significance."));
        assert!(!TRAILING_PARTICIPLE.is_match(" running on port 8080."));
    }

    #[test]
    fn trailing_participle_second_pass_verbs_compile_and_match() {
        assert!(TRAILING_PARTICIPLE.is_match(", ensuring every request is retried once."));
        assert!(TRAILING_PARTICIPLE.is_match(", encompassing every downstream service too."));
        assert!(TRAILING_PARTICIPLE.is_match(", enabling teams to ship changes faster."));
        assert!(TRAILING_PARTICIPLE.is_match(", allowing the queue to drain safely."));
        assert!(TRAILING_PARTICIPLE.is_match(", resulting in a much smaller diff overall."));
        assert!(TRAILING_PARTICIPLE.is_match(", leading to fewer on-call pages each week."));
        assert!(TRAILING_PARTICIPLE.is_match(", paving the way for the next migration."));
        assert!(TRAILING_PARTICIPLE.is_match(", making it possible to roll back instantly."));
        assert!(TRAILING_PARTICIPLE.is_match(", making it easier to onboard new engineers."));
        assert!(!TRAILING_PARTICIPLE.is_match(" running on port 8080, enabled by default."));
    }

    #[test]
    fn reasoning_chain_fragment_compiles_and_matches_both_apostrophes() {
        let re = Regex::new(&format!("(?i){REASONING_CHAIN_FRAGMENT}")).unwrap();
        assert!(re.is_match("let's think about this differently"));
        assert!(re.is_match("let\u{2019}s think about this differently"));
        assert!(re.is_match("Step 1: parse the input"));
        assert!(re.is_match("breaking this down further"));
        assert!(!re.is_match("we thought this through carefully"));
    }

    #[test]
    fn adjacent_hedge_stack_compiles_and_matches() {
        assert!(ADJACENT_HEDGE_STACK.is_match("This might potentially fail under load."));
        assert!(ADJACENT_HEDGE_STACK.is_match("It could possibly break older clients."));
        assert!(!ADJACENT_HEDGE_STACK.is_match("This could work in some cases."));
    }
}
