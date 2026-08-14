use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose::ProseDoc;
use crate::registry::RuleDef;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP028",
    name: "Weak verb phrase / vague quantifier",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: true,
    path_gated: false,
    check,
};

/// (a) Nominalized weak-verb phrases where a direct verb already exists ("made a decision" ->
/// "decided", "is able to" -> "can", "for the purpose of" -> "for", ...). No digit gate: padding
/// a sentence with a hollow verb phrase is a smell regardless of whether a number shows up
/// nearby.
static WEAK_VERB_PHRASE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:made|make) a decision\b|\bhas the ability to\b|\bhave the ability to\b|\bhas the capability to\b|\bis able to\b|\bare able to\b|\bprovides support for\b|\bconducted an analysis\b|\bperformed an evaluation\b|\bgives consideration to\b|\bgive consideration to\b|\btake into consideration\b|\bmake an assessment\b|\bin a timely manner\b|\bon a regular basis\b|\bat this point in time\b|\bdue to the fact that\b|\bfor the purpose of\b")
        .unwrap()
});

/// (b) Vague quantifiers standing in for a real number: an intensifier immediately adjacent to
/// a change verb/adjective ("significantly improves", "increases dramatically" -- either order),
/// plus a few standalone hand-wave quantities ("a wide range of", "numerous", "countless").
/// Digit-gated per LINE at the call site: a concrete number on the same line means the writer
/// actually measured the thing.
static VAGUE_QUANTIFIER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:significantly|substantially|dramatically|vastly|greatly|considerably|markedly) (?:improves?|improved|increases?|reduces?|faster|better|more|higher|lower)\b|\b(?:improves?|improved|increases?|reduces?|faster|better|more|higher|lower) (?:significantly|substantially|dramatically|vastly|greatly|considerably|markedly)\b|\ba wide range of\b|\ba variety of\b|\bnumerous\b|\bcountless\b")
        .unwrap()
});

/// Per-matched-phrase replacement for a (a) WEAK_VERB_PHRASE_RE match, keyed by a substring/exact
/// check on the lowercased matched text (so "made a decision" and "make a decision" both map to
/// the same fix, etc.). Returns `None` for anything NOT in family (a) -- in particular, every (b)
/// VAGUE_QUANTIFIER_RE match -- so the caller can fall back to a single shared fix for those.
fn weak_verb_phrase_fix(phrase: &str) -> Option<&'static str> {
    if phrase.contains("decision") {
        Some("decided")
    } else if phrase.contains("ability")
        || phrase.contains("capability")
        || phrase == "is able to"
        || phrase == "are able to"
    {
        Some("can")
    } else if phrase == "due to the fact that" {
        Some("because")
    } else if phrase == "for the purpose of" {
        Some("for")
    } else if phrase == "at this point in time" {
        Some("now")
    } else if phrase == "in a timely manner" {
        Some("give the actual deadline")
    } else if phrase == "on a regular basis" {
        Some("say how often")
    } else if phrase.contains("consideration") {
        Some("consider")
    } else if phrase == "conducted an analysis" {
        Some("analyzed")
    } else if phrase == "performed an evaluation" {
        Some("evaluated")
    } else if phrase == "make an assessment" {
        Some("assess")
    } else if phrase == "provides support for" {
        Some("supports")
    } else {
        None
    }
}

/// A digit anywhere on `byte`'s own line in `masked`. Used only to gate (b): a concrete number
/// on the line means the vague quantifier isn't standing in for one after all.
///
/// Memoized on the last line inspected, because matches arrive in byte order and the scan costs
/// the length of the line: 100k quantifiers on one 900 KB line rescanned that line 100k times
/// (22s) before this.
fn line_digit_check<'d>(doc: &'d ProseDoc<'_>) -> impl FnMut(usize) -> bool + 'd {
    let mut memo: Option<((usize, usize), bool)> = None;
    move |byte| {
        let span = doc.line_span(byte);
        match memo {
            Some((s, has_digit)) if s == span => has_digit,
            _ => {
                let has_digit = doc.masked[span.0..span.1]
                    .bytes()
                    .any(|b| b.is_ascii_digit());
                memo = Some((span, has_digit));
                has_digit
            }
        }
    }
}

