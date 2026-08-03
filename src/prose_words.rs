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
// TIER-1 weight 2 (distinctive):
pub static VOCAB_TIER1: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(delv(e|es|ed|ing)|underscor(e|es|ed|ing)|showcas(e|es|ed|ing)|meticulous(ly)?|intricate|intricac(y|ies)|commendable|tapestr(y|ies)|testament|boast(s|ed|ing)?|bolster(s|ed|ing)?|garner(s|ed|ing)?|interplay|elucidat(e|es|ed|ing)|unveil(s|ed|ing)?|indelible|quintessential|multifaceted|groundbreaking|seamless(ly)?|holistic|transformative|spearhead(s|ed|ing)?|exemplif(y|ies|ied|ying)|underpin(s|ned|ning)?|myriad|plethora|nuanced|resonat(e|es|ed|ing)|captivat(e|es|ed|ing)|paradigm|synerg(y|ies)|burgeoning|veritable|aforementioned|beacon(s)?|supercharg(e|es|ed|ing)|ever-evolving)\b").unwrap()
});
// TIER-2 weight 1 (common):
pub static VOCAB_TIER2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(comprehensive|crucial|robust|leverag(e|es|ed|ing)|foster(s|ed|ing)?|enhanc(e|es|ed|ing)|elevat(e|es|ed|ing)|streamlin(e|es|ed|ing)|facilitat(e|es|ed|ing)|encompass(es|ed|ing)?|navigat(e|es|ed|ing)|amplif(y|ies|ied|ying)|empower(s|ed|ing)?|notably|particularly|additionally|moreover|furthermore|consequently|nevertheless|nonetheless|pivotal|vibrant|landscape|realm|profound(ly)?|dynamic|integral|cohesive|vital|essential|invaluable|ubiquitous|pertinent|salient|valuable|enduring|discerning|advancement(s)?|revolutionary|unprecedented|cutting-edge|versatile|intuitive|keen|adept|utiliz(e|es|ed|ing)|harness(es|ed|ing)?|paramount)\b").unwrap()
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
    Regex::new(r",\s+(highlighting|underscoring|emphasizing|showcasing|reflecting|symbolizing|fostering|cultivating|contributing to|reinforcing|solidifying|cementing|reaffirming|underlining|exemplifying|demonstrating|signaling|embodying|encapsulating|marking)\b[^.?!\n]*[.?!]").unwrap()
});

// SLOP027 — empty filler phrases (case-insensitive). Consumer: rules::filler.
// "needless to say" is deliberately OMITTED: it's already in HEDGE_PHRASES (SLOP015) above.
pub static FILLER_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(when it comes to|at its core|in the age of|in the world of|the reality is|the truth is|in terms of|with regard to|with respect to|in order to|going forward|in this article|in this post|let'?s dive in|let'?s take a look|as we'?ve seen|as mentioned earlier|it goes without saying|for all intents and purposes|the fact of the matter is)\b").unwrap()
});

// SLOP027 — filler adverbs. Consumer: rules::filler. Position-gated (capture group 1 is the
// adverb itself): only counts a sentence-initial adverb (leading markdown list/quote/heading
// markers allowed before it) or one directly after a
// copula ("is/are/was/were/be/being/been" or a contracted "'s"/"'re"). This is what keeps
// "simply" in "the simply typed lambda calculus" (mid-sentence, no copula before it) from
// counting, while still catching "It's simply a wrapper" / "Simply put, ...".
pub static FILLER_ADVERBS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?im)(?:^[ \t>*_#-]*|[.!?]["')\]]?[ \t]+|\b(?:is|are|was|were|be|being|been)[ \t]+|'(?:s|re)[ \t]+)(just|literally|honestly|simply|actually|truly|fundamentally|importantly|crucially|inherently|inevitably|basically|essentially|arguably|undoubtedly|obviously|clearly)\b"#).unwrap()
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
        assert!(VOCAB_TIER2.is_match("Uptime is of paramount concern here."));
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
}
