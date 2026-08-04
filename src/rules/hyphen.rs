use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP032",
    name: "Hyphenated-compound overuse",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: false,
    path_gated: false,
    check,
};

// Stacked hyphenated modifiers used as filler (case-insensitive, word-bounded). "state-of-the-
// art" and "best-in-class" are deliberately EXCLUDED: they belong to promo.rs (SLOP031)
// instead, which already claims that exact span.
static HYPHEN_COMPOUNDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(third-party|cross-functional|client-facing|data-driven|decision-making|well-known|high-quality|real-time|long-term|short-term|end-to-end|future-proof|battle-tested|out-of-the-box|user-friendly|feature-rich|purpose-built|first-class|top-tier|full-fledged|next-level|world-class|mission-critical|enterprise-grade)\b")
        .unwrap()
});

/// Density rule over the masked prose stream (headings in scope, frontmatter and URLs
/// skipped), same shape as `hedging.rs` (SLOP015): fires once, document-level, anchored at the
/// first in-scope occurrence, when the total N meets the density floor
/// (`N >= max(4, ceil(4 * words / 1000))`) OR any single compound repeats `>= 3` times.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let mut total = 0usize;
    let mut first_byte = None;
    let mut per_phrase: HashMap<String, usize> = HashMap::new();
    for m in HYPHEN_COMPOUNDS.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) {
            continue;
        }
        total += 1;
        first_byte.get_or_insert(byte);
        *per_phrase.entry(m.as_str().to_lowercase()).or_insert(0) += 1;
    }

    let threshold = (4 * doc.words).div_ceil(1000).max(4);
    let repeated_phrase = per_phrase.iter().find(|&(_, &n)| n >= 3).map(|(p, _)| p);
    if total >= threshold || repeated_phrase.is_some() {
        let (line, col) = doc.line_col(first_byte.unwrap());
        let message = if total >= threshold {
            format!(
                "high density of hyphenated-compound modifiers ({total} occurrences vs threshold {threshold})"
            )
        } else {
            let phrase = repeated_phrase.unwrap();
            format!("hyphenated compound repeated: \"{phrase}\" appears three or more times")
        };
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            message,
            "cut the modifiers or replace them with a concrete number",
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
    fn flags_four_distinct_compounds() {
        let src =
            "This is a third-party, cross-functional, client-facing, data-driven integration.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP032");
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("cut the modifiers or replace them with a concrete number")
        );
    }

    #[test]
    fn flags_repeated_single_compound_three_times() {
        let src =
            "The system is real-time. The dashboard is real-time. The pipeline is real-time.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("three or more"));
    }

    #[test]
    fn clean_three_hits_below_floor_and_no_triple_repeat() {
        let src = "The service is well-known for its user-friendly setup and long-term support.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_compound_repeated_only_twice() {
        let src = "The API is real-time. Later, the dashboard is also real-time.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_claim_state_of_the_art_or_best_in_class() {
        // Both belong to promo.rs (SLOP031) instead.
        let src = "Our stack is state-of-the-art. Our support is best-in-class. Ship it today.\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
