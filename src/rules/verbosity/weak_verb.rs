use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::ProseDoc;
use crate::registry::RuleDef;
use crate::rules::rhetoric::fragmentation::earliest_qualifying;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP028",
    name: "Weak verb phrase / vague quantifier",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

/// (a) Nominalized weak-verb phrases where a direct verb already exists ("made a decision" ->
/// "decided", "is able to" -> "can", "for the purpose of" -> "for", ...). No digit gate: padding
/// a sentence with a hollow verb phrase is a smell regardless of whether a number shows up
/// nearby.
static WEAK_VERB_PHRASE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:made|make) a decision(?-u:\b)|(?-u:\b)has the ability to(?-u:\b)|(?-u:\b)have the ability to(?-u:\b)|(?-u:\b)has the capability to(?-u:\b)|(?-u:\b)is able to(?-u:\b)|(?-u:\b)are able to(?-u:\b)|(?-u:\b)provides support for(?-u:\b)|(?-u:\b)conducted an analysis(?-u:\b)|(?-u:\b)performed an evaluation(?-u:\b)|(?-u:\b)gives consideration to(?-u:\b)|(?-u:\b)give consideration to(?-u:\b)|(?-u:\b)take into consideration(?-u:\b)|(?-u:\b)make an assessment(?-u:\b)|(?-u:\b)in a timely manner(?-u:\b)|(?-u:\b)on a regular basis(?-u:\b)|(?-u:\b)at this point in time(?-u:\b)|(?-u:\b)due to the fact that(?-u:\b)|(?-u:\b)for the purpose of(?-u:\b)")
        .unwrap()
});

/// (b) Vague quantifiers standing in for a real number: an intensifier immediately adjacent to
/// a change verb/adjective ("significantly improves", "increases dramatically" -- either order),
/// plus a few standalone hand-wave quantities ("a wide range of", "numerous", "countless").
/// Digit-gated per LINE at the call site: a concrete number on the same line means the writer
/// actually measured the thing.
static VAGUE_QUANTIFIER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:significantly|substantially|dramatically|vastly|greatly|considerably|markedly) (?:improves?|improved|increases?|reduces?|faster|better|more|higher|lower)(?-u:\b)|(?-u:\b)(?:improves?|improved|increases?|reduces?|faster|better|more|higher|lower) (?:significantly|substantially|dramatically|vastly|greatly|considerably|markedly)(?-u:\b)|(?-u:\b)a wide range of(?-u:\b)|(?-u:\b)a variety of(?-u:\b)|(?-u:\b)numerous(?-u:\b)|(?-u:\b)countless(?-u:\b)")
        .unwrap()
});

/// Brazilian-Portuguese twin of `WEAK_VERB_PHRASE_RE`. Measured against a 318-document,
/// 1.3-million-word human corpus and dropped above 2 hits: `uma série de` (59 human hits), `inúmeros` (29), `uma variedade de`
/// (20), `oferece suporte a` (21), `com o objetivo de` (18), `é capaz de` (30, 24 docs), `são
/// capazes de` (7), `possui a capacidade de` (1), `fornece suporte a/para` (3), `realizar uma
/// análise` (1), `levar em consideração` (4), `neste momento` (13), `devido ao fato de que` (1),
/// `com a finalidade de` (2), `no sentido de` (11), `dá a possibilidade de` (4), the forward-order
/// vague quantifier (verb-then-adverb), `uma ampla gama de` (6), `uma grande variedade de` (18),
/// `incontáveis` (2), `tomar uma decisão` (2, one of them generated; dropped as a plain
/// connective) -- every one has human hits, the Portuguese counterpart of the English panel's own
/// dropped nominalizations. `tem a capacidade de` starts on the ASCII `t`, so the leading
/// `(?-u:\b)` is fine throughout.
static WEAK_VERB_PHRASE_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:t[êe]m? a capacidade de|efetu(?:ar|ou|am) uma avalia[çc][ãa]o|faz(?:er|em)? uma avalia[çc][ãa]o|em tempo h[áa]bil|de forma regular)(?-u:\b)")
        .unwrap()
});

/// Brazilian-Portuguese twin of `VAGUE_QUANTIFIER_RE`. Only the adjective/verb-then-adverb order
/// ships: Portuguese naturally puts the intensifying adverb after the verb ("reduz
/// significativamente"), so the reverse order the English panel also covers has no pt-BR twin
/// here. Same digit-on-line gate as English, applied at the call site.
static VAGUE_QUANTIFIER_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:melhor|maior|menor|mais r[áa]pid[oa]|reduz|aumenta|melhora)(?:m|s)? (?:significativamente|substancialmente|drasticamente|consideravelmente|amplamente|notavelmente)(?-u:\b)")
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

