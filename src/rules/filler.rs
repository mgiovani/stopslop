use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::ProseDoc;
use crate::prose_words::{FILLER_ADVERBS, FILLER_PHRASES};
use crate::registry::RuleDef;
use crate::rules::fragmentation::earliest_qualifying;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP027",
    name: "Empty filler phrase & adverb density",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

/// Brazilian-Portuguese twin of `FILLER_PHRASES`. Lives here rather than in `prose_words.rs`
/// (AGENTS.md step 7: that file holds only the panels the prose density rules SHARE, and this
/// panel has exactly one consumer). Measured against a 318-document, 1.3-million-word human
/// corpus and dropped above 2 hits: `realmente` (63 human hits), `simplesmente` (55), `a fim de`
/// (56), `em termos de` (27), `basicamente` (18), `essencialmente` (25), `claramente` (33),
/// `obviamente` (12), `certamente` (30), `definitivamente` (24), `verdadeiramente` (16),
/// `genuinamente` (6), `honestamente` (3), `literalmente` (9), `inevitavelmente` (3), `sem dúvida`
/// (12), `quando se trata de` (4), `em sua essência` (2), `no mundo do` (5), `no que diz respeito
/// a` (1, dropped as a plain connective), `como vimos` (2, plain connective), `é importante
/// lembrar` (1, plain connective), `com relação a` (3) -- every one has human hits, exactly like
/// the English panel's own dropped members. `na era d[oa]` also dropped on the final corpus
/// measurement: 3 human hits in 3 documents vs 7 generated hits across 4 documents, under the 4x
/// bar.
///
/// Split into two groups instead of one `\b(?:...)\b`, same idiom as `hedging.rs`'s
/// `HEDGE_PHRASES_PT_BR`: `[ée] desnecessário dizer` opens on the accented "é", where a leading
/// `(?-u:\b)` never matches, so it drops the leading boundary and keeps only the trailing one
/// (`desnecessário` ends in the ASCII `o`).
static FILLER_PHRASES_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:(?-u:\b)(?:a realidade [ée] que|a verdade [ée] que|no que tange a|de agora em diante|daqui para frente|neste artigo|neste post|vamos mergulhar|vamos dar uma olhada|como mencionado (?:anteriormente|acima)|nem [ée] preciso dizer|para todos os efeitos|o fato [ée] que|por tr[áa]s dos panos|sem sombra de d[úu]vida|vale lembrar que)(?-u:\b)|[ée] desnecess[áa]rio dizer(?-u:\b))")
        .unwrap()
});

/// Brazilian-Portuguese twin of `FILLER_ADVERBS`, same position gate (line start, after
/// sentence-ending punctuation, or right after a copula). `fundamentalmente` dropped on the final
/// corpus measurement: 7 human hits in 7 documents. The copula alternatives (`é`, `são`, ...)
/// start on an accented letter or lack an ASCII boundary counterpart, so unlike the English
/// panel's leading `(?-u:\b)(?:is|are|...)`, this one has none in front of the copula group -- in
/// real prose the copula is always preceded by whitespace anyway, and the cost of a rare mid-word
/// match on that side is a missed adverb, never a false positive.
static FILLER_ADVERBS_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?im)(?:^[ \t>*_#-]*|[.!?]["')\]]?[ \t]+|(?:é|s[ãa]o|est[áa]|est[ãa]o|foi|era|eram|ser)[ \t]+)(inerentemente|indiscutivelmente|inegavelmente|sinceramente)(?-u:\b)"#).unwrap()
});

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
    let mut per_phrase: HashMap<String, (usize, usize)> = HashMap::new();

    let en = ctx.natlangs.contains(&NatLang::En);
    let pt_br = ctx.natlangs.contains(&NatLang::PtBr);

    // Each language's phrase/adverb pass feeds the same `first_byte` anchor and `per_phrase`
    // tally, so every hit must be merged in by min (not just "set once"): the earliest match
    // overall may come from any of the four loops.
    if en {
        count_filler_phrases(
            doc,
            &FILLER_PHRASES,
            &mut weighted,
            &mut first_byte,
            &mut per_phrase,
        );
        count_filler_adverbs(doc, &FILLER_ADVERBS, &mut weighted, &mut first_byte);
    }
    if pt_br {
        count_filler_phrases(
            doc,
            &FILLER_PHRASES_PT_BR,
            &mut weighted,
            &mut first_byte,
            &mut per_phrase,
        );
        count_filler_adverbs(doc, &FILLER_ADVERBS_PT_BR, &mut weighted, &mut first_byte);
    }

    // Integer ceil of 6 * words / 1000, floored at an absolute minimum of 6 (weighted units;
    // phrases count double, so this floor is equivalent to hedging.rs's "3 hedges" floor
    // expressed in phrase-equivalents).
    let threshold = (6 * doc.words).div_ceil(1000).max(6);
    let repeated_phrase = earliest_qualifying(&per_phrase, 2);
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

/// One language panel's filler-PHRASE contribution (weight 2): every match outside
/// frontmatter/URLs bumps `weighted` by 2, tracks the earliest byte in `first_byte`, and tallies
/// its lowercased text in `per_phrase` for the repeated-phrase branch.
fn count_filler_phrases(
    doc: &ProseDoc,
    re: &Regex,
    weighted: &mut usize,
    first_byte: &mut Option<usize>,
    per_phrase: &mut HashMap<String, (usize, usize)>,
) {
    for m in re.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) {
            continue;
        }
        *weighted += 2;
        *first_byte = Some(first_byte.map_or(byte, |b| b.min(byte)));
        let entry = per_phrase
            .entry(m.as_str().to_lowercase())
            .or_insert((0, byte));
        entry.0 += 1;
        entry.1 = entry.1.min(byte);
    }
}

