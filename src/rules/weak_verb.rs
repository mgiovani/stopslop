use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::registry::RuleDef;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP028",
    name: "Weak verb phrase / vague quantifier",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: false,
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

/// A digit anywhere on `byte`'s own line in `masked`. Used only to gate (b): a concrete number
/// on the line means the vague quantifier isn't standing in for one after all.
fn line_has_digit(masked: &str, byte: usize) -> bool {
    let start = masked[..byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = masked[byte..]
        .find('\n')
        .map(|i| byte + i)
        .unwrap_or(masked.len());
    masked[start..end].bytes().any(|b| b.is_ascii_digit())
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
    let mut per_phrase: HashMap<String, usize> = HashMap::new();

    for m in WEAK_VERB_PHRASE_RE.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) {
            continue;
        }
        total += 1;
        first_byte.get_or_insert(byte);
        *per_phrase.entry(m.as_str().to_lowercase()).or_insert(0) += 1;
    }
    for m in VAGUE_QUANTIFIER_RE.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) || line_has_digit(&doc.masked, byte) {
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
                "high density of weak-verb/vague-quantifier phrases ({total} occurrences vs threshold {threshold})"
            )
        } else {
            let phrase = repeated_phrase.unwrap();
            format!(
                "weak-verb/vague-quantifier phrase repeated: \"{phrase}\" appears multiple times"
            )
        };
        out.push(Diagnostic::at(rule, ctx, line, col, message));
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
    }

    #[test]
    fn flags_three_distinct_vague_quantifiers() {
        let src = "The update significantly improves throughput.\n\nA wide range of teams adopted the change.\n\nNumerous engineers reviewed the design.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP028");
    }

    #[test]
    fn flags_repeated_single_phrase() {
        let src = "We take into consideration every request. Later, we take into consideration the edge cases too.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("repeated"));
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
