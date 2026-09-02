use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose_words::{ADJACENT_HEDGE_STACK, HEDGE_PHRASES};
use crate::registry::RuleDef;
use std::collections::HashMap;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP015",
    name: "Hedging & filler-phrase density",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: true,
    path_gated: false,
    check,
};

/// Counts hedging/filler-phrase occurrences in the masked prose stream (headings in scope,
/// frontmatter and URLs skipped). A single "in conclusion" or "it's worth noting" is completely
/// normal in human writing, so this never fires on one hit. Flags once, document-level,
/// anchored at the first occurrence, when the total N meets the density floor
/// (`N >= max(3, ceil(3 * words / 1000))`) OR any single phrase repeats `>= 2` times. Separately,
/// below, an adjacent hedge stack ("might potentially") fires on every match regardless of
/// density -- see the comment at that loop for why it can't share this gate.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let mut total = 0usize;
    let mut first_byte = None;
    let mut per_phrase: HashMap<String, usize> = HashMap::new();
    for m in HEDGE_PHRASES.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) {
            continue;
        }
        total += 1;
        if first_byte.is_none() {
            first_byte = Some(byte);
        }
        *per_phrase.entry(m.as_str().to_lowercase()).or_insert(0) += 1;
    }
    // Integer ceil of 3 * words / 1000, floored at an absolute minimum of 3.
    let threshold = (3 * doc.words).div_ceil(1000).max(3);
    // The two trigger conditions need different wording: "N occurrences vs threshold N" reads as
    // a failed comparison when total < threshold and it's really the phrase-repeated branch that
    // fired (e.g. total=2, threshold=3), so branch the message on which condition actually fired.
    let repeated_phrase = per_phrase.iter().find(|&(_, &n)| n >= 2).map(|(p, _)| p);
    if total >= threshold || repeated_phrase.is_some() {
        let (line, col) = doc.line_col(first_byte.unwrap());
        let message = if total >= threshold {
            format!(
                "high density of hedging/filler phrases ({total} occurrences vs threshold {threshold})"
            )
        } else {
            let phrase = repeated_phrase.unwrap();
            format!("hedging/filler phrase repeated: \"{phrase}\" appears multiple times")
        };
        out.push(Diagnostic::at(rule, ctx, line, col, message));
    }

    // Adjacent hedge stack ("might potentially fail") is checked OUTSIDE the density
    // calculation above and on every match, not just when the document-wide total crosses a
    // threshold: the density path exists precisely to avoid flagging a single ordinary hedge, so
    // a lone doubled-up hedge would get averaged away and never surface if it had to go through
    // that same gate. Stacking two hedge words back to back is a defect on its own, independent
    // of how much (or how little) hedging surrounds it.
    for m in ADJACENT_HEDGE_STACK.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) {
            continue;
        }
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            format!(
                "adjacent hedge stack: \"{}\" hedges twice in a row",
                m.as_str()
            ),
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
            index: None,
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
    fn flags_three_distinct_hedges() {
        let src = "It's important to note that caching reduces database load significantly.\n\nIn conclusion, the cache should be enabled for all production services.\n\nFirst and foremost, monitor hit rates before tuning eviction policies.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP015");
    }

    #[test]
    fn flags_repeated_single_phrase() {
        // total=2 < threshold=3: this only fires via the repeated-phrase branch, so the message
        // must not claim a "vs threshold" comparison that reads as failed (2 < 3).
        let src = "In conclusion, ship it. Later, in conclusion, ship it again.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("repeated"));
        assert!(!diags[0].message.contains("vs threshold"));
    }

    #[test]
    fn clean_two_hedges_below_floor() {
        // N=2, no repeated phrase -> below the absolute floor of 3, must not fire.
        let src =
            "It's worth noting that latency improved. In conclusion, the rollout succeeded.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_single_adjacent_hedge_stack_independent_of_density() {
        // A single hedge, nowhere near the density floor -- must still fire via the separate
        // adjacent-stack path.
        let src = "This change might potentially fail under sustained load.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP015");
        assert!(diags[0].message.contains("adjacent hedge stack"));
    }

    #[test]
    fn clean_non_adjacent_hedge_words() {
        // "could" and "possibly" both appear, but not stacked back to back -- ordinary hedging,
        // not the doubled-up shape this path targets.
        let src = "This could work, but possibly not for every workload.\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
