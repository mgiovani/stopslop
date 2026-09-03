use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::ProseDoc;
use crate::prose_words::{CLAUSE_MARKER, NEGATIVE_PARALLELISM, RULE_OF_THREE, TRAILING_PARTICIPLE};
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP017",
    name: "Rhetorical parallelism / false-depth scaffolding density",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

/// pt-BR twin of `RULE_OF_THREE`, the two-comma "X, Y, e Z" shape. Portuguese only writes the
/// second comma when the third part is a clause with its own subject, so on generated summaries
/// this mostly catches "X, não Y, e Z-clause" coordination: 0 hits on 28 human-written corpus
/// files, 39 generated documents. Bare `e` can collide with the English letter in mixed
/// technical prose ("a, b, e c denote"); `is_single_letter_item` closes that instead of dropping
/// the conjunction. The corpus's real false positives were comma-bounded appositives, see
/// `APPOSITIVE_LEAD_PT_BR`.
///
/// `(?-u:\b)` at both ends instead of a Unicode `\b`: a match that starts or ends one accented
/// letter short of the true item boundary is acceptable (the density gate only needs matches to
/// be counted, not byte-perfect), while a Unicode boundary anywhere forces the slow engine on
/// the whole regex (issue #21).
static RULE_OF_THREE_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u:\b)(?:\w+\s+){0,3}\w+,\s+(?:\w+\s+){0,3}\w+,\s+(?:e|ou)\s+(?:\w+\s+){0,3}\w+(?-u:\b)")
        .unwrap()
});

/// The everyday pt-BR triad, "X, Y e Z" with one comma, the way Portuguese writes every list.
/// Items are one or two lowercase words with a head of four letters or more, closed by
/// punctuation or a function word, and the triad is led by punctuation (a colon, a bullet, a
/// bracket, a sentence end), never by a word: `led_by_punctuation` rejects the mid-sentence
/// list ("comprei arroz, feijão e carne"), which is how humans write one. Measured as the
/// SLOP017 files the panel adds: 0 of 28 human-written corpus files, 1 of 206 human-written
/// documents in a second set, 26 generated documents ("Três eixos: clareza, ritmo e
/// coerência"). Without the gate the shape fired on 16 of 28 human files; with items up to
/// four words, on 4.
static TRIAD_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?-u:\b)\p{Ll}{4,}(?:\s\p{Ll}{2,})?,\s+\p{Ll}{4,}(?:\s\p{Ll}{2,})?\s+(?:e|ou)\s+\p{Ll}{4,}(?:\s\p{Ll}{2,})?(?:[.,;:!?)]|\s+(?:que|para|com|em|no|na|de|do|da)(?-u:\b))",
    )
    .unwrap()
});

/// Portuguese narrative prose freely inserts a comma-bounded prepositional aside ("o harpasto,
/// em Roma, e o epísquiro...", "na Itália, e foi utilizada...") that has the same two-comma-plus-
/// conjunction shape as a tricolon but is a location/attribute note, not a list item. Neither
/// `CLAUSE_MARKER_PT_BR` (no finite verb in "em Roma") nor the capitalized-first-word check (a
/// bare preposition is lowercase) catches it -- confirmed on the pt-BR corpus, where this was the
/// dominant remaining false-positive shape after those two filters. A genuine list item is an
/// adjective or short noun phrase and practically never opens on a bare preposition, so a middle
/// item starting with one is read as an aside. English has no equivalent (its matches are already
/// filtered by `CLAUSE_MARKER`'s finite verbs), so this stays PT-BR-only rather than a third
/// parameter on `is_rhetorical_tricolon`.
static APPOSITIVE_LEAD_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(em|na|no|nas|nos|de|da|do|das|dos|para|com|à|ao|às|aos)\s").unwrap()
});

