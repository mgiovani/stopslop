use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose_words::{VOCAB_TIER1, VOCAB_TIER2};
use crate::registry::RuleDef;
use std::collections::HashMap;

/// `natlangs` below stays `en`-only, unlike the seven phase-2 prose rules: issue #30 phase 2
/// measured candidate discriminating pt-BR words (`engajamento` 0 vs 140 generated hits/34 docs,
/// `amplificar` 1 vs 47, `sinergia` 0 vs 19, `potencializar` 0 vs 20, `alavancar` 0 vs 15,
/// `insights` 0 vs 40, `intuitivo` 1 vs 13, on a 1.3-million-word human corpus vs. 94 generated
/// documents), but even the full candidate list fires this rule's own density-and-breadth gate
/// on 0 of the 94 generated documents -- most of the words are business-register vocabulary the
/// generated corpus itself barely uses at that density, so a panel here can't clear the bar this
/// rule already holds English to.
pub static RULE: RuleDef = RuleDef {
    code: "SLOP016",
    name: "Overused-vocabulary density",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En],
    default_on: true,
    path_gated: false,
    check,
};

/// Aggregate density over the shared VOCAB_TIER1 (weight 2, distinctive: "delve", "tapestry",
/// "meticulous", ...) and VOCAB_TIER2 (weight 1, common: "robust", "leverage", "comprehensive",
/// ...) marker panels from `prose_words`. No single hit is a signal (every word is ordinary
/// English) — only a whole-document aggregate crossing both a density AND a breadth (distinct
/// lemma) gate fires. Scope: skip headings/frontmatter/URLs (code is already masked).
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    // lemma (lowercased matched text) -> (occurrence count, byte offset of first occurrence)
    let mut lemmas: HashMap<String, (usize, usize)> = HashMap::new();
    let mut w1 = 0usize;
    let mut w2 = 0usize;

    for m in VOCAB_TIER1.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_heading(byte) || doc.in_url(byte) {
            continue;
        }
        w1 += 1;
        let entry = lemmas.entry(m.as_str().to_lowercase()).or_insert((0, byte));
        entry.0 += 1;
    }
    for m in VOCAB_TIER2.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_heading(byte) || doc.in_url(byte) {
            continue;
        }
        w2 += 1;
        let entry = lemmas.entry(m.as_str().to_lowercase()).or_insert((0, byte));
        entry.0 += 1;
    }

    let weighted = 2 * w1 + w2;
    let distinct = lemmas.len();
    let words = doc.words;
    let density = (weighted * 1000).checked_div(words).unwrap_or(0);

    let flagged = (words >= 250 && density >= 12 && distinct >= 6)
        || (words < 250 && weighted >= 6 && distinct >= 4);
    if !flagged {
        return;
    }

    // Anchor at the earliest marker occurrence in the document (either tier).
    let first_byte = lemmas.values().map(|&(_, b)| b).min().unwrap();

    // Top 5 offending lemmas: count desc, ties broken by first-occurrence byte asc.
    let mut ranked: Vec<(&str, usize, usize)> = lemmas
        .iter()
        .map(|(lemma, &(count, byte))| (lemma.as_str(), count, byte))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
    let top = ranked
        .into_iter()
        .take(5)
        .map(|(lemma, _, _)| lemma)
        .collect::<Vec<_>>()
        .join(", ");

    let (line, col) = doc.line_col(first_byte);
    let message = format!("overused-vocabulary density high (top markers: {top})");
    match replacement_fix(lemmas.keys()) {
        Some(fix) => out.push(Diagnostic::at_fix(rule, ctx, line, col, message, fix)),
        None => out.push(Diagnostic::at(rule, ctx, line, col, message)),
    }
}

/// A clean, unambiguous 1:1 replacement for a matched vocab lemma, keyed by prefix/exact match on
/// the lowercased matched text (so any inflected form -- "delves"/"delved"/"delving" -- maps to
/// the same replacement). Not every flagged term has one: this only covers the subset of both
/// panels with a genuinely obvious plain-English substitute; anything else returns `None` and is
/// simply left out of the fix string.
fn lemma_replacement(lemma: &str) -> Option<&'static str> {
    if lemma.starts_with("leverag") || lemma.starts_with("utiliz") || lemma.starts_with("harness") {
        Some("use")
    } else if lemma.starts_with("delv") {
        Some("examine")
    } else if lemma == "myriad" || lemma == "plethora" {
        Some("many")
    } else if lemma.starts_with("facilitat") {
        Some("ease")
    } else if lemma.starts_with("showcas") {
        Some("show")
    } else if lemma.starts_with("elucidat") {
        Some("explain")
    } else if lemma.starts_with("underscor") {
        Some("emphasize")
    } else if lemma.starts_with("garner") {
        Some("gain")
    } else if lemma.starts_with("foster") {
        Some("encourage")
    } else if lemma.starts_with("streamlin") {
        Some("simplify")
    } else if lemma.starts_with("unveil") {
        Some("reveal")
    } else {
        None
    }
}

