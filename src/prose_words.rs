//! Bulky phrase/word panels for the prose density rules (SLOP014-017), as `LazyLock<Regex>`
//! statics, same convention as `rules::preamble`. Each panel has exactly one consumer (see
//! WP-FOUNDATION spec §0 C5) — this file is a tidy home for the bulky panels, not deduplication.

use regex::Regex;
use std::sync::LazyLock;

// SLOP014 — cliché phrases (case-insensitive). Consumer: rules::cliche.
pub static CLICHE_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(in (today'?s|this|the) (ever-evolving|ever-changing|fast-paced|dynamic|digital|rapidly[ -]changing|modern) (world|landscape|era|age)|in the (ever-)?evolving landscape of|in an era where|in the realm of|unlock(ing)? the (full )?(potential|power) of|harness(ing)? the (full )?power of|embark(s|ed|ing)? (on|upon) (a|this|our|your) journey|navigat(e|es|ing) the (complexities|complexity|landscape|challenges|world) of|(stands?|serves?) as a testament to|is a testament to|a testament to|tapestr(y|ies) of|a treasure trove of|can(not|'?t) be overstated|the power of\s+\w+\s+cannot be|game[ -]changer|this changes everything|this is huge|paradigm shift|a beacon of|the holy grail of)(?-u:\b)").unwrap()
});
// NOTE: "when it comes to" is deliberately OMITTED from the default set (catalog: borderline-
// common, opt-down member). Do not add it.

// SLOP015 — hedging / filler phrases (case-insensitive). Consumer: rules::hedging.
// Used with OCCURRENCE COUNTING: iterate find_iter, tally per-phrase (lowercased matched text)
// for the "repeats >= 2" branch and total N for the density branch.
pub static HEDGE_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(it'?s (important|worth|crucial|essential|interesting) (to note|noting|mentioning|to mention|to point out) that|it is (important|worth) (to note|noting|mentioning|to remember|to understand) that|it should be noted that|it'?s (also )?worth (noting|mentioning)|plays? an? (vital|crucial|significant|pivotal|key|important|central) (role|part) in|in conclusion|in summary|to sum up|to summarize|at the end of the day|a (wealth|plethora) of|needless to say|that (being|said) said|first and foremost|last but not least)(?-u:\b)").unwrap()
});

// SLOP016 — vocab marker panel. Consumer: rules::vocabulary.
// TIER-1 weight 2 (distinctive). A word may only appear in one tier -- `vocabulary.rs` would
// double-count it otherwise. "paramount" belongs here, not TIER2; "ever-evolving" already lives
// here -- don't re-add either.
pub static VOCAB_TIER1: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(delv(e|es|ed|ing)|underscor(e|es|ed|ing)|showcas(e|es|ed|ing)|meticulous(ly)?|intricate|intricac(y|ies)|commendable|tapestr(y|ies)|testament|boast(s|ed|ing)?|bolster(s|ed|ing)?|garner(s|ed|ing)?|interplay|elucidat(e|es|ed|ing)|unveil(s|ed|ing)?|indelible|quintessential|multifaceted|groundbreaking|seamless(ly)?|holistic|transformative|spearhead(s|ed|ing)?|exemplif(y|ies|ied|ying)|underpin(s|ned|ning)?|myriad|plethora|nuanced|resonat(e|es|ed|ing)|captivat(e|es|ed|ing)|paradigm|synerg(y|ies)|burgeoning|veritable|aforementioned|beacon(s)?|supercharg(e|es|ed|ing)|ever-evolving|interconnected|paramount|noteworthy|emblematic|evocative|poignant)(?-u:\b)").unwrap()
});
// TIER-2 weight 1 (common). "utiliz(e|es|ed|ing)" is NOT re-added: it was already present in this
// exact panel from the start. "effortlessly" lives HERE (not in FILLER_ADVERBS/SLOP027) -- see
// filler.rs's doc comment for why the two panels stay disjoint on that word.
pub static VOCAB_TIER2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(comprehensive|crucial|robust|leverag(e|es|ed|ing)|foster(s|ed|ing)?|enhanc(e|es|ed|ing)|elevat(e|es|ed|ing)|streamlin(e|es|ed|ing)|facilitat(e|es|ed|ing)|encompass(es|ed|ing)?|navigat(e|es|ed|ing)|amplif(y|ies|ied|ying)|empower(s|ed|ing)?|notably|particularly|additionally|moreover|furthermore|consequently|nevertheless|nonetheless|pivotal|vibrant|landscape|realm|profound(ly)?|dynamic|integral|cohesive|vital|essential|invaluable|ubiquitous|pertinent|salient|valuable|enduring|discerning|advancement(s)?|revolutionary|unprecedented|cutting-edge|versatile|intuitive|keen|adept|utiliz(e|es|ed|ing)|harness(es|ed|ing)?|effortlessly|performant|imperative)(?-u:\b)").unwrap()
});

