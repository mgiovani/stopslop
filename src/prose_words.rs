//! Bulky phrase/word panels for the prose density rules (SLOP014-017), as `LazyLock<Regex>`
//! statics, same convention as `rules::preamble`. Each panel has exactly one consumer (see
//! WP-FOUNDATION spec §0 C5) — this file is a tidy home for the bulky panels, not deduplication.

use regex::Regex;
use std::sync::LazyLock;

// SLOP014 — cliché phrases (case-insensitive). Consumer: rules::cliche.
pub static CLICHE_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(in (today'?s|this|the) (ever-evolving|ever-changing|fast-paced|dynamic|digital|rapidly[ -]changing|modern) (world|landscape|era|age)|in the (ever-)?evolving landscape of|in an era where|in the realm of|unlock(ing)? the (full )?(potential|power) of|harness(ing)? the (full )?power of|embark(s|ed|ing)? (on|upon) (a|this|our|your) journey|navigat(e|es|ing) the (complexities|complexity|landscape|challenges|world) of|(stands?|serves?) as a testament to|is a testament to|a testament to|tapestr(y|ies) of|a treasure trove of|can(not|'?t) be overstated|the power of\s+\w+\s+cannot be)\b").unwrap()
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
    Regex::new(r"(?i)\b(delv(e|es|ed|ing)|underscor(e|es|ed|ing)|showcas(e|es|ed|ing)|meticulous(ly)?|intricate|intricac(y|ies)|commendable|tapestr(y|ies)|testament|boast(s|ed|ing)?|bolster(s|ed|ing)?|garner(s|ed|ing)?|interplay|elucidat(e|es|ed|ing)|unveil(s|ed|ing)?|indelible|quintessential|multifaceted|groundbreaking|seamless(ly)?|holistic|transformative|spearhead(s|ed|ing)?|exemplif(y|ies|ied|ying)|underpin(s|ned|ning)?|myriad|plethora|nuanced|resonat(e|es|ed|ing)|captivat(e|es|ed|ing)|paradigm|synerg(y|ies)|burgeoning|veritable|aforementioned)\b").unwrap()
});
// TIER-2 weight 1 (common):
pub static VOCAB_TIER2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(comprehensive|crucial|robust|leverag(e|es|ed|ing)|foster(s|ed|ing)?|enhanc(e|es|ed|ing)|elevat(e|es|ed|ing)|streamlin(e|es|ed|ing)|facilitat(e|es|ed|ing)|encompass(es|ed|ing)?|navigat(e|es|ed|ing)|amplif(y|ies|ied|ying)|empower(s|ed|ing)?|notably|particularly|additionally|moreover|furthermore|consequently|nevertheless|nonetheless|pivotal|vibrant|landscape|realm|profound(ly)?|dynamic|integral|cohesive|vital|essential|invaluable|ubiquitous|pertinent|salient|valuable|enduring|discerning|advancement(s)?|revolutionary|unprecedented|cutting-edge|versatile|intuitive|keen|adept)\b").unwrap()
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

// SLOP030 — throat-clearing / faux-insight openers. Consumer: rules::opener.
// Anchored sentence/paragraph-initial: the prefix alternation is `^` (with leading markdown
// list/quote/heading markers) OR end-of-previous-sentence punctuation. The phrase itself is
// capture group 1 so the diagnostic column points at the phrase, not the prefix.
// Verified disjoint from CLICHE_PHRASES, HEDGE_PHRASES and residue's RE_OPENER/RE_CLOSER.
pub static THROAT_CLEARING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?im)(?:^[ \t>*_#-]*|[.!?]["')\]]?[ \t]+)(here'?s (?:the (?:thing|kicker|catch|secret|deal|problem|part)|what (?:nobody|no one|most people) (?:tells?|realiz(?:e|es)|miss(?:es)?)|why that matters)|what nobody tells you|what most people (?:miss|get wrong)|let'?s be (?:honest|real|clear)|the (?:truth|reality) is|sound(?:s)? familiar|what if I told you|plot twist|spoiler alert|the bottom line is|make no mistake|but (?:here'?s|there'?s) the (?:thing|catch|rub))"#).unwrap()
});

// SLOP031 — weasel attribution subjects. Consumer: rules::weasel.
// Deliberately excludes `users|developers|data|critics|reports` subjects: "users report a
// crash" and "the data shows 40% latency" are ordinary technical writing, not weaseling.
pub static WEASEL_ATTRIBUTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(experts (?:agree|say|believe|note|warn)|(?:many|most|some) experts\b|according to (?:experts|researchers|studies|scientists)|studies (?:show|suggest|indicate|have shown|have found)|research (?:shows|suggests|indicates|has shown)|a (?:recent|new) study\b|scientists (?:say|believe|agree)|researchers (?:say|found|believe)|it is (?:widely|generally|commonly) (?:believed|known|accepted|agreed)|it'?s (?:widely|generally|commonly) (?:believed|known|accepted)|surveys? (?:show|suggest|indicate)|analysts (?:predict|say|expect))\b").unwrap()
});

// SLOP031 — inline citation signals. A weasel attribution near any of these is attributed
// writing, not a bare appeal to authority. Consumer: rules::weasel.
pub static CITATION_SIGNAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\^[^\]\s]+\]|\[#[^\]\s]*\]_|\[\d+\]|\([^()]*\b(?:19|20)\d{2}[^()]*\)|\bdoi:|https?://").unwrap()
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throat_clearing_compiles_and_matches() {
        assert!(THROAT_CLEARING.is_match("Here's the thing: the cache never expires."));
        assert!(THROAT_CLEARING.is_match("We shipped it. Let's be honest, it was rushed."));
        assert!(!THROAT_CLEARING.is_match("The cache expires after 60 seconds."));
    }

    #[test]
    fn throat_clearing_needs_sentence_initial_anchor() {
        assert!(!THROAT_CLEARING.is_match("Rewriting the truth is hard work for anyone."));
    }

    #[test]
    fn weasel_attribution_compiles_and_matches() {
        assert!(WEASEL_ATTRIBUTION.is_match("Experts agree that caching helps."));
        assert!(WEASEL_ATTRIBUTION.is_match("Studies show a 40% improvement."));
        assert!(!WEASEL_ATTRIBUTION.is_match("Users report a crash on startup."));
        assert!(!WEASEL_ATTRIBUTION.is_match("The data shows 40% lower latency."));
    }

    #[test]
    fn citation_signal_compiles_and_matches() {
        assert!(CITATION_SIGNAL.is_match("as shown in [^1]"));
        assert!(CITATION_SIGNAL.is_match("(Chen et al. 2021)"));
        assert!(CITATION_SIGNAL.is_match("see https://example.org/paper"));
        assert!(!CITATION_SIGNAL.is_match("a plain sentence with no citation"));
    }

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