/// Builds the fix string from whichever matched lemmas (in this document) have a clean 1:1
/// replacement, grouping lemmas that share the same replacement together (e.g. "leverage"/
/// "utilize" both -> "use"). Returns `None` when no matched lemma has a known replacement, per
/// `Diagnostic`'s convention of leaving density findings unfixed when there's no substitutable
/// span. Output is sorted for determinism (HashMap/HashSet iteration order isn't stable).
fn replacement_fix<'a>(matched_lemmas: impl Iterator<Item = &'a String>) -> Option<String> {
    let mut groups: HashMap<&'static str, Vec<&str>> = HashMap::new();
    for lemma in matched_lemmas {
        if let Some(replacement) = lemma_replacement(lemma) {
            groups.entry(replacement).or_default().push(lemma.as_str());
        }
    }
    if groups.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = groups
        .into_iter()
        .map(|(replacement, mut lemmas)| {
            lemmas.sort_unstable();
            lemmas.dedup();
            let joined = lemmas
                .iter()
                .map(|l| format!("`{l}`"))
                .collect::<Vec<_>>()
                .join("/");
            format!("{joined} -> `{replacement}`")
        })
        .collect();
    parts.sort();
    Some(parts.join(", "))
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
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn flags_dense_short_doc() {
        // words < 250: weighted >= 6 && distinct >= 4. 4 distinct tier-1 hits (weight 2 each) =
        // weighted 8, distinct 4 -> fires.
        let src = "The team delved into the archive, unveiling a tapestry of intricate detail that underscored a truly meticulous process.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP016");
        assert!(diags[0].message.contains("top markers"));
        // "delved" -> `examine` and "underscored" -> `emphasize` both have known replacements;
        // "unveiling"/"tapestry"/"intricate"/"meticulous" don't, and are simply left out.
        let fix = diags[0].fix.as_deref().unwrap();
        assert!(fix.contains("`delved` -> `examine`"));
        assert!(fix.contains("`underscored` -> `emphasize`"));
    }

    #[test]
    fn fix_groups_shared_replacements_and_is_none_without_a_known_mapping() {
        assert_eq!(
            replacement_fix(["leverage".to_string(), "utilize".to_string()].iter()).as_deref(),
            Some("`leverage`/`utilize` -> `use`")
        );
        assert_eq!(
            replacement_fix(["myriad".to_string(), "plethora".to_string()].iter()).as_deref(),
            Some("`myriad`/`plethora` -> `many`")
        );
        assert!(replacement_fix(["quintessential".to_string()].iter()).is_none());
    }

    #[test]
    fn clean_below_floor_two_tier2_hits() {
        let src = "A robust error handling layer paired with a comprehensive test suite keeps this service stable in production.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn boundary_words_500_density_exactly_12_fires() {
        // words >= 250 branch: 6 distinct tier-2 markers (weighted=6, distinct=6) over exactly
        // 500 words gives density = 6*1000/500 = 12, right at the threshold.
        let markers = "robust crucial vital pertinent salient adept";
        let filler = "filler ".repeat(500 - markers.split_whitespace().count());
        let src = format!("{markers} {filler}\n");
        let diags = diagnostics_for(&src);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn boundary_words_501_density_truncates_below_threshold_stays_clean() {
        // Same 6 distinct markers, but one extra filler word: integer-truncating density is
        // 6000/501 = 11 (< 12), so this must NOT fire.
        let markers = "robust crucial vital pertinent salient adept";
        let filler = "filler ".repeat(501 - markers.split_whitespace().count());
        let src = format!("{markers} {filler}\n");
        assert!(diagnostics_for(&src).is_empty());
    }

    #[test]
    fn skips_headings() {
        let src = "# Delve into the Meticulous Tapestry of Testaments\n\nBody text with nothing special going on here at all.\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