/// Density rule over the masked prose stream (headings in scope, frontmatter and URLs skipped),
/// same shape as `hedging.rs` (SLOP015): a single nominalized phrase or vague quantifier is
/// unremarkable on its own, so this fires document-level when the total N meets the density
/// floor (`N >= max(3, ceil(3 * words / 1000))`) OR any single phrase repeats `>= 2` times.
/// Chosen over a per-line rule because these phrases (especially the (a) nominalizations) are
/// common enough individually in ordinary, if wordy, prose that a per-occurrence Tier B rule
/// would be too noisy to be worth opting into; the density gate is what makes it a genuine
/// pattern rather than a single word choice.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let mut total = 0usize;
    let mut first_byte = None;
    let mut first_phrase: Option<String> = None;
    let mut per_phrase: HashMap<String, usize> = HashMap::new();

    for m in WEAK_VERB_PHRASE_RE.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) {
            continue;
        }
        total += 1;
        if first_byte.is_none() {
            first_byte = Some(byte);
            first_phrase = Some(m.as_str().to_lowercase());
        }
        *per_phrase.entry(m.as_str().to_lowercase()).or_insert(0) += 1;
    }
    let mut line_has_digit = line_digit_check(doc);
    for m in VAGUE_QUANTIFIER_RE.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) || line_has_digit(byte) {
            continue;
        }
        total += 1;
        if first_byte.is_none() {
            first_byte = Some(byte);
            first_phrase = Some(m.as_str().to_lowercase());
        }
        *per_phrase.entry(m.as_str().to_lowercase()).or_insert(0) += 1;
    }

    let threshold = (3 * doc.words).div_ceil(1000).max(3);
    let repeated_phrase = per_phrase.iter().find(|&(_, &n)| n >= 2).map(|(p, _)| p);
    if total >= threshold || repeated_phrase.is_some() {
        let (line, col) = doc.line_col(first_byte.unwrap());
        let message = if total >= threshold {
            format!(
                "high density of weak-verb/vague-quantifier phrases ({total} occurrences vs threshold {threshold})"
            )
        } else {
            let phrase = repeated_phrase.unwrap();
            format!(
                "weak-verb/vague-quantifier phrase repeated: \"{phrase}\" appears multiple times"
            )
        };
        // The anchor phrase (whichever one drove this diagnostic) determines the fix: a repeated
        // phrase wins if that's what fired, otherwise the earliest matched phrase in the
        // document. `weak_verb_phrase_fix` only recognizes family (a) phrases, so any family (b)
        // (vague-quantifier) anchor falls through to the shared fallback fix.
        let anchor_phrase: &str = repeated_phrase
            .map(String::as_str)
            .unwrap_or_else(|| first_phrase.as_deref().unwrap());
        let fix = weak_verb_phrase_fix(anchor_phrase).unwrap_or("give the measured number");
        out.push(Diagnostic::at_fix(rule, ctx, line, col, message, fix));
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
    fn flags_three_distinct_weak_verb_phrases() {
        let src = "The team made a decision on Monday.\n\nWe have the ability to roll back at any time.\n\nThis change was done for the purpose of reducing latency.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP028");
        // Anchored at the first match ("made a decision"), so the fix follows that phrase.
        assert_eq!(diags[0].fix.as_deref(), Some("decided"));
    }

    #[test]
    fn flags_three_distinct_vague_quantifiers() {
        let src = "The update significantly improves throughput.\n\nA wide range of teams adopted the change.\n\nNumerous engineers reviewed the design.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP028");
        assert_eq!(diags[0].fix.as_deref(), Some("give the measured number"));
    }

    #[test]
    fn flags_repeated_single_phrase() {
        let src = "We take into consideration every request. Later, we take into consideration the edge cases too.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("repeated"));
        assert_eq!(diags[0].fix.as_deref(), Some("consider"));
    }

    #[test]
    fn per_phrase_fix_hints_match_the_lookup_table() {
        let cases: &[(&str, &str)] = &[
            ("has the ability to", "can"),
            ("has the capability to", "can"),
            ("is able to", "can"),
            ("are able to", "can"),
            ("due to the fact that", "because"),
            ("for the purpose of", "for"),
            ("at this point in time", "now"),
            ("in a timely manner", "give the actual deadline"),
            ("on a regular basis", "say how often"),
            ("gives consideration to", "consider"),
            ("conducted an analysis", "analyzed"),
            ("performed an evaluation", "evaluated"),
            ("make an assessment", "assess"),
            ("provides support for", "supports"),
        ];
        for (phrase, fix) in cases {
            // Two occurrences of the SAME phrase so the repeated-phrase branch fires reliably
            // regardless of document word count (avoids depending on the density floor).
            let src = format!("The team {phrase} once. Later, the team {phrase} again.\n");
            let diags = diagnostics_for(&src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {phrase}");
            assert_eq!(
                diags[0].fix.as_deref(),
                Some(*fix),
                "wrong fix for phrase: {phrase}"
            );
        }
    }

    #[test]
    fn clean_two_weak_verb_hits_below_floor() {
        let src =
            "We have the ability to roll back at any time. Ship the fix in a timely manner.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_vague_quantifier_suppressed_by_digit_on_line() {
        // A concrete number on the same line means the writer did the work; the quantifier is
        // no longer standing in for a real measurement.
        let src = "The change significantly improves throughput, from 400 to 900 requests per second.\n\nA wide range of 12 teams adopted the change.\n\nNumerous 3 engineers reviewed the design.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_quantifier_not_adjacent_to_change_verb() {
        // The intensifier and the verb must be immediately adjacent; a quantifier separated from
        // its verb by other words is ordinary prose, not the tight "significantly improves"
        // shape this rule targets.
        let src = "Caching reduces database load significantly, according to the benchmark.\n\nCaching reduces database load significantly, and it also cuts memory pressure.\n\nCaching reduces database load significantly, per the dashboard.\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
