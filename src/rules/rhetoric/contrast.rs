use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::first_byte_per_line;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP023",
    name: "Binary contrast / negative listing",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

/// (a) Binary contrast: the "not X, but Y" shapes that `parallelism.rs`'s NEGATIVE_PARALLELISM
/// (SLOP017) does NOT already cover. SLOP017 requires the literal words "not only"/"not just" (or
/// "it's not just/only ... it's") within a SINGLE sentence (its gap class `[^.?!\n]` forbids a
/// sentence break). These four sub-shapes are either cross-sentence ("it's not X. it's Y.", "the
/// problem isn't X. the problem is Y.") or drop the just/only qualifier entirely ("this isn't X,
/// it's Y.", "the question isn't X, it's Y.", "it's not about X, it's about Y."), so none of them
/// can trip SLOP017's patterns.
///
/// The two cross-sentence alternatives join their halves with `[ \t]*\n?[ \t]*`, not `\s+`:
/// `\s+` also matches a blank line, so it could pair the LAST sentence of one paragraph with the
/// FIRST sentence of the next merely because both happened to open with the trigger words --
/// found while corpus-testing the pt-BR twin below, where "Não é X.\n\nÉ Y." (two unrelated
/// paragraphs) matched under the old `\s+` gap. The tightened gap still crosses one hard line
/// wrap (a sentence split mid-paragraph by the source's line width) but never a paragraph break.
static BINARY_CONTRAST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(?-u:\b)it'?s not(?-u:\b)[^.!?\n]{0,60}[.!][ \t]*\r?\n?[ \t]*it'?s(?-u:\b)|(?-u:\b)(?:this|it|the question) isn'?t(?-u:\b)[^.!?\n]{0,60},\s*it'?s(?-u:\b)|(?-u:\b)it'?s not about(?-u:\b)[^.!?\n]{0,60},\s*it'?s about(?-u:\b)|(?-u:\b)the problem isn'?t(?-u:\b)[^.!?\n]{0,60}[.!][ \t]*\r?\n?[ \t]*the problem is(?-u:\b)"#,
    )
    .unwrap()
});

/// (b) Negative listing: two or more consecutive "Not X." / "No Y." fragments, e.g. "Not a
/// framework. Not a library. A compiler." The first fragment is anchored at a sentence or line
/// start (optionally after a list marker, blockquote `>`, or bold/italic punctuation) so a normal
/// mid-sentence negation ("...service, not just the session store.") never anchors a match; the
/// second fragment's anchor comes for free from the `[.!]\s+` gap that closes the first one.
/// Capture group 1 is the whole two-fragment run, used as the diagnostic anchor.
///
/// The `not` and `no` fragments use different gap patterns because they aren't symmetric words:
/// `no` alone is also the Brazilian-Portuguese word for "in the" (`No começo, tudo funcionava.`),
/// so an unqualified `[^.!?\n]{1,40}` let a comma-bearing Portuguese clause slide through as a
/// false "No X." fragment. The English `no`-listing shape is a short noun phrase (`No magic. No
/// config. No lock-in.`), never a full clause with an internal comma, so its gap is capped
/// shorter than `not`'s (25 vs 40 chars) and forbids a comma outright; `not` keeps its original
/// shape since "not" is not a Portuguese word and carries no equivalent ambiguity.
///
/// The join between fragment 1 and fragment 2 is `[ \t]*\n?[ \t]*`, not `\s+`, for the same
/// paragraph-break reason as `BINARY_CONTRAST` above: two openers separated by a blank line are
/// two different paragraphs' first sentences, not one deliberate listing run.
static NEGATIVE_LISTING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?im)(?:^[ \t>*_#-]*|[.!?]["')\]]?[ \t]+)((?:not [^.!?\n]{1,40}[.!]|no [^.!?,\n]{1,25}[.!])[ \t]*\r?\n?[ \t]*(?:not [^.!?\n]{1,40}[.!]|no [^.!?,\n]{1,25}[.!]))"#,
    )
    .unwrap()
});