/// Per-matched-phrase replacement for a `WEAK_VERB_PHRASE_PT_BR` match, same shape as
/// `weak_verb_phrase_fix`. Returns `None` for anything else -- in particular every
/// `VAGUE_QUANTIFIER_PT_BR` match -- so the caller falls back to the shared fix.
fn weak_verb_phrase_fix_pt_br(phrase: &str) -> Option<&'static str> {
    if phrase.contains("capacidade de") {
        Some("pode")
    } else if phrase.contains("avaliação") {
        Some("avaliar")
    } else if phrase == "em tempo hábil" {
        Some("dê o prazo real")
    } else if phrase == "de forma regular" {
        Some("diga a frequência")
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
    let mut first_byte: Option<usize> = None;
    let mut first_phrase: Option<String> = None;
    let mut per_phrase: HashMap<String, (usize, usize)> = HashMap::new();

    let en = ctx.natlangs.contains(&NatLang::En);
    let pt_br = ctx.natlangs.contains(&NatLang::PtBr);

    if en {
        count_phrases(
            doc,
            &WEAK_VERB_PHRASE_RE,
            &mut total,
            &mut first_byte,
            &mut first_phrase,
            &mut per_phrase,
        );
    }
    if pt_br {
        count_phrases(
            doc,
            &WEAK_VERB_PHRASE_PT_BR,
            &mut total,
            &mut first_byte,
            &mut first_phrase,
            &mut per_phrase,
        );
    }
    let mut line_has_digit = line_digit_check(doc);
    if en {
        count_quantifiers(
            doc,
            &VAGUE_QUANTIFIER_RE,
            &mut line_has_digit,
            &mut total,
            &mut first_byte,
            &mut first_phrase,
            &mut per_phrase,
        );
    }
    if pt_br {
        count_quantifiers(
            doc,
            &VAGUE_QUANTIFIER_PT_BR,
            &mut line_has_digit,
            &mut total,
            &mut first_byte,
            &mut first_phrase,
            &mut per_phrase,
        );
    }

    let threshold = (3 * doc.words).div_ceil(1000).max(3);
    let repeated_phrase = earliest_qualifying(&per_phrase, 2);
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
        // `weak_verb_phrase_fix`/`weak_verb_phrase_fix_pt_br` only recognize family (a) phrases,
        // so any family (b) (vague-quantifier) anchor, in either language, falls through to the
        // shared fallback fix.
        let anchor_phrase: &str =
            repeated_phrase.unwrap_or_else(|| first_phrase.as_deref().unwrap());
        let fix = weak_verb_phrase_fix(anchor_phrase)
            .or_else(|| weak_verb_phrase_fix_pt_br(anchor_phrase))
            .unwrap_or("give the measured number");
        out.push(Diagnostic::at_fix(rule, ctx, line, col, message, fix));
    }
}

/// Updates `first_byte`/`first_phrase` to `byte`/`phrase` when `byte` is earlier than (or the
/// first) seen so far. Needed because `check` merges up to four independent `find_iter` passes
/// (English/pt-BR x phrase/quantifier) into one anchor: within a single pass matches arrive in
/// byte order, but across passes they don't, so "first encountered" is wrong and "minimum byte"
/// is required instead.
fn track_first(
    byte: usize,
    phrase: &str,
    first_byte: &mut Option<usize>,
    first_phrase: &mut Option<String>,
) {
    if first_byte.is_none_or(|b| byte < b) {
        *first_byte = Some(byte);
        *first_phrase = Some(phrase.to_string());
    }
}

/// One language panel's family-(a) contribution: every match outside frontmatter/URLs bumps
/// `total`, tracks the earliest byte/phrase via `track_first`, and tallies its lowercased text in
/// `per_phrase` for the repeated-phrase branch.
fn count_phrases(
    doc: &ProseDoc,
    re: &Regex,
    total: &mut usize,
    first_byte: &mut Option<usize>,
    first_phrase: &mut Option<String>,
    per_phrase: &mut HashMap<String, (usize, usize)>,
) {
    for m in re.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) {
            continue;
        }
        *total += 1;
        let phrase = m.as_str().to_lowercase();
        track_first(byte, &phrase, first_byte, first_phrase);
        let entry = per_phrase.entry(phrase).or_insert((0, byte));
        entry.0 += 1;
        entry.1 = entry.1.min(byte);
    }
}

