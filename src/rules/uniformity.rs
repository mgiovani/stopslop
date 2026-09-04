//! SLOP041 — mechanical uniformity: the one detection axis none of SLOP001-040 measure. Every
//! other rule is a phrase match or an AST shape; nothing else in the crate scores lexical
//! diversity or clause-skeleton reuse across a whole document, so templated prose that rotates
//! its vocabulary just enough to dodge the phrase panels passes every other rule cleanly.
//!
//! Three document-level signals, each individually noisy:
//!   - burstiness: stddev(sentence word counts) / mean, over every sentence in the document.
//!     Human prose varies sentence length a lot; templated prose flattens it.
//!   - type-token ratio (TTR): unique / total lowercased word forms over the first
//!     `TTR_WORD_WINDOW` words. Low TTR means the same words keep coming back.
//!   - trigram repetition: the fraction of word-trigrams that are repeats of an earlier trigram.
//!     High repetition means the same 3-word clause skeletons keep coming back.
//!
//! Firing on ANY ONE signal alone has an unacceptable false-positive rate: short documents make
//! burstiness and TTR noisy (a 40-word doc can trivially read as "uniform" by chance), and
//! narrowly technical documents legitimately reuse the same nouns (an API reference that says
//! "the client" and "the request" forty times is not slop, it's precise). Two independent gates
//! keep this rule from over-firing: a document-length floor (`MIN_DOC_WORDS`) before any signal
//! is even trusted, and a 2-of-3 signal requirement before the rule fires at all. A single tripped
//! signal on a long document — low TTR alone in a terse technical doc, say — stays silent; only
//! when two independent measures agree does the pattern get reported.
//!
//! Deliberate overlap with SLOP030 (`fragmentation::robotic_rhythm`, around line 204 of that
//! file): that check already flags uniform sentence length, but PER PARAGRAPH and using a raw
//! word-count spread. SLOP041's burstiness signal covers the same underlying idea but
//! document-wide, and only counts as one of the three signals this rule needs — it never fires
//! alone. The two lexical signals (TTR, trigram repetition) are the genuinely new material; the
//! 2-of-3 gate is what keeps this from being a louder, document-scoped duplicate of SLOP030.

use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, PROSE_LANGS};
use crate::prose::ProseDoc;
use crate::registry::RuleDef;
use crate::rules::fragmentation;
use std::collections::HashSet;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP041",
    name: "Mechanical uniformity (templated prose)",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

/// Evidence for the five constants below, measured with these values on the two human corpora
/// used throughout this rule's development: 323 pt-BR documents (129 translated Python-docs pages
/// and 194 featured pt.wikipedia articles) and 100 English featured-Wikipedia articles, neither
/// corpus containing any AI-generated text. SLOP041 fired on 35/323 (10.8%) pt-BR documents and
/// 20/100 (20%) English documents at these thresholds. That is not zero: both corpora are
/// naturally narrow, reused-vocabulary prose (translated API reference pages, encyclopedia
/// articles that repeat a subject's name and stock phrasing throughout), the exact shape the
/// module doc comment above calls out as legitimately reusing the same words -- so a double-digit
/// trip rate here is a known property of the rule at these values on this corpus, not a
/// regression to chase in this PR. Per-signal trip rate among the diagnostics that DID fire (a
/// signal's status is only observable in the message when the rule already tripped 2-of-3; a
/// signal that stayed under threshold on a silent document leaves no record to count): pt-BR --
/// burstiness 3/35, type-token ratio 32/35, trigram repetition 35/35; English -- burstiness 8/20,
/// type-token ratio 17/20, trigram repetition 20/20. Trigram repetition is the signal every firing
/// document shares in both corpora; type-token ratio is the frequent second signal; burstiness is
/// the rarest.
const MIN_DOC_WORDS: usize = 200;
/// TTR drifts downward over any long document regardless of style (a 5000-word doc always looks
/// less diverse than a 100-word one); capping the window keeps the measure meaningful.
const TTR_WORD_WINDOW: usize = 500;
/// stddev/mean below this means sentence length barely varies across the whole document.
const BURSTINESS_THRESHOLD: f64 = 0.45;
/// Below this, under half the words in the first window are distinct forms.
const TTR_THRESHOLD: f64 = 0.45;
/// Above this, more than 4% of word-trigrams are repeats of an earlier trigram.
const TRIGRAM_REPETITION_THRESHOLD: f64 = 0.04;
/// Trigram repetition is windowed for the same reason `TTR_WORD_WINDOW` windows type-token ratio:
/// it's a sampling decision, not a threshold: the fraction of repeated trigrams is meant to read
/// as "how repetitive is this document's prose", and an unbounded window answers a different
/// question the more text it's handed ("how repetitive is this document's prose by the time it's
/// this long"), because more distinct trigrams keep accumulating as any long document runs on.
/// The largest document in either measured corpus (see the evidence block on `MIN_DOC_WORDS`
/// above) is pydocs/library-os.md at ~26,030 masked words; 30,000 sits comfortably above every
/// document sampled, so the window changes nothing for real documents and only samples a document
/// that would run longer than any seen so far. This also bounds the `HashSet` the signal builds:
/// unwindowed, that set grew with every word in the document and peaked at 432 MB RSS on a 20 MB
/// single-file input (issue #21 phase-2) -- a useful side effect of the sampling decision, not
/// the reason for it.
const TRIGRAM_WORD_WINDOW: usize = 30_000;