/// pt-BR twin of `CLAUSE_MARKER`: finite verb forms and subject pronouns, none of which is also
/// an English word (unlike `era`, `a`, `o`, `no`, which are omitted for that reason).
///
/// Only ever run via `is_match` on a short, comma-free item, so whitespace (or string edge)
/// delimiters do the same job a word boundary would without the accented-edge trap: every
/// alternative here starts or ends on `é`/`ã`, where `(?-u:\b)` would silently fail to match.
static CLAUSE_MARKER_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|\s)(?:é|são|foi|foram|será|está|estão|tem|têm|tinha|pode|podem|deve|devem|vai|vão|ele|ela|eles|elas|nós|você|vocês|eu)(?:\s|$)").unwrap()
});

/// pt-BR twin of `NEGATIVE_PARALLELISM`: "não só/apenas/somente X, mas também/ainda Y", "não é
/// (um/uma) X, mas sim (um/uma) Y", and "mais do que X, é Y". The cross-sentence "Não é X. É Y."
/// and "não é sobre X, é sobre Y" without "mas" belong to SLOP023 (contrast/parallel track), not
/// here -- every shape below requires "mas" or "mais do que" to fire.
///
/// The middle shape requires "mas sim", not a bare "mas": on the pt-BR corpus, bare "não é X,
/// mas Y" fired twice in one ordinary Python tutorial ("não é uma função, mas um procedimento")
/// -- an everyday technical clarification, not rhetorical parallelism. "mas sim" is the marked,
/// AI-shaped form of the same contrast ("não é emular a cognição, mas sim ...", also seen in the
/// corpus) and kept the true positive while dropping the false one.
///
/// The third shape ends on a literal "é " (accent plus the trailing space), not `[ée]\b`: an
/// unaccented `e` there is the conjunction ("mais do que um recurso simples, e ajuda..."), and
/// `(?-u:\b)` right after the accented `é` would never match, so the required space does the
/// boundary's job instead. Every other `\b` here is ASCII-scoped (issue #21).
static NEGATIVE_PARALLELISM_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)((?-u:\b)n[ãa]o (?:s[óo]|apenas|somente) [^.?!\n]{0,80}?(?-u:\b)mas(?-u:\b)(\s+(também|ainda))?|(?-u:\b)n[ãa]o [ée] (um|uma )?[^.?!\n]{0,60}?,\s*mas sim(?-u:\b)(\s+(um|uma))?|(?-u:\b)mais do que(?-u:\b)[^.?!\n]{0,60}?,\s*é )",
    )
    .unwrap()
});

/// pt-BR twin of `TRAILING_PARTICIPLE`: a comma-led closed list of evaluative gerunds ending the
/// sentence. Deliberately a closed verb list, not `\w+ndo`, which would also match `quando`
/// ("when"), `segundo` ("second"/"according to"), and `mundo` ("world").
///
/// `permitindo`, `garantindo`, `tornando`, `resultando`, `possibilitando`, `criando`, `gerando`,
/// `levando`, `fazendo`, `oferecendo`, `proporcionando`, `facilitando`, and `assegurando` are left
/// out because they cut on the pt-BR corpus (Wikipedia, literature, a Python tutorial) -- Portuguese
/// leans on trailing gerunds for ordinary cause/result narration in encyclopedic prose far more
/// than English does ("..., permitindo a participação do Poder Judiciário." is a plain
/// legal-history sentence, not rhetorical scaffolding). What survives is framing/evidentiary
/// gerunds -- presenting a point rather than narrating an outcome -- none of which fired on the
/// corpus. `(?-u:\b)` is safe here: every gerund in the list ends on the ASCII letter `o`.
static TRAILING_PARTICIPLE_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i),\s+(refor[çc]ando|destacando|mostrando|demonstrando|evidenciando|contribuindo|promovendo)(?-u:\b)[^.!?\n]{3,80}[.!?]",
    )
    .unwrap()
});

