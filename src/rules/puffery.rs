use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose::first_byte_per_line;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP024",
    name: "Importance puffery / fake-strong verb",
    tier: Tier::A,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: true,
    path_gated: false,
    check,
};

/// (a) Importance puffery: an inflated significance claim that states no fact ("marks a pivotal
/// moment", "solidifies its position", ...). "stands as a testament" and "plays a
/// vital/crucial role" are deliberately OMITTED from this panel even though the catalog groups
/// them under the same shape: `cliche.rs`'s CLICHE_PHRASES already matches "stands/serves as a
/// testament to" and `hedging.rs`'s HEDGE_PHRASES already matches "plays/play an
/// (vital|crucial|...) role in" -- including them here would double-flag the exact same span
/// under two rules.
static IMPORTANCE_PUFFERY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(marks? a pivotal moment|marked a pivotal moment|marks? a turning point|marked a turning point|solidif(?:ies|ied) its position|cements? its place|cemented its place|underscores? its significance|underscored its significance|represents? a significant milestone|represented a significant milestone|highlights? the importance of|highlighted the importance of|reflects? a broader shift|reflected a broader shift|is poised to revolutionize|are poised to revolutionize|sets? a new standard for|set a new standard for)\b")
        .unwrap()
});

/// (b) Fake-strong linking verb: "serves/acts/functions/stands as a/an <noun phrase>", where a
/// plain "is"/"has" would be clearer. Gated tightly on the article ("as a/an") so idiomatic
/// non-nominal uses ("serves as expected", "acts as designed", "functions as intended") never
/// match -- those have no article after "as" at all. Up to two leading modifier words before the
/// head noun covers "serves as a fully centralized hub"-style phrasing.
static FAKE_STRONG_VERB_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:serves?|acts?|functions?|stands?) as an? (?:\w+ ){0,2}\w+\b").unwrap()
});

/// Scans both sub-patterns over the masked prose stream (headings in scope, frontmatter and
/// URLs skipped). A (b) match whose noun phrase is "testament" is skipped: that exact shape
/// ("stands/serves as a testament to ...") is SLOP014's CLICHE_PHRASES territory already. One
/// diagnostic per matching line, anchored at the first (leftmost) match.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    let importance = IMPORTANCE_PUFFERY_RE
        .find_iter(&doc.masked)
        .map(|m| m.start());
    let fake_strong = FAKE_STRONG_VERB_RE
        .find_iter(&doc.masked)
        .filter(|m| !m.as_str().to_lowercase().contains("testament"))
        .map(|m| m.start());

    let bytes = importance
        .chain(fake_strong)
        .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte));
    let by_line = first_byte_per_line(doc, bytes);
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "importance puffery or inflated linking verb; state the fact plainly instead",
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
    fn flags_importance_puffery() {
        let src = "The launch marks a pivotal moment for the product line.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP024");
    }

    #[test]
    fn flags_fake_strong_verb() {
        let src = "The app serves as a centralized hub for sponsor management.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP024");
    }

    #[test]
    fn clean_ordinary_prose() {
        let src = "The release ships next week with the fixes from this sprint.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_idiomatic_serves_as_without_article() {
        // "serves as expected"/"acts as designed" have no article after "as": ordinary
        // technical phrasing, not the inflated-hub shape this rule targets.
        let src = "The retry wrapper serves as expected under sustained load.\n\nThe fallback acts as designed when the primary region is down.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_duplicate_cliche_testament_span() {
        // "stands as a testament to" is SLOP014's CLICHE_PHRASES territory; SLOP024 must stay
        // silent on that exact span so the two Tier-A rules never both fire on one line.
        use crate::rules::cliche;
        let src = "The bridge stands as a testament to careful engineering.\n";
        let puffery_diags = diagnostics_for(src);
        assert!(puffery_diags.is_empty());

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
        let mut cliche_out = Vec::new();
        (cliche::RULE.check)(&cliche::RULE, &ctx, &mut cliche_out);
        assert_eq!(cliche_out.len(), 1);
        assert_eq!(cliche_out[0].code, "SLOP014");
    }

    #[test]
    fn does_not_duplicate_hedging_role_span() {
        // "plays a vital role" is SLOP015's HEDGE_PHRASES territory; SLOP024 must stay silent on
        // that phrase even though the catalog groups it under "importance puffery" too.
        use crate::rules::hedging;
        let src = "Caching plays a vital role in keeping latency low.\n";
        assert!(diagnostics_for(src).is_empty());

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
        let mut hedging_out = Vec::new();
        (hedging::RULE.check)(&hedging::RULE, &ctx, &mut hedging_out);
        // hedging.rs is a density rule (needs 3+ hits or a repeat) so a single occurrence alone
        // won't fire either -- the point of this test is only that puffery.rs never claims it.
        assert!(hedging_out.is_empty());
    }
}
