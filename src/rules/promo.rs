use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::PROSE_LANGS;
use crate::registry::RuleDef;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP031",
    name: "Promotional / advertisement language",
    tier: Tier::B,
    langs: PROSE_LANGS,
    default_on: true,
    path_gated: false,
    check,
};

// Marketing-brochure register in technical prose (case-insensitive, word-bounded).
// Two catalog members are deliberately OMITTED because an existing panel already owns the
// exact span:
// - "boasts an?" is dropped: `boast(s|ed|ing)?` is already matched bare by prose_words.rs's
//   VOCAB_TIER1 (SLOP016) -- adding it here would double-flag "boasts" under two rules.
// - "game-chang(er|ing)" is narrowed to just "game-changing": the "game[ -]changer"/"game
//   changer" form is already matched by prose_words.rs's CLICHE_PHRASES (SLOP014); only the
//   "-changing" inflection is untaken.
static PROMO_PHRASES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(nestled (?:in|among|between)|in the heart of|renowned for|world-renowned|breathtaking|must-visit|must-have|stunning|state-of-the-art|best-in-class|industry-leading|award-winning|unparalleled|unrivaled|second to none|a commitment to excellence|natural beauty|a hidden gem|one-stop shop|game-changing|next-generation|turnkey|rich (?:history|heritage|tradition))\b")
        .unwrap()
});

/// Density rule over the masked prose stream (headings in scope, frontmatter and URLs
/// skipped), same shape as `hedging.rs` (SLOP015): fires once, document-level, anchored at the
/// first in-scope occurrence, when the total N meets the density floor
/// (`N >= max(3, ceil(3 * words / 1000))`) OR any single phrase repeats `>= 2` times.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let mut total = 0usize;
    let mut first_byte = None;
    let mut per_phrase: HashMap<String, usize> = HashMap::new();
    for m in PROMO_PHRASES.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) {
            continue;
        }
        total += 1;
        first_byte.get_or_insert(byte);
        *per_phrase.entry(m.as_str().to_lowercase()).or_insert(0) += 1;
    }

    let threshold = (3 * doc.words).div_ceil(1000).max(3);
    let repeated_phrase = per_phrase.iter().find(|&(_, &n)| n >= 2).map(|(p, _)| p);
    if total >= threshold || repeated_phrase.is_some() {
        let (line, col) = doc.line_col(first_byte.unwrap());
        let message = if total >= threshold {
            format!(
                "high density of promotional/advertisement phrases ({total} occurrences vs threshold {threshold})"
            )
        } else {
            let phrase = repeated_phrase.unwrap();
            format!("promotional phrase repeated: \"{phrase}\" appears multiple times")
        };
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            message,
            "state the measurable property instead",
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
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn flags_three_distinct_promo_phrases() {
        let src = "The venue is nestled among rolling hills.\n\nOur platform is state-of-the-art and built for scale.\n\nThe new release is truly a hidden gem for power users.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP031");
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("state the measurable property instead")
        );
    }

    #[test]
    fn flags_repeated_single_phrase() {
        let src = "Our tool is state-of-the-art. Later, the API is also state-of-the-art.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("repeated"));
    }

    #[test]
    fn clean_two_promo_hits_below_floor() {
        let src = "The dashboard is stunning at a glance. It is also turnkey for new teams.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_duplicate_vocabulary_boast_span() {
        // "boasts an?" is dropped from this panel: VOCAB_TIER1 (SLOP016) already matches bare
        // "boast(s|ed|ing)?", so this rule must stay silent even on a promotional-sounding
        // "boasts a" construction.
        let src =
            "The service boasts a fast API. The service boasts a fast API. The service boasts a fast API.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_duplicate_cliche_game_changer_span() {
        // "game changer"/"game-changer" is CLICHE_PHRASES (SLOP014) territory; only the
        // "-changing" inflection belongs to this panel.
        let src = "This release is a real game-changer for the team.\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