/// Word counts of every sentence in the document, sourced from `fragmentation::paragraph_blocks`
/// so headings/lists/tables/code are excluded exactly as SLOP030 excludes them.
fn all_sentence_word_counts(doc: &ProseDoc) -> Vec<usize> {
    fragmentation::paragraph_blocks(doc)
        .iter()
        .flat_map(|b| fragmentation::split_sentences(&b.text))
        .map(fragmentation::word_count)
        .collect()
}

/// stddev(counts) / mean(counts), population variance (no compelling reason to prefer the
/// sample correction here — this is a descriptive ratio, not an inferential estimate). `None`
/// when there are fewer than 2 sentences or the mean is 0 (nothing to divide by).
fn burstiness(counts: &[usize]) -> Option<f64> {
    if counts.len() < 2 {
        return None;
    }
    let n = counts.len() as f64;
    let mean = counts.iter().sum::<usize>() as f64 / n;
    if mean == 0.0 {
        return None;
    }
    let variance = counts
        .iter()
        .map(|&c| {
            let d = c as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / n;
    Some(variance.sqrt() / mean)
}

/// Lowercased word forms from `doc.masked`, frontmatter excluded — same source `doc.words`
/// itself uses, so fenced/inline code is already blanked out. Punctuation is trimmed off each
/// token's edges (not the middle, so "team's"/"low-latency" stay one token). HTML reads its
/// paragraphs instead: the masked stream also carries nav links, buttons, comments, and `href`
/// values, and a footer that repeats the nav is trigram repetition by construction, not a tell.
/// Stops at `TRIGRAM_WORD_WINDOW`, the wider of the two windows its consumers read, so a 20 MB
/// file no longer materializes millions of lowercase copies it never looks at.
fn masked_words(doc: &ProseDoc) -> Vec<String> {
    let fm_end = doc.frontmatter.map(|(_, e)| e).unwrap_or(0);
    let prose: String;
    let text = if doc.paragraphs.is_some() {
        prose = fragmentation::paragraph_blocks(doc)
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        prose.as_str()
    } else {
        &doc.masked[fm_end..]
    };
    text.split_whitespace()
        .filter_map(|tok| {
            let trimmed = tok.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-');
            (!trimmed.is_empty()).then(|| trimmed.to_lowercase())
        })
        .take(TRIGRAM_WORD_WINDOW)
        .collect()
}

/// unique / total over the first `TTR_WORD_WINDOW` words. `1.0` (maximally diverse, never trips)
/// on an empty window — that can only happen below `MIN_DOC_WORDS`, which already gates the rule
/// off before this is called for real.
fn type_token_ratio(words: &[String]) -> f64 {
    let window = &words[..words.len().min(TTR_WORD_WINDOW)];
    if window.is_empty() {
        return 1.0;
    }
    let unique: HashSet<&str> = window.iter().map(String::as_str).collect();
    unique.len() as f64 / window.len() as f64
}

/// (total trigrams - unique trigrams) / total trigrams: the fraction of trigram occurrences that
/// are a repeat of one already seen earlier in the document, over the first `TRIGRAM_WORD_WINDOW`
/// words. Trigrams are borrowed `(&str, &str, &str)` tuples, never joined into a `String`: the
/// prior version allocated one heap `String` per trigram just to get something `Hash`, which is
/// most of what drove the 432 MB RSS peak `TRIGRAM_WORD_WINDOW`'s doc comment measures.
fn trigram_repetition(words: &[String]) -> f64 {
    let window = &words[..words.len().min(TRIGRAM_WORD_WINDOW)];
    if window.len() < 3 {
        return 0.0;
    }
    let total = window.len() - 2;
    let unique: HashSet<(&str, &str, &str)> = window
        .windows(3)
        .map(|w| (w[0].as_str(), w[1].as_str(), w[2].as_str()))
        .collect();
    (total - unique.len()) as f64 / total as f64
}

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    if doc.words < MIN_DOC_WORDS {
        return;
    }

    let words = masked_words(doc);
    let ttr = type_token_ratio(&words);
    let trigram = trigram_repetition(&words);
    let burst = burstiness(&all_sentence_word_counts(doc));

    let mut tripped = Vec::new();
    if let Some(b) = burst {
        if b < BURSTINESS_THRESHOLD {
            tripped.push(format!("burstiness {b:.2} (< {BURSTINESS_THRESHOLD})"));
        }
    }
    if ttr < TTR_THRESHOLD {
        tripped.push(format!("type-token ratio {ttr:.2} (< {TTR_THRESHOLD})"));
    }
    if trigram > TRIGRAM_REPETITION_THRESHOLD {
        tripped.push(format!(
            "trigram repetition {trigram:.2} (> {TRIGRAM_REPETITION_THRESHOLD})"
        ));
    }

    if tripped.len() >= 2 {
        let message = format!(
            "mechanical uniformity: {} signals tripped ({})",
            tripped.len(),
            tripped.join(", ")
        );
        out.push(Diagnostic::at(rule, ctx, 1, 1, message));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;

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
            image: None,
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn burstiness_matches_hand_computed_value() {
        // counts: 4, 4, 4, 4 -> mean 4, variance 0 -> burstiness 0.0
        assert_eq!(burstiness(&[4, 4, 4, 4]), Some(0.0));
        // counts: 2, 4, 6, 8 -> mean 5, population variance = ((3)^2+(1)^2+(1)^2+(3)^2)/4 = 5,
        // stddev = sqrt(5) ~= 2.236..., burstiness = 2.236/5 ~= 0.447
        let b = burstiness(&[2, 4, 6, 8]).unwrap();
        assert!((b - 0.4472136).abs() < 1e-6, "got {b}");
    }

    #[test]
    fn burstiness_none_below_two_sentences() {
        assert_eq!(burstiness(&[]), None);
        assert_eq!(burstiness(&[5]), None);
    }

    #[test]
    fn type_token_ratio_matches_hand_computed_value() {
        let words: Vec<String> = "the cat sat on the mat"
            .split_whitespace()
            .map(String::from)
            .collect();
        // 6 tokens, 5 unique ("the" repeats) -> 5/6
        let ttr = type_token_ratio(&words);
        assert!((ttr - (5.0 / 6.0)).abs() < 1e-9, "got {ttr}");
    }

    #[test]
    fn type_token_ratio_windowed_to_first_500_words() {
        // 600 identical words followed by 600 distinct ones: only the first 500 (all identical)
        // are in the window, so TTR must reflect that, not the diverse tail.
        let mut words: Vec<String> = std::iter::repeat_n("same".to_string(), 600).collect();
        words.extend((0..600).map(|i| format!("word{i}")));
        let ttr = type_token_ratio(&words);
        assert!((ttr - (1.0 / 500.0)).abs() < 1e-9, "got {ttr}");
    }

    #[test]
    fn trigram_repetition_matches_hand_computed_value() {
        let words: Vec<String> = "a b c a b c a b c"
            .split_whitespace()
            .map(String::from)
            .collect();
        // trigrams: "a b c", "b c a", "c a b", "a b c", "b c a", "c a b", "a b c" (7 total)
        // unique: "a b c", "b c a", "c a b" (3) -> repeats = 7 - 3 = 4 -> 4/7
        let r = trigram_repetition(&words);
        assert!((r - (4.0 / 7.0)).abs() < 1e-9, "got {r}");
    }

    #[test]
    fn trigram_repetition_zero_below_three_words() {
        let words: Vec<String> = vec!["a".to_string(), "b".to_string()];
        assert_eq!(trigram_repetition(&words), 0.0);
    }

    #[test]
    fn trigram_repetition_windowed_to_first_window_words() {
        // Every word beyond TRIGRAM_WORD_WINDOW is fresh and unique; if it leaked into the
        // computation it would pull the ratio down. It must not: only the first
        // TRIGRAM_WORD_WINDOW words are counted, same idea as the TTR window test above.
        let mut words: Vec<String> =
            std::iter::repeat_n("same".to_string(), TRIGRAM_WORD_WINDOW).collect();
        let without_tail = trigram_repetition(&words);
        words.extend((0..5_000).map(|i| format!("word{i}")));
        let with_tail = trigram_repetition(&words);
        assert_eq!(without_tail, with_tail);
        // Every trigram in the all-"same" window is "same same same": one unique trigram.
        assert!(with_tail > 0.99, "got {with_tail}");
    }

    #[test]
    fn short_document_stays_silent_even_if_templated() {
        // Same templated sentence repeated: would trip all three signals, but the document is
        // nowhere near MIN_DOC_WORDS, so the length floor must keep it silent.
        let src = "The tool helps teams ship code. The tool helps teams ship code. The tool helps teams ship code.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn exactly_one_tripped_signal_stays_silent() {
        // A long (>=200 word), lexically ordinary document, but with every sentence built to
        // the same word count so ONLY burstiness trips. If the gate were 1-of-3 instead of
        // 2-of-3, this fixture would fire; it must not.
        let sentences = [
            "The server logs every request that arrives at the edge.",
            "The client sends a token with every call it makes today.",
            "The queue holds messages until a worker picks them up soon.",
            "The cache stores results so repeat lookups return much faster.",
            "The scheduler runs each job once its dependencies have finished.",
            "The database keeps a copy of every row that changes here.",
            "The gateway checks a signature before it forwards any traffic.",
            "The monitor pages an engineer once error rates climb too high.",
            "The archive stores old records for auditors who need them later.",
            "The backup process copies fresh snapshots to a separate region.",
            "The router picks a healthy node before it sends a request out.",
            "The compiler flags a warning when a variable goes unused for long.",
            "The linter blocks a merge until every open issue gets resolved.",
            "The dashboard shows a graph of latency across the whole fleet.",
            "The pipeline waits for every test suite to finish running clean.",
            "The registry stores a manifest for every image that gets pushed.",
            "The proxy strips a header before it forwards the request along.",
            "The tracer records a span for every hop a request takes here.",
            "The sampler drops some spans once the trace volume grows large.",
            "The exporter ships metrics to a backend every fifteen seconds flat.",
        ];
        let src = sentences.join(" ");
        assert!(
            src.split_whitespace().count() >= MIN_DOC_WORDS,
            "fixture too short"
        );
        assert!(diagnostics_for(&src).is_empty());
    }

    #[test]
    fn two_tripped_signals_fires_and_names_them() {
        // A short clause repeated past the word floor trips near-zero burstiness and low TTR.
        // Avoid reusing the same opening word 3+ times, which would also trip SLOP030's
        // separate per-paragraph check.
        let sentence = "Our platform helps teams ship faster and monitor systems better";
        let src = std::iter::repeat_n(sentence, 30)
            .collect::<Vec<_>>()
            .join(". ")
            + ".";
        let diags = diagnostics_for(&src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP041");
        assert_eq!((diags[0].line, diags[0].col), (1, 1));
        assert!(diags[0].message.contains("type-token ratio"));
    }
}