/// Counts matches of the shared RULE_OF_THREE (a), NEGATIVE_PARALLELISM (b), and
/// TRAILING_PARTICIPLE (c) sub-patterns from `prose_words` over the masked prose stream. Each
/// device is legitimate once; the tell is the shape recurring across a document, so this is a
/// density rule, not a per-match one. Scope: skip headings/frontmatter/URLs (code is already
/// masked).
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    let mut a = (0usize, None);
    let mut b = (0usize, None);
    let mut c = (0usize, None);

    if ctx.natlangs.contains(&NatLang::En) {
        a = merge_counts(
            a,
            count_scoped_filtered(doc, &RULE_OF_THREE, |m, r| {
                is_rhetorical_tricolon(m, r, &CLAUSE_MARKER)
            }),
        );
        b = merge_counts(b, count_scoped(doc, &NEGATIVE_PARALLELISM));
        c = merge_counts(c, count_scoped(doc, &TRAILING_PARTICIPLE));
    }
    if ctx.natlangs.contains(&NatLang::PtBr) {
        a = merge_counts(
            a,
            count_scoped_filtered(doc, &RULE_OF_THREE_PT_BR, |m, r| {
                let middle = m[r.clone()].split(',').nth(1).unwrap_or("");
                is_rhetorical_tricolon(m, r, &CLAUSE_MARKER_PT_BR)
                    && !APPOSITIVE_LEAD_PT_BR.is_match(middle)
                    && !is_single_letter_item(middle)
            }),
        );
        a = merge_counts(
            a,
            count_scoped_filtered(doc, &TRIAD_PT_BR, |m, r| {
                led_by_punctuation(m, r.start) && !CLAUSE_MARKER_PT_BR.is_match(&m[r])
            }),
        );
        b = merge_counts(b, count_scoped(doc, &NEGATIVE_PARALLELISM_PT_BR));
        c = merge_counts(c, count_scoped(doc, &TRAILING_PARTICIPLE_PT_BR));
    }
    let (a, a_first) = a;
    let (b, b_first) = b;
    let (c, c_first) = c;

    let t = a + b + c;
    let s = b + c;

    // Replaces `t>=t_floor || b>=3 || c>=3`: t_floor>=4 and t=a+s make those disjuncts provably
    // unreachable unless a>=3 or s>=2 already holds, so removal is proven dead code, not missing
    // coverage.
    let flagged = s >= 2 || a >= 3;
    if !flagged {
        return;
    }

    // Anchor and hint follow whichever signal fired: the old min-byte anchor plus always-print
    // participial hint could point a tricolon run at an enumeration with a nonexistent
    // participle to cut.
    let (first_byte, fix) = if s >= 2 {
        (
            [b_first, c_first].into_iter().flatten().min().unwrap(),
            "cut the participial tail or make it its own sentence",
        )
    } else {
        (
            a_first.unwrap(),
            "three-item lists used as rhetorical scaffolding; drop one or write them out",
        )
    };
    let (line, col) = doc.line_col(first_byte);
    out.push(Diagnostic::at_fix(
        rule,
        ctx,
        line,
        col,
        format!("rhetorical parallelism / false-depth scaffolding density high ({t} occurrences)"),
        fix,
    ));
}

/// Counts matches of `re` over `doc.masked`, skipping any in frontmatter/heading/URL spans.
/// Returns the count and the byte offset of the first counted match (if any).
fn count_scoped(doc: &ProseDoc<'_>, re: &Regex) -> (usize, Option<usize>) {
    count_scoped_filtered(doc, re, |_, _| true)
}

/// Sums two `(count, first_byte)` pairs from independent language passes over the same doc; the
/// combined anchor is whichever pass's first match sits earlier in the file.
fn merge_counts(
    acc: (usize, Option<usize>),
    next: (usize, Option<usize>),
) -> (usize, Option<usize>) {
    (acc.0 + next.0, [acc.1, next.1].into_iter().flatten().min())
}