/// One language panel's filler-ADVERB contribution (weight 1, no repeated-phrase tally: a common
/// adverb repeating twice is unremarkable, unlike a repeated empty clause).
fn count_filler_adverbs(
    doc: &ProseDoc,
    re: &Regex,
    weighted: &mut usize,
    first_byte: &mut Option<usize>,
) {
    for caps in re.captures_iter(&doc.masked) {
        let g = caps.get(1).unwrap();
        let byte = g.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) {
            continue;
        }
        *weighted += 1;
        *first_byte = Some(first_byte.map_or(byte, |b| b.min(byte)));
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
            image: None,
            natlangs,
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

    #[test]
    fn flags_each_pt_br_filler_phrase_alternative() {
        let phrases = [
            "a realidade é que",
            "a verdade é que",
            "no que tange a",
            "de agora em diante",
            "daqui para frente",
            "neste artigo",
            "neste post",
            "vamos mergulhar",
            "vamos dar uma olhada",
            "como mencionado anteriormente",
            "é desnecessário dizer",
            "nem é preciso dizer",
            "para todos os efeitos",
            "o fato é que",
            "por trás dos panos",
            "sem sombra de dúvida",
            "vale lembrar que",
        ];
        for s in phrases {
            assert!(FILLER_PHRASES_PT_BR.is_match(s), "{s}");
        }
    }

    #[test]
    fn flags_each_pt_br_filler_adverb_alternative() {
        let cases = [
            "Inerentemente, o sistema é mais lento em picos de tráfego.",
            "Indiscutivelmente, a mudança ajudou a reduzir o número de erros.",
            "Inegavelmente, o resultado melhorou depois do ajuste.",
            "Sinceramente, não sei se vale a pena manter esse formato.",
        ];
        for s in cases {
            assert!(FILLER_ADVERBS_PT_BR.is_match(s), "{s}");
        }
    }

    #[test]
    fn flags_dense_short_doc_pt_br() {
        // 2 distinct pt-BR phrases (weight 2 each = 4) + 2 distinct pt-BR adverbs (weight 1
        // each = 2) = weighted 6, at/above the absolute floor of 6.
        let src = "Neste artigo, vamos explorar o assunto com calma. Vamos dar uma olhada nos detalhes técnicos. Inerentemente, o sistema é mais lento em picos de tráfego. Sinceramente, não sei se vale a pena manter esse formato.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP027");
        assert!(diags[0].message.contains("vs threshold"));
    }

    /// Every one of these has human hits in the pt-BR corpus (see `FILLER_PHRASES_PT_BR`'s doc
    /// comment) and was dropped from both panels.
    #[test]
    fn clean_pt_br_dropped_filler_shapes() {
        let dropped = [
            "realmente",
            "simplesmente",
            "a fim de",
            "em termos de",
            "basicamente",
            "essencialmente",
            "claramente",
            "obviamente",
            "certamente",
            "definitivamente",
            "verdadeiramente",
            "genuinamente",
            "honestamente",
            "literalmente",
            "inevitavelmente",
            "sem dúvida",
            "quando se trata de",
            "em sua essência",
            "no mundo do",
            "no que diz respeito a",
            "como vimos",
            "é importante lembrar",
            "na era do",
            "fundamentalmente",
        ];
        for phrase in dropped {
            assert!(!FILLER_PHRASES_PT_BR.is_match(phrase), "{phrase}");
            assert!(!FILLER_ADVERBS_PT_BR.is_match(phrase), "{phrase}");
        }
    }

    #[test]
    fn natlang_gate_silences_the_other_languages_panel() {
        let pt_positive =
            "Vale lembrar que o cache expira rápido. Depois, vale lembrar que expira de novo.\n";
        assert!(diagnostics_for_natlangs(pt_positive, &[NatLang::En]).is_empty());

        let en_positive =
            "In order to ship this, review it first. Later, in order to ship it again, review it twice.\n";
        assert!(diagnostics_for_natlangs(en_positive, &[NatLang::PtBr]).is_empty());
    }

    /// Two DIFFERENT filler phrases both repeat (>=2) -- exactly the shape that used to read a
    /// `HashMap`'s iteration order non-deterministically. Padded well past the density floor so
    /// this exercises the repeated-phrase branch: the message must always name "when it comes
    /// to" (textually first), never "in order to" (textually later).
    #[test]
    fn repeated_phrase_message_names_the_earliest_occurrence() {
        let filler =
            "The gardener watered the young trees every quiet morning before sunrise. ".repeat(160);
        let src = format!(
            "When it comes to caching, defaults matter a lot. {filler}In order to reduce latency, review it first. {filler}When it comes to caching again, revisit the defaults. In order to reduce latency again, review it twice.\n"
        );
        let diags = diagnostics_for(&src);
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("repeated"),
            "{}",
            diags[0].message
        );
        assert!(
            diags[0].message.contains("\"when it comes to\""),
            "message was: {}",
            diags[0].message
        );
    }
}