// SLOP017 — parallelism sub-patterns. Consumer: rules::parallelism.
// Bounded to 4 words/item so matching can't run away across a paragraph; AI "trichotomy"
// phrasing (e.g. "solo devs, growing startups, or enterprises") uses short multi-word items,
// not single adjectives.
pub static RULE_OF_THREE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u:\b)(?:\w+\s+){0,3}\w+,\s+(?:\w+\s+){0,3}\w+,\s+(?:and|or)\s+(?:\w+\s+){0,3}\w+(?-u:\b)")
        .unwrap()
});
/// Finite verbs and subject pronouns. An enumeration's items are noun phrases or adjectives
/// ("clear", "package imports"); an item carrying one of these is a CLAUSE, which makes the whole
/// match a compound sentence rather than a list -- "...findings from one rule, it absorbs exactly
/// three, and a fourth is reported" is three clauses, not a tricolon. Consumer:
/// rules::parallelism.
pub static CLAUSE_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(is|are|was|were|be|been|being|has|have|had|does|do|did|will|would|shall|should|can|could|may|might|must|it|they|we|you|i)(?-u:\b)").unwrap()
});

pub static NEGATIVE_PARALLELISM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)((?-u:\b)not only(?-u:\b)[^.?!\n]{0,80}?(?-u:\b)but(\s+also)?(?-u:\b)|(?-u:\b)not just(?-u:\b)[^.?!\n]{0,60}?(?-u:\b)(but|it'?s)(?-u:\b)|(?-u:\b)it'?s not (just|only)(?-u:\b)[^.?!\n]{0,60}?(?-u:\b)it'?s(?-u:\b)|(?-u:\b)not an? \w+[^,.?!\n]{0,40},\s*but(?-u:\b))").unwrap()
});
pub static TRAILING_PARTICIPLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r",\s+(highlighting|underscoring|emphasizing|showcasing|reflecting|symbolizing|fostering|cultivating|contributing to|reinforcing|solidifying|cementing|reaffirming|underlining|exemplifying|demonstrating|signaling|embodying|encapsulating|marking|ensuring|encompassing|enabling|allowing|resulting in|leading to|paving the way for|making it possible|making it easier)(?-u:\b)[^.?!\n]*[.?!]").unwrap()
});

// SLOP027 — empty filler phrases. "needless to say" is omitted (already in HEDGE_PHRASES/
// SLOP015). "with regard to", "in terms of", "the fact of the matter is" are already here from
// an earlier pass -- don't re-add.
pub static FILLER_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(when it comes to|at its core|in the age of|in the world of|the reality is|the truth is|in terms of|with regard to|with respect to|in order to|going forward|in this article|in this post|let'?s dive in|let'?s take a look|as we'?ve seen|as mentioned earlier|it goes without saying|for all intents and purposes|the fact of the matter is|out of the box|under the hood|gracefully handles|subsequent to)(?-u:\b)").unwrap()
});

// SLOP027 — filler adverbs. Position-gated: only a sentence-initial adverb or one after a
// copula counts, so "simply" in "the simply typed lambda calculus" doesn't count, but "It's
// simply a wrapper" does. "effortlessly" lives in VOCAB_TIER2 instead.
pub static FILLER_ADVERBS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?im)(?:^[ \t>*_#-]*|[.!?]["')\]]?[ \t]+|(?-u:\b)(?:is|are|was|were|be|being|been)[ \t]+|'(?:s|re)[ \t]+)(just|literally|honestly|simply|actually|truly|fundamentally|importantly|crucially|inherently|inevitably|basically|essentially|arguably|undoubtedly|obviously|clearly|easily)(?-u:\b)"#).unwrap()
});

