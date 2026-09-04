use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::ProseDoc;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP026",
    name: "Dramatic colon reveal",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

/// Only sub-check (a) from the spec is implemented (see module-level report): a short,
/// sentence-initial noun phrase headed by one of a CLOSED set of dramatic "reveal" nouns, a
/// colon, then a lowercase clause on the same line. The closed noun list (not a general
/// no-finite-verb parse, which regex can't do reliably) is what keeps this silent on ordinary
/// technical writing: "The return value: a boolean ..." or "Inline code stays silent: `a-b`."
/// never match because "value"/"silent" aren't in the list, even though both have a colon
/// followed by a lowercase clause. The capitalized determiner + `(?:^|[.!?]\s+)` anchor also
/// means bare labels ("Note:", "Warning:", "Example:", "Tip:", "TODO:") never match: none of
/// them start with one of the listed determiners. A relative clause (`... that makes it work`)
/// is allowed after the noun since it's still one noun phrase, not a second finite main clause.
/// `^[ \t]*` rather than `^`: a blanked HTML tag always leaves spaces before the line's text.
static COLON_REVEAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)(?:^[ \t]*|[.!?]\s+)((?:The|A|An|This|That|One|My|Our|Another)(?:\s+[a-z]+){0,2}\s+(?:part|thing|secret|trick|catch|kicker|twist|magic|truth|surprise|moment|takeaway|punchline|punch line|detail)(?:\s+(?:that|which|who)\s+[a-z]+(?:\s+[a-z]+){0,3})?)\s*:\s+([a-z][^\n]*[.!?])",
    )
    .unwrap()
});

/// Portuguese mirror of `COLON_REVEAL`: same anchor, closed reveal-noun set, colon, then clause.
/// Two shape differences from English: adjectives typically FOLLOW the noun in Portuguese ("A
/// parte mais importante:", "O detalhe curioso:") rather than only preceding it, so up to 3
/// lowercase words are allowed after the noun in addition to up to 2 before it; and the
/// relative-clause pronouns are `que`/`quem`/`onde` instead of `that`/`which`/`who`. No `(?i)`:
/// the determiners must be genuinely capitalized, same as English -- folding case would also
/// match the lowercase articles "um"/"uma"/"a"/"o" that open countless ordinary sentences.
/// `resultado`/`problema`/`diferença` are dropped from the noun set: with a determiner in front,
/// each is an ordinary postmortem/bug-report lead-in ("O problema: a equipe não testou o build."),
/// not a dramatic reveal.
static COLON_REVEAL_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)(?:^[ \t]*|[.!?]\s+)((?:A|O|Uma|Um|Essa|Esse|Esta|Este|Minha|Meu|Nossa|Nosso|Outra|Outro)(?:\s+\p{Ll}+){0,2}\s+(?:parte|coisa|segredo|truque|detalhe|verdade|surpresa|momento|li[çc][ãa]o|virada|sacada|pegadinha|pulo do gato)(?:\s+\p{Ll}+){0,3}(?:\s+(?:que|quem|onde)(?:\s+\p{Ll}+){0,4})?)\s*:\s+([^\W\d_][^\n]*[.!?])",
    )
    .unwrap()
});

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    if ctx.natlangs.contains(&NatLang::En) {
        check_panel(&COLON_REVEAL, doc, rule, ctx, out);
    }
    if ctx.natlangs.contains(&NatLang::PtBr) {
        check_panel(&COLON_REVEAL_PT_BR, doc, rule, ctx, out);
    }
}