/// `count_scoped` with an extra per-match predicate, given `(masked, match_start..match_end)`.
fn count_scoped_filtered(
    doc: &ProseDoc<'_>,
    re: &Regex,
    keep: impl Fn(&str, std::ops::Range<usize>) -> bool,
) -> (usize, Option<usize>) {
    let mut n = 0usize;
    let mut first = None;
    for m in re.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_heading(byte) || doc.in_url(byte) {
            continue;
        }
        if !keep(&doc.masked, m.range()) {
            continue;
        }
        n += 1;
        if first.is_none() {
            first = Some(byte);
        }
    }
    (n, first)
}

/// True when a RULE_OF_THREE match is a genuine rhetorical tricolon rather than the tail of an
/// ordinary enumeration.
///
/// `RULE_OF_THREE` matches any comma series of three-or-more items, including the *tail* of a
/// longer one -- which is why its reported column used to land mid-list ("...race, caste,
/// >>>color, religion, or sexual<<<"). Two gates, both cheap:
///
/// 1. **Exactly three items.** A rule of three is three; a five- or seven-item list is just a
///    list. The regex matches a longer list's tail, and a tail is always immediately preceded by
///    the previous item's comma -- so rejecting a match that a comma runs straight into drops the
///    Contributor Covenant's protected-characteristics enumeration, a list of agent product
///    names, and "lint, test, build, security scan, and deploy", while keeping "clear, concise,
///    and correct" and "solo developers, growing startups, or established enterprises". Done by
///    looking backwards, since the `regex` crate has no lookbehind.
///
///    ponytail: known false negative -- a tricolon behind a leading adverbial ("In practice, it
///    is clear, concise, and correct") reads as a tail and is skipped. Acceptable on a density
///    rule that needs three matches to fire; tighten only if it shows up in practice.
///
/// 2. **Items are phrases, not clauses.** An item carrying a finite verb or subject pronoun
///    (`prose_words::CLAUSE_MARKER`) makes this a compound sentence, not an enumeration:
///    "...findings from one rule, it absorbs exactly three, and a fourth is reported" is three
///    clauses joined by commas.
///
/// 3. **Not a proper-noun list.** Two or more of the three items *beginning* with a capitalized
///    word is a list of names ("GitHub, GitLab, and Bitbucket"), not rhetoric. Counting items
///    rather than tokens keeps acronyms mid-item ("the API is clear, concise, and correct") and
///    sentence-initial capitals ("Clear, concise, and correct prose wins") from tripping it.
fn is_rhetorical_tricolon(
    masked: &str,
    range: std::ops::Range<usize>,
    clause_marker: &Regex,
) -> bool {
    // `\w` excludes `-`/`/`, so a match can start mid-token ("weak-verb" matches at "verb");
    // walk back over the rest of the token before checking for the list-tail comma.
    let before = masked[..range.start]
        .trim_end_matches(|c: char| c.is_alphanumeric() || c == '-' || c == '/' || c == '_');
    if before.trim_end().ends_with(',') {
        return false;
    }

    let items: Vec<&str> = masked[range].splitn(3, ',').collect();

    // Only the middle item is checked: it alone is comma-bounded on both sides. The outer two
    // spill into the surrounding sentence (the regex's 4-word run), so judging them would
    // reject ordinary enumerations.
    if items
        .get(1)
        .is_some_and(|item| clause_marker.is_match(item))
    {
        return false;
    }

    let capitalized_items = items
        .iter()
        .filter(|item| {
            item.split_whitespace()
                .find(|w| !matches!(*w, "and" | "or" | "e" | "ou"))
                .and_then(|w| w.chars().next())
                .is_some_and(char::is_uppercase)
        })
        .count();
    capitalized_items < 2
}