// SLOP002/SLOP011 — reasoning-chain scaffolding, shared by `rules::preamble` (code) and
// `rules::residue` (prose) as a FRAGMENT, not a compiled `Regex`, since each anchors it
// differently. "step 1:" is excluded: it's legitimate procedural writing needing per-consumer
// position context.
pub const REASONING_CHAIN_FRAGMENT: &str =
    "let(?:'|\u{2019})s think|let me think|thinking through this|breaking this down|first, let(?:'|\u{2019})s consider";

// SLOP015 — adjacent hedge stack (case-insensitive). Consumer: rules::hedging. Checked
// independent of the density gate: two hedge words stacked back to back is a defect on a SINGLE
// occurrence, unlike the rest of the panel above which only means something in aggregate.
pub static ADJACENT_HEDGE_STACK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:could|may|might) (?:potentially|possibly|perhaps)(?-u:\b)").unwrap()
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
        assert!(re.is_match("breaking this down further"));
        assert!(!re.is_match("we thought this through carefully"));
        // "step 1:" deliberately left out: it needs per-consumer position context, so each
        // consumer owns it (see the fragment's doc comment).
        assert!(!re.is_match("Step 1: parse the input"));
    }

    #[test]
    fn adjacent_hedge_stack_compiles_and_matches() {
        assert!(ADJACENT_HEDGE_STACK.is_match("This might potentially fail under load."));
        assert!(ADJACENT_HEDGE_STACK.is_match("It could possibly break older clients."));
        assert!(!ADJACENT_HEDGE_STACK.is_match("This could work in some cases."));
    }

    fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    // A Unicode word boundary drops the regex crate from its lazy DFA to the PikeVM as soon as
    // the haystack has one non-ASCII byte, so every boundary must stay ASCII-scoped (issue #21).
    /// A bare Unicode boundary is never allowed: every `\b`, English or Portuguese, must read
    /// `(?-u:\b)`. The one exemption is a line whose trimmed start is `//`, so a doc comment may
    /// name the boundary it is explaining.
    fn first_unscoped_boundary(bytes: &[u8]) -> Option<usize> {
        #[allow(clippy::byte_char_slices)] // spelling the needle as a literal would trip the scan
        let needle = [b'\\', b'b'];
        let mut line_start = 0;
        bytes.split(|&b| b == b'\n').find_map(|line| {
            let start = line_start;
            line_start += line.len() + 1;
            if line.trim_ascii_start().starts_with(b"//") {
                return None;
            }
            line.windows(2)
                .enumerate()
                .filter(|(_, w)| *w == needle)
                .map(|(pos, _)| pos)
                .find(|&pos| pos < 5 || &line[pos - 5..pos] != b"(?-u:")
                .map(|pos| start + pos)
        })
    }

    #[test]
    #[allow(clippy::byte_char_slices)]
    fn unscoped_boundary_scan_flags_only_bare_boundaries() {
        assert_eq!(first_unscoped_boundary(br"(?i)(?-u:\b)word(?-u:\b)"), None);
        let mut bare = br"(?i)(?-u:\b)word".to_vec();
        bare.extend([b'\\', b'b']);
        assert_eq!(first_unscoped_boundary(&bare), Some(16));
        let mut second_line = b"(?i)ascii\n".to_vec();
        second_line.extend(bare.iter());
        assert_eq!(first_unscoped_boundary(&second_line), Some(26));
        // An accented line no longer gets a pass -- a Portuguese panel's bare `\b` is just as
        // unscoped as an English one's.
        let mut accented = "(?i)n[ãa]o".as_bytes().to_vec();
        accented.extend([b'\\', b'b']);
        assert_eq!(first_unscoped_boundary(&accented), Some(11));
        // Built with `extend`: a literal escape on this line would trip the self-scan below.
        let mut comment = b"    // uses ".to_vec();
        comment.extend([b'\\', b'b']);
        comment.extend(b"word");
        comment.extend([b'\\', b'b']);
        comment.extend(b" as an example");
        assert_eq!(first_unscoped_boundary(&comment), None);
    }

    #[test]
    fn every_regex_word_boundary_is_ascii_scoped() {
        let mut files = Vec::new();
        collect_rs_files(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        assert!(!files.is_empty());
        for path in &files {
            let bytes = std::fs::read(path).unwrap();
            assert!(
                first_unscoped_boundary(&bytes).is_none(),
                "{}: a regex word boundary on an ASCII-only line is not ASCII-scoped, write (?-u:) in front of it",
                path.display()
            );
        }
    }
}