/// (a) only: a short noun phrase headed by a closed set of dramatic "reveal" nouns, then a colon,
/// then a lowercase clause on the same line. See each static's doc comment for what keeps it
/// silent on ordinary prose. Word-count cap (<=6, incl. the determiner) enforced here since the
/// regex crate has no lookaround to bound it declaratively; shared by both panels so the two
/// languages stay tuned to the same threshold.
fn check_panel(
    pattern: &Regex,
    doc: &ProseDoc,
    rule: &'static RuleDef,
    ctx: &LintContext,
    out: &mut Vec<Diagnostic>,
) {
    for caps in pattern.captures_iter(&doc.masked) {
        let phrase = caps.get(1).unwrap();
        if phrase.as_str().split_whitespace().count() > 6 {
            continue;
        }
        let clause = caps.get(2).unwrap();
        let byte = phrase.start();
        if doc.in_frontmatter(byte) || doc.in_url(byte) || doc.in_heading(byte) {
            continue;
        }
        // Locate the actual colon between the two captures and reject a doubled `::` (reST
        // directive fields use this; a real reveal colon is always single).
        let between = &doc.masked[phrase.end()..clause.start()];
        let Some(colon_rel) = between.find(':') else {
            continue;
        };
        let colon_byte = phrase.end() + colon_rel;
        if doc.in_url(colon_byte) {
            continue;
        }
        let bytes = doc.masked.as_bytes();
        if bytes.get(colon_byte + 1) == Some(&b':')
            || (colon_byte > 0 && bytes[colon_byte - 1] == b':')
        {
            continue;
        }
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "dramatic colon reveal; state the point plainly instead of a colon-and-reveal",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::prose::ProseDoc;

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        diagnostics_in(ProseDoc::parse(src), src, Lang::Md)
    }

    fn diagnostics_for_html(src: &str) -> Vec<Diagnostic> {
        diagnostics_in(ProseDoc::parse_html(src), src, Lang::Html)
    }

    fn diagnostics_in<'a>(doc: ProseDoc<'a>, src: &'a str, lang: Lang) -> Vec<Diagnostic> {
        diagnostics_in_natlangs(doc, src, lang, crate::lang::ALL_NATLANGS)
    }

    fn diagnostics_in_natlangs<'a>(
        doc: ProseDoc<'a>,
        src: &'a str,
        lang: Lang,
        natlangs: &'static [NatLang],
    ) -> Vec<Diagnostic> {
        let ctx = LintContext {
            display_path: "test.md".to_string(),
            source: src,
            index: None,
            lang,
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
    fn indented_markdown_line_still_opens_a_sentence() {
        assert_eq!(diagnostics_for("   The best part: it learns.\n").len(), 1);
    }

    #[test]
    fn html_paragraph_opening_reveal() {
        let diags = diagnostics_for_html("<p>The best part: it learns.</p>\n");
        assert_eq!(diags.len(), 1);
        assert_eq!((diags[0].line, diags[0].col), (1, 4));
    }

    #[test]
    fn flags_short_noun_phrase_reveal() {
        let diags = diagnostics_for("The best part: it learns.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP026");
    }

    #[test]
    fn flags_relative_clause_reveal() {
        let diags = diagnostics_for("The detail that makes it work: a separate agent grades it.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP026");
    }

    #[test]
    fn clean_list_intro_colon() {
        // Colon at end of line, followed by a list: nothing follows on the same line, so the
        // regex (which requires a lowercase clause on the same line) never matches.
        let src = "The best part:\n\n- It learns fast.\n- It adapts quickly.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_label_prefix() {
        let src = "Note: remember to restart the server after a configuration change.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_clock_time() {
        let src = "Meetings to review the rollout start at 12:30 every Thursday.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_url_colon() {
        let src = "See https://example.com/config/reference for the full list of options.\n"; // ai-slop-ignore
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_frontmatter_key_value() {
        let src = "---\ntitle: Guide\nsummary: How to configure the service safely\n---\n\nBody text follows here without any colon reveal at all.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_ordinary_technical_colon_outside_noun_list() {
        // Has the exact shape (determiner + noun + colon + lowercase clause) but the noun
        // ("value") is not in the closed reveal-noun set, so this ordinary technical sentence
        // must stay silent.
        let src = "The return value: a boolean indicating whether the write succeeded.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_phrase_over_word_cap() {
        // The regex shape matches (determiner + 2 filler words + noun + a short relative
        // clause), but "The single strangest detail that surprised everyone" is 7 words --
        // over the 6-word cap -- so the post-match word-count check must reject it.
        let src =
            "The single strangest detail that surprised everyone: it only happens under load.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn ptbr_flags_short_noun_phrase_reveal() {
        let diags = diagnostics_for("A melhor parte: ele aprende sozinho.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP026");
    }

    #[test]
    fn ptbr_flags_relative_clause_reveal() {
        let diags = diagnostics_for("A parte que ninguém vê: o cache nunca expira.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP026");
    }

    #[test]
    fn ptbr_flags_idiom_noun() {
        let diags = diagnostics_for("O pulo do gato: rodar tudo em paralelo.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP026");
    }

    #[test]
    fn ptbr_clean_ordinary_technical_colon_outside_noun_list() {
        // Same shape, but "valor" (value) is not in the closed reveal-noun set.
        let src = "O valor de retorno: um booleano.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn ptbr_clean_phrase_over_word_cap() {
        let src = "A mais estranha parte que intrigou todo mundo: ninguém sabe explicar.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn ptbr_clean_bare_label_colon() {
        // No determiner before the noun, so the sentence/line anchor never matches.
        let src = "Resultado: 3 a 1.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn ptbr_clean_url_colon() {
        let src = "Veja https://example.com/config/referencia para a lista completa de opções.\n"; // ai-slop-ignore
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn ptbr_clean_problema_lead_in() {
        let src = "O problema: a equipe não testou o build antes do release.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn ptbr_clean_diferenca_lead_in() {
        let src = "A diferença: o novo índice reduz o tempo de consulta.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn ptbr_flags_onde_relative_clause_reveal() {
        let diags = diagnostics_for("O momento onde tudo mudou: o deploy de sexta.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP026");
    }

    #[test]
    fn pt_br_gate_silences_portuguese_panel_when_only_english_selected() {
        let src = "A melhor parte: ele aprende sozinho.\n";
        assert!(
            diagnostics_in_natlangs(ProseDoc::parse(src), src, Lang::Md, &[NatLang::En]).is_empty()
        );
        let src_en = "The best part: it learns.\n";
        assert!(diagnostics_in_natlangs(
            ProseDoc::parse(src_en),
            src_en,
            Lang::Md,
            &[NatLang::PtBr]
        )
        .is_empty());
    }
}