/// True when `item` (trimmed) is a single letter -- the shape of a math/notation list using a
/// short variable name next to the Portuguese conjunction "e" ("f, g, e c denote the
/// constants"), not a real Portuguese list item. A genuine Portuguese enumeration item is a
/// word, never one letter, so this closes the "e" collision `RULE_OF_THREE_PT_BR`'s doc comment
/// describes without giving up the conjunction for real tricolons.
fn is_single_letter_item(item: &str) -> bool {
    let mut chars = item.trim().chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c.is_alphabetic())
}

/// True unless the last non-blank character before `start` is a letter, a digit, or a comma,
/// i.e. the match sits mid-sentence or is the tail of a longer list. Whitespace is skipped
/// including newlines, so a list that happens to open a wrapped line ("ativo sobre\ndoença,
/// exame e prevenção") is still mid-sentence; a sentence end, a colon, a bullet, a bracket, or
/// the document start passes. Walks back over the token first because `(?-u:\b)` lets a match
/// start one accented letter into a word ("única" matches at "nica") or after a hyphen
/// ("guarda-chuva").
fn led_by_punctuation(masked: &str, start: usize) -> bool {
    let before = masked[..start]
        .trim_end_matches(|c: char| c.is_alphanumeric() || c == '-' || c == '/' || c == '_');
    !before
        .trim_end()
        .ends_with(|c: char| c.is_alphanumeric() || c == ',')
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

    /// Real strings from a 130-document corpus that the ungated rule-of-three counted. None is
    /// rhetorical; each is an ordinary enumeration or a list of names.
    #[test]
    fn plain_enumerations_do_not_count_as_tricolons() {
        for src in [
            "We do not tolerate discrimination on the basis of nationality, personal appearance, race, caste, color, religion, or sexual orientation.\nThe policy covers age, body size, visible disability, ethnicity, and level of experience.\nIt also covers education, socio-economic status, gender identity, and expression.\n",
            "It works with Claude Code, Codex, Cursor, OpenCode, Gemini CLI, and other agents.\nIt ships configs for GitHub Actions, GitLab CI, CircleCI, Jenkins, and Buildkite.\nIt reads pyproject.toml, package.json, Cargo.toml, go.mod, and Gemfile.\n",
            "The pipeline defines stages for lint, test, build, security scan, and deploy.\nEach stage reports status, duration, artifacts, cache hits, and exit codes.\nFailures capture logs, environment, git metadata, timing, and the command line.\n",
            "We support GitHub, GitLab, and Bitbucket.\nWe test on Linux, macOS, and Windows.\nWe target Python, Ruby, and Rust.\n",
        ] {
            assert!(
                diagnostics_for(src).is_empty(),
                "plain enumeration flagged: {src:?}"
            );
        }
    }

    /// `\w` excludes `-`, so the match starts at "verb" inside "weak-verb" and the byte right
    /// before it is `-`, not the comma that makes this a longer list's tail. Found in this
    /// repo's own README, which enumerates fourteen rule families in one sentence.
    #[test]
    fn hyphenated_token_does_not_hide_a_list_tail() {
        let src = "It covers hedging, overused vocabulary, boldface overuse, smart quotes, heading formatting, colon reveals, filler and adverb density, weak-verb phrasing, dramatic fragmentation, and mechanical uniformity.\nIt also covers alpha, beta, gamma, delta, epsilon, zeta-eta phrasing, theta density, and iota.\nAnd it covers one, two, three, four, five, six-seven forms, eight kinds, and nine.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// Commas joining clauses are not an enumeration. Found in this repo's own README.
    #[test]
    fn compound_sentences_are_not_tricolons() {
        let src = "If a file has three accepted findings from one rule, it absorbs exactly three, and a fourth is reported.\nWhen the budget runs out for that rule, we report the surplus, and the count ratchets down.\nOnce the cache is warm for a run, it reuses the parse, and a rescan is skipped.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// Only the middle item is judged: the outer two spill into the surrounding sentence, so
    /// judging them would reject ordinary enumerations that merely sit next to a verb.
    #[test]
    fn clause_words_outside_the_middle_item_do_not_reject() {
        let src = "The output should be clear, concise, and correct.\nA resolver can miss workspace dependencies, dynamic imports, and unusual build setups.\nIt must handle fences, placeholder credentials, and package imports.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
    }

    #[test]
    fn flags_three_abstract_tricolons() {
        let src = "The output should be clear, concise, and correct.\n\nThe architecture is scalable, maintainable, and robust.\n\nIt suits solo developers, growing startups, or established enterprises.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
    }

    /// The hint must describe the signal that fired: a tricolon run has no participial tail to cut.
    #[test]
    fn tricolon_run_gets_the_tricolon_hint() {
        let src = "The output should be clear, concise, and correct.\n\nThe architecture is scalable, maintainable, and robust.\n\nIt suits solo developers, growing startups, or established enterprises.\n";
        let diags = diagnostics_for(src);
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("three-item lists used as rhetorical scaffolding; drop one or write them out")
        );
    }

    /// An acronym mid-item and a sentence-initial capital are not a proper-noun list.
    #[test]
    fn acronyms_and_sentence_initial_capitals_still_count() {
        let src = "The API is clear, concise, and correct.\n\nClear, concise, and correct prose wins.\n\nThe CLI stays small, sharp, and predictable.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
    }

    #[test]
    fn flags_two_strong_subsignals() {
        // S = b + c >= 2: one negative-parallelism + one trailing participle.
        let src = "The API is not only fast but also simple to use, and it favors clarity, underscoring its focus on developer experience.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("cut the participial tail or make it its own sentence")
        );
    }

    #[test]
    fn flags_two_new_trailing_participle_verbs() {
        // S = c = 2: two of the newly added trailing-participle verbs, no rule-of-three and no
        // negative parallelism anywhere, isolating the widened TRAILING_PARTICIPLE panel.
        let src = "The change reworks the retry path, ensuring every request lands exactly once. It also simplifies the config loader, resulting in a much smaller startup file.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
    }

    #[test]
    fn flags_three_rule_of_three_lists_alone() {
        // a=3, s=0: three independent rule-of-three lists, no negative-parallelism or trailing
        // participle anywhere -- isolates the a>=3 disjunct from s>=2.
        let src = "The plan is clear, simple, and complete.\n\nThe team is fast, careful, and thorough.\n\nThe result is neat, clean, and correct.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
    }

    #[test]
    fn clean_single_rule_of_three_and_factual_gerund() {
        // T=1 (one rule-of-three), S=0 (the gerund isn't in the evaluative participle list).
        let src = "The style guide asks for code that is clear, concise, and correct, running on port 8080 by default.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn skips_headings() {
        let src = "# Fast, Simple, and Reliable\n\nBody text with nothing special going on here at all.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_three_pt_br_abstract_tricolons() {
        let src = "O sistema é rápido, simples, e confiável.\n\nA arquitetura é escalável, sustentável, e robusta.\n\nAtende pequenas equipes, startups em crescimento, ou empresas consolidadas.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
    }

    #[test]
    fn flags_three_one_comma_pt_br_triads() {
        let src = "Três eixos: clareza, ritmo e coerência.\n\n- método, cadência e escuta.\n\nO ciclo tem três frentes (planejar, medir e ajustar) em cada rodada.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("three-item lists used as rhetorical scaffolding; drop one or write them out")
        );
    }

    #[test]
    fn one_comma_pt_br_lists_that_are_not_triads_stay_silent() {
        let src = "Maria, João e Pedro chegaram cedo.\n\nMaria, João e Pedro saíram tarde.\n\nMaria, João e Pedro voltaram.\n\nVendem: ferro, cobre, ouro e prata.\n\nVendem: ferro, cobre, ouro e prata.\n\nVendem: ferro, cobre, ouro e prata.\n\nComprei arroz, feijão e carne no mercado.\n\nComprei arroz, feijão e carne no mercado.\n\nComprei arroz, feijão e carne no mercado.\n\nDiz: você decide, planeja e executa.\n\nDiz: você decide, planeja e executa.\n\nDiz: você decide, planeja e executa.\n\nVendem produtos\nnovos, usados e raros.\n\nVendem produtos\nnovos, usados e raros.\n\nVendem produtos\nnovos, usados e raros.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_pt_br_negative_parallelism_plus_gerund_tail() {
        // S = b + c >= 2: one "não só ... mas também" plus one trailing participle.
        let src = "A API não só é rápida, mas também simples de usar, e isso resulta em uma experiência melhor, destacando seu foco em produtividade.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("cut the participial tail or make it its own sentence")
        );
    }

    #[test]
    fn plain_pt_br_grocery_list_is_silent() {
        // One comma, not two: the everyday "X, Y e Z" list has no Oxford comma before "e".
        let src = "Comprei pão, leite e ovos.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn pt_br_compound_sentence_with_e_in_the_middle_item_is_silent() {
        let src =
            "Quando o índice cresce, ele é reindexado, e a consulta seguinte é mais rápida.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn pt_br_proper_noun_list_is_silent() {
        let src = "Maria, João e Pedro chegaram.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// The bare `e` alternative in `RULE_OF_THREE_PT_BR` risks colliding with the English
    /// letter/word "e" in mixed technical prose; `is_single_letter_item` closes it.
    #[test]
    fn english_math_list_with_bare_e_is_silent() {
        let src = "Let a, b, e c denote the constants used in this proof.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// The third `NEGATIVE_PARALLELISM_PT_BR` shape must require the accented "é", not any "e":
    /// paired with a trailing participle so s >= 2 fires on the "mais do que" match alone.
    #[test]
    fn flags_pt_br_mais_do_que_negative_parallelism_plus_gerund_tail() {
        let src = "Isso é mais do que uma API, é uma plataforma, e a equipe usa isso todo dia, destacando a flexibilidade para times inteiros ao longo do projeto.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
    }

    /// The unaccented conjunction "e" right after the comma must not be mistaken for "é": this
    /// is an everyday clarification, not the "mais do que X, é Y" rhetorical shape.
    #[test]
    fn plain_mais_do_que_followed_by_conjunction_e_is_silent() {
        let src = "Isso é mais do que um recurso simples, e ajuda bastante no dia a dia.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// `APPOSITIVE_LEAD_PT_BR` must keep excluding the comma-bounded prepositional-aside shape
    /// even across three repetitions, since the density gate alone (a >= 3) would otherwise flag
    /// three matches that individually pass `is_rhetorical_tricolon`.
    #[test]
    fn repeated_appositive_lead_lists_stay_silent() {
        let src = "Praticavam o harpasto, em Roma, e o epísquiro, na Grécia.\n\nCultivavam a oliveira, na Itália, e a vinha, na Espanha.\n\nEstudavam a retórica, na Grécia, e o direito, em Roma.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// A bare "mas" without "sim" is an everyday clarification (see the panel's doc comment),
    /// not the marked rhetorical contrast the rule targets.
    #[test]
    fn bare_mas_without_sim_is_silent() {
        let src = "O cache não é um banco, mas uma camada volátil.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn natlang_gate_silences_the_other_languages_panel() {
        let pt_positive = "O sistema é rápido, simples, e confiável.\n\nA arquitetura é escalável, sustentável, e robusta.\n\nAtende pequenas equipes, startups em crescimento, ou empresas consolidadas.\n";
        assert!(diagnostics_for_natlangs(pt_positive, &[NatLang::En]).is_empty());

        let en_positive = "The output should be clear, concise, and correct.\n\nThe architecture is scalable, maintainable, and robust.\n\nIt suits solo developers, growing startups, or established enterprises.\n";
        assert!(diagnostics_for_natlangs(en_positive, &[NatLang::PtBr]).is_empty());
    }
}