/// pt-BR twin of (a): the four `BINARY_CONTRAST` sub-shapes translated, plus a fifth idiom
/// ("não se trata de X, mas sim de Y") that has no natural single-word English equivalent.
/// Deliberately requires the accented `é` (not a `[ée]` fold) everywhere except directly after
/// `não`, where the sibling SLOP017 panel (`parallelism::NEGATIVE_PARALLELISM_PT_BR`) already
/// established that `não e` only ever reads as an accent-dropped `não é` -- "not and" is not
/// idiomatic Portuguese. A `[ée]` fold on the *second* copula in each shape would instead catch
/// ordinary `sobre X, e sobre Y` / sentence-initial `E ...` ("and ...") conjunctions. The fifth
/// idiom requires `(?:mas|e) sim de`, not a bare `mas de`: "não se trata de sorte, mas de
/// preparação" reads as ordinary clarification prose without the `sim`.
///
/// `\b` is ASCII-only (`(?-u:\b)`, issue #21): `n[ãa]o` always starts on the ASCII `n`, and
/// `sobre`/`de` always end on an ASCII letter, so those boundaries convert directly. The two
/// spots where a boundary would otherwise land right after the accented `é` (`não é` opening a
/// clause, and the sentence-initial `é` that opens the second half) instead require the literal
/// space that shape always has there, since an ASCII boundary never matches next to `é`.
///
/// The cross-sentence and `[,.!]`-led joins use `[ \t]*\r?\n?[ \t]*`, not `\s+`/`\s*`, so the gap
/// crosses one hard line wrap but never a paragraph break: a pt-BR fixture draft once had "Não é
/// X.\n\nÉ Y." across two unrelated paragraphs match under the old `\s+` gap. The plain `,\s*`
/// joins (shapes A and D) stay as `\s*` -- a comma never opens a new paragraph.
static BINARY_CONTRAST_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)(?-u:\b)n[ãa]o [ée] sobre(?-u:\b)[^.!?\n]{0,60},\s*é sobre(?-u:\b)",
        r"|(?-u:\b)n[ãa]o [ée] [^.!?\n]{0,60}[.!][ \t]*\r?\n?[ \t]*é [^.!?\n]{0,60}[.!]",
        r"|(?-u:\b)(?:o problema|a quest[ãa]o|o ponto|o segredo) n[ãa]o [ée] [^.!?\n]{0,60}[,.!][ \t]*\r?\n?[ \t]*(?:(?:o problema|a quest[ãa]o|o ponto|o segredo)\s+)?é [^.!?\n]{0,60}[.!]",
        r"|(?-u:\b)n[ãa]o se trata de(?-u:\b)[^.!?\n]{0,60},\s*(?:mas|e) sim de(?-u:\b)",
    ))
    .unwrap()
});

/// pt-BR twin of (b): fragments open with `Não`/`Nem`/`Nenhum(a)`/`Sem`/`Zero` instead of
/// `not`/`no`. Same anchor and 1-40-char fragment cap as the English panel; unlike `NEGATIVE_
/// LISTING`'s split `not`/`no` gaps, none of these openers doubles as an unrelated common word
/// (`sem` and `nenhum` are not English or Portuguese function words that also mean something
/// else), so one shared gap suffices. Same `[ \t]*\r?\n?[ \t]*` join as `NEGATIVE_LISTING` (not
/// `\s+`) so it cannot pair a paragraph-final "Não X." with the next paragraph's own "Não Y." --
/// `Não`/`Nem`/`Sem` open Portuguese paragraphs far more often than `not`/`no` open English ones,
/// so this panel hits that failure mode harder than its English twin does.
///
/// A corpus check against pt-BR literary dialogue with real `— Não X. Não Y.` runs found no false
/// positive from short dialogue fragments: BR convention opens spoken lines with an em dash,
/// which is not in the anchor's `[ \t>*_#-]*` class, so a line like "— Não quero. Não posso."
/// never anchors fragment 1 and stays silent. A minimum fragment word count was considered as an
/// extra guard but rejected: the true-positive shape ("Sem mágica. Sem configuração. Sem
/// lock-in.") is itself three one-word fragments, so any length floor tight enough to silence
/// "Não quero. Não posso." would silence the intended catch too. The one residual gap -- a
/// dash-less paragraph that is nothing but two short negations, e.g. a chat transcript -- is
/// accepted at Tier B, the same bar the English panel already accepts for "No. No, thanks."
/// dialogue.
///
/// Each fragment excludes a comma (`[^.!?,\n]`, not `[^.!?\n]`): a real `Sem mágica. Sem
/// configuração.` fragment never carries one, and a corpus pass found the wider class pairing
/// "Sem isso, nada funciona." with the next, unrelated sentence.
static NEGATIVE_LISTING_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?im)(?:^[ \t>*_#-]*|[.!?]["')\]]?[ \t]+)((?:n[ãa]o|nem|nenhum|nenhuma|sem|zero) [^.!?,\n]{1,40}[.!][ \t]*\r?\n?[ \t]*(?:n[ãa]o|nem|nenhum|nenhuma|sem|zero) [^.!?,\n]{1,40}[.!])"#,
    )
    .unwrap()
});

