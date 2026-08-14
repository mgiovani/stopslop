use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose_words::{FILLER_ADVERBS, FILLER_PHRASES};
use crate::registry::RuleDef;
use std::collections::HashMap;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP027",
    name: "Empty filler phrase & adverb density",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: true,
    path_gated: false,
    check,
};

/// Counts empty filler-phrase ("when it comes to", "in order to", ...) and filler-adverb
/// ("basically", "obviously", ...) occurrences in the masked prose stream (headings in scope,
/// frontmatter and URLs skipped, same scope as the SLOP015 hedging density rule this is modeled
/// on). Phrases are weighted 2, adverbs weighted 1 (phrases are the stronger signal: a whole
/// empty clause vs. a single word that occasionally belongs). Fires document-level, anchored at
/// the first in-scope occurrence, when the weighted total N meets the density floor
/// (`N >= max(6, ceil(6 * words / 1000))`) OR any single PHRASE (not adverb -- a common adverb
/// repeating twice is unremarkable, a repeated empty clause isn't) repeats `>= 2` times.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let mut weighted = 0usize;
    let mut first_byte: Option<usize> = None;
    let mut per_phrase: HashMap<String, usize> = HashMap::new();

    // Two separate regexes feed the same `first_byte` anchor, so each hit must be merged in by
    // min (not just "set once"): the earliest match overall may come from either loop.
    for m in FILLER_PHRASES.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) {
            continue;
        }
        weighted += 2;
        first_byte = Some(first_byte.map_or(byte, |b| b.min(byte)));
        *per_phrase.entry(m.as_str().to_lowercase()).or_insert(0) += 1;
    }
    for caps in FILLER_ADVERBS.captures_iter(&doc.masked) {
        let g = caps.get(1).unwrap();
        let byte = g.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) {
            continue;
        }
        weighted += 1;
        first_byte = Some(first_byte.map_or(byte, |b| b.min(byte)));
    }

    // Integer ceil of 6 * words / 1000, floored at an absolute minimum of 6 (weighted units;
    // phrases count double, so this floor is equivalent to hedging.rs's "3 hedges" floor
    // expressed in phrase-equivalents).
    let threshold = (6 * doc.words).div_ceil(1000).max(6);
    let repeated_phrase = per_phrase.iter().find(|&(_, &n)| n >= 2).map(|(p, _)| p);
    if weighted >= threshold || repeated_phrase.is_some() {
        let (line, col) = doc.line_col(first_byte.unwrap());
        let message = if weighted >= threshold {
            format!(
                "high density of empty filler phrases/adverbs ({weighted} weighted occurrences vs threshold {threshold})"
            )
        } else {
            let phrase = repeated_phrase.unwrap();
            format!("filler phrase repeated: \"{phrase}\" appears multiple times")
        };
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            message,
            "delete the filler",
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
    fn flags_dense_short_doc() {
        // 3 distinct phrases (weight 2 each) = weighted 6, at/above the absolute floor of 6.
        let src = "When it comes to caching, defaults matter a lot. In terms of latency, the numbers speak for themselves. Going forward, keep an eye on hit rates.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP027");
        assert!(diags[0].message.contains("vs threshold"));
        assert_eq!(diags[0].fix.as_deref(), Some("delete the filler"));
    }

    #[test]
    fn flags_dense_short_doc_with_second_pass_phrases() {
        // 3 distinct second-pass phrases (weight 2 each) = weighted 6, at/above the floor of 6.
        let src = "This client works out of the box with no setup. Under the hood, it batches every request. The wrapper gracefully handles a dropped connection cleanly.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP027");
    }

    #[test]
    fn flags_repeated_single_phrase() {
        // Same phrase twice: fires via the repeated-phrase branch regardless of the density
        // floor (weighted total here is only 4, well under the floor of 6).
        let src = "In order to ship this, review it first. Later, in order to ship it again, review it twice.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("repeated"));
    }

    #[test]
    fn clean_single_phrase_and_single_adverb_below_floor() {
        // One phrase (weight 2) + one adverb (weight 1) = weighted 3, below the floor of 6, and
        // no phrase repeats -- must not fire.
        let src = "In order to reduce latency we added a cache. Basically, it works.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_mid_sentence_adverb_not_counted() {
        // "simply" here has no copula before it and isn't sentence-initial: a compound term,
        // not filler, so it must not contribute to the weighted total at all.
        let src = "The simply typed lambda calculus is a foundational model of computation. The simply typed lambda calculus also has a sound type system that rules out a wide class of runtime errors before a program ever executes, which is part of why it remains a popular teaching tool decades after it was first introduced.\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