/// One language panel's family-(b) contribution, additionally gated by `line_has_digit` (see
/// its doc comment).
fn count_quantifiers(
    doc: &ProseDoc,
    re: &Regex,
    line_has_digit: &mut impl FnMut(usize) -> bool,
    total: &mut usize,
    first_byte: &mut Option<usize>,
    first_phrase: &mut Option<String>,
    per_phrase: &mut HashMap<String, (usize, usize)>,
) {
    for m in re.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) || line_has_digit(byte) {
            continue;
        }
        *total += 1;
        let phrase = m.as_str().to_lowercase();
        track_first(byte, &phrase, first_byte, first_phrase);
        let entry = per_phrase.entry(phrase).or_insert((0, byte));
        entry.0 += 1;
        entry.1 = entry.1.min(byte);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        diagnostics_for_natlangs(src, crate::lang::ALL_NATLANGS)
    }

    fn diagnostics_for_natlangs(src: &str, natlangs: &[NatLang]) -> Vec<Diagnostic> {
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
            natlangs,
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

    #[test]
    fn flags_each_pt_br_weak_verb_phrase_alternative() {
        let phrases = [
            "tem a capacidade de",
            "efetuar uma avaliação",
            "fazer uma avaliação",
            "em tempo hábil",
            "de forma regular",
        ];
        for phrase in phrases {
            assert!(WEAK_VERB_PHRASE_PT_BR.is_match(phrase), "{phrase}");
        }
    }

    #[test]
    fn flags_each_pt_br_vague_quantifier_alternative() {
        let phrases = [
            "melhor significativamente",
            "maior significativamente",
            "menor significativamente",
            "mais rápido significativamente",
            "reduz significativamente",
            "aumenta significativamente",
            "melhora significativamente",
        ];
        for phrase in phrases {
            assert!(VAGUE_QUANTIFIER_PT_BR.is_match(phrase), "{phrase}");
        }
    }

    #[test]
    fn pt_br_fix_hints_match_the_lookup_table() {
        let cases: &[(&str, &str)] = &[
            ("tem a capacidade de", "pode"),
            ("efetuar uma avaliação", "avaliar"),
            ("fazer uma avaliação", "avaliar"),
            ("em tempo hábil", "dê o prazo real"),
            ("de forma regular", "diga a frequência"),
        ];
        for (phrase, fix) in cases {
            // Two occurrences of the SAME phrase, same convention as the English lookup test.
            let src = format!("O time {phrase} uma vez. Depois, o time {phrase} de novo.\n");
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
    fn flags_three_distinct_pt_br_weak_verb_hits_at_threshold() {
        // 3 distinct pt-BR weak-verb phrases, matching the density floor of 3 exactly.
        let src = "O sistema tem a capacidade de escalar sob carga.\n\nA equipe vai efetuar uma avaliação completa no próximo sprint.\n\nEntregamos o relatório em tempo hábil.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP028");
        assert!(diags[0].message.contains("vs threshold"));
    }

    /// Every one of these has human hits in the pt-BR corpus (see `WEAK_VERB_PHRASE_PT_BR`'s doc
    /// comment) and was dropped from both panels, including the reverse-order vague quantifier
    /// (Portuguese only ships the verb/adjective-then-adverb order).
    #[test]
    fn clean_pt_br_dropped_weak_verb_shapes() {
        let dropped = [
            "uma série de",
            "inúmeros",
            "uma variedade de",
            "oferece suporte a",
            "com o objetivo de",
            "é capaz de",
            "são capazes de",
            "possui a capacidade de",
            "fornece suporte",
            "realizar uma análise",
            "levar em consideração",
            "neste momento",
            "devido ao fato de que",
            "com a finalidade de",
            "no sentido de",
            "dá a possibilidade de",
            "significativamente reduz",
            "uma ampla gama de",
            "uma grande variedade de",
            "incontáveis",
        ];
        for phrase in dropped {
            assert!(!WEAK_VERB_PHRASE_PT_BR.is_match(phrase), "{phrase}");
            assert!(!VAGUE_QUANTIFIER_PT_BR.is_match(phrase), "{phrase}");
        }
    }

    #[test]
    fn natlang_gate_silences_the_other_languages_panel() {
        let pt_positive =
            "Em tempo hábil, o time revisa o código. Depois, em tempo hábil, revisa de novo.\n";
        assert!(diagnostics_for_natlangs(pt_positive, &[NatLang::En]).is_empty());

        let en_positive = "We take into consideration every request. Later, we take into consideration the edge cases too.\n";
        assert!(diagnostics_for_natlangs(en_positive, &[NatLang::PtBr]).is_empty());
    }

    /// Two DIFFERENT weak-verb phrases both repeat (>=2) -- exactly the shape that used to read a
    /// `HashMap`'s iteration order non-deterministically. Padded well past the density floor so
    /// this exercises the repeated-phrase branch: the message must always name "made a decision"
    /// (textually first), never "for the purpose of" (textually later).
    #[test]
    fn repeated_phrase_message_names_the_earliest_occurrence() {
        let filler =
            "The gardener watered the young trees every quiet morning before sunrise. ".repeat(160);
        let src = format!(
            "The team made a decision on Monday. {filler}This change was done for the purpose of reducing latency. {filler}The team made a decision again on Tuesday. This change was done for the purpose of testing it again.\n"
        );
        let diags = diagnostics_for(&src);
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("repeated"),
            "{}",
            diags[0].message
        );
        assert!(
            diags[0].message.contains("\"made a decision\""),
            "message was: {}",
            diags[0].message
        );
    }
}