/// Scans the masked prose stream for binary-contrast and negative-listing shapes. Frontmatter and
/// URL spans are skipped; code is already blanked. Emits one diagnostic per matching line,
/// anchored at that line's first match.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    let mut bytes: Vec<usize> = Vec::new();
    if ctx.natlangs.contains(&NatLang::En) {
        bytes.extend(BINARY_CONTRAST.find_iter(&doc.masked).map(|m| m.start()));
        bytes.extend(
            NEGATIVE_LISTING
                .captures_iter(&doc.masked)
                .map(|c| c.get(1).unwrap().start()),
        );
    }
    if ctx.natlangs.contains(&NatLang::PtBr) {
        bytes.extend(
            BINARY_CONTRAST_PT_BR
                .find_iter(&doc.masked)
                .map(|m| m.start()),
        );
        bytes.extend(
            NEGATIVE_LISTING_PT_BR
                .captures_iter(&doc.masked)
                .map(|c| c.get(1).unwrap().start()),
        );
    }
    let bytes = bytes
        .into_iter()
        .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte));
    let by_line = first_byte_per_line(doc, bytes);
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "binary contrast / negative listing; rewrite it directly",
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
            image: None,
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn flags_cross_sentence_its_not_contrast() {
        let diags =
            diagnostics_for("It's not a caching bug. It's a race in the invalidation path.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn flags_isnt_comma_contrast() {
        let diags =
            diagnostics_for("This isn't a performance tweak, it's a full rewrite of the path.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn flags_the_question_isnt_contrast() {
        let diags = diagnostics_for(
            "The question isn't whether to cache, it's how long to keep an entry.\n",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn flags_not_about_contrast() {
        let diags = diagnostics_for("It's not about speed, it's about getting the answer right.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn flags_the_problem_isnt_contrast() {
        let diags = diagnostics_for(
            "The problem isn't the query planner. The problem is a missing index.\n",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn clean_ordinary_two_sentence_paragraph() {
        let diags = diagnostics_for(
            "It's a small utility that wraps the retry logic. It's used by every client.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_negative_listing_run() {
        let diags =
            diagnostics_for("Not a framework. Not a library. A compiler for your test suite.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn clean_single_mid_sentence_negation() {
        // Only one negation, and it lands mid-sentence (after a comma) rather than opening one,
        // so this must not be mistaken for the two-fragment negative-listing shape.
        let diags = diagnostics_for(
            "That patch benefits every service that talks to the queue, not just the worker pool.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn does_not_duplicate_slop017_not_only_but_also() {
        // SLOP017 (parallelism.rs) owns this exact shape; SLOP023 must stay out of its way.
        let diags = diagnostics_for(
            "The new client is not only fast but also simple to integrate with existing code.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_negative_listing_no_fragments() {
        let diags = diagnostics_for("No magic. No config. No lock-in.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn clean_negative_listing_does_not_cross_a_blank_line() {
        // Two different paragraphs that each happen to open with "Not"/"No" are not a
        // deliberate listing run; the fragment gap must not swallow the blank line between them.
        let diags = diagnostics_for("Not a framework.\n\nNo config needed here.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn clean_portuguese_no_is_not_english_negation() {
        // "No" here is the Portuguese contraction of "em" + "o" ("in the"), not the English
        // negation -- the regression this rule shipped to fix (issue #30 phase 1).
        let diags = diagnostics_for("No começo, tudo funcionava. No fim, nada funcionava.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_pt_br_nao_e_sobre_contrast() {
        let diags =
            diagnostics_for("Não é sobre performance, é sobre confiabilidade no longo prazo.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn flags_pt_br_cross_sentence_contrast() {
        let diags = diagnostics_for("Não é um bug de cache. É uma corrida na invalidação.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn flags_pt_br_o_problema_contrast() {
        let diags =
            diagnostics_for("O problema não é a equipe, o problema é a comunicação entre times.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn flags_pt_br_nao_se_trata_de_contrast() {
        let diags = diagnostics_for("Não se trata de sorte, mas sim de preparação.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn flags_pt_br_nao_se_trata_de_e_sim_de_contrast() {
        let diags = diagnostics_for("Não se trata de sorte, e sim de preparação.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn clean_pt_br_nao_se_trata_de_without_sim() {
        // A bare "mas de" (no "sim") is ordinary clarification prose, not the contrast idiom.
        let diags = diagnostics_for(
            "Não se trata de escrever mais código, mas de escrever código melhor.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_pt_br_negative_listing_run() {
        let diags = diagnostics_for("Sem mágica. Sem configuração. Sem lock-in.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn clean_pt_br_negative_listing_fragment_with_comma() {
        // A real negative-listing fragment never carries a comma; a clause like "Sem isso, nada
        // funciona." is ordinary prose, not the "Sem X. Sem Y." shape.
        let diags = diagnostics_for("Sem isso, nada funciona. Não adie a revisão.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn clean_pt_br_negative_listing_does_not_cross_a_blank_line() {
        // Two unrelated paragraphs that each happen to open with "Não"/"Sem" are not a listing
        // run; found by corpus-testing a synthetic dialogue fixture (issue #30 phase 1).
        let diags = diagnostics_for("Não é a mesma coisa.\n\nSem essa desculpa, por favor.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn clean_pt_br_binary_contrast_does_not_cross_a_blank_line() {
        // Same paragraph-break guard as the negative-listing panels, for the cross-sentence
        // "não é X. é Y." shape: "É bom lembrar..." opening the next paragraph is not the
        // second half of the prior paragraph's contrast.
        let diags =
            diagnostics_for("Não é a mesma coisa.\n\nÉ bom lembrar disso sempre que possível.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_pt_br_negative_listing_across_a_single_line_wrap() {
        // A hard line wrap mid-paragraph (no blank line) is still one listing run.
        let diags = diagnostics_for("Sem mágica.\nSem configuração e sem promessas vagas.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn clean_pt_br_does_not_duplicate_slop017_nao_apenas_mas() {
        // SLOP017 (parallelism.rs)'s NEGATIVE_PARALLELISM_PT_BR owns "não apenas X, mas também
        // Y"; SLOP023 must stay out of its way.
        let diags = diagnostics_for(
            "O time não apenas entregou o recurso, mas também documentou cada etapa.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn clean_pt_br_plain_negation() {
        let diags = diagnostics_for("Não sei.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_pt_br_cross_sentence_contrast_over_crlf() {
        let diags = diagnostics_for("Não é um bug de cache.\r\nÉ uma corrida na invalidação.\r\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP023");
    }

    #[test]
    fn pt_br_gate_silences_portuguese_panel_when_only_english_selected() {
        let src = "Não é sobre performance, é sobre confiabilidade no longo prazo.\n";
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
            natlangs: &[NatLang::En],
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        assert!(out.is_empty());

        let src_en = "It's not a caching bug. It's a race in the invalidation path.\n";
        let doc_en = ProseDoc::parse(src_en);
        let ctx_en = LintContext {
            display_path: "test.md".to_string(),
            source: src_en,
            index: None,
            lang: Lang::Md,
            comments: &doc_en.ignore_comments,
            strings: &[],
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: Some(&doc_en),
            image: None,
            natlangs: &[NatLang::PtBr],
        };
        let mut out_en = Vec::new();
        check(&RULE, &ctx_en, &mut out_en);
        assert!(out_en.is_empty());
    }
}
