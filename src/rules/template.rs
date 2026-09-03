use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::first_byte_per_line;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP013",
    name: "Unfilled template placeholder text",
    tier: Tier::A,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

/// (1) Bracketed instructional placeholders: just the opening `[` + keyword. The matching close
/// `]` is found by hand (see `find_bracket_close`) rather than with `[^\]]*\]`, which stops at
/// the FIRST `]` and is defeated by nested brackets in real link text (`[link to [our
/// repo]](url)`). The post-match rejection below (no lookaround in the `regex` crate) is what
/// keeps `[click here](url)` / `[link to docs](url)` from firing even though "link to" is itself
/// a listed keyword.
static RE_BRACKET_OPEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\[(insert|describe|add|replace|your name|company name|entertainer'?s name|link to|paste|tbd|todo|placeholder|xxx)(?-u:\b)")
        .unwrap()
});

/// Byte offset of the `]` that closes the `[` this match opened, tracking real nesting depth
/// from `start` (the position right after the opening `[`+keyword). Returns `None` if the
/// brackets never balance before EOF.
fn find_bracket_close(s: &str, start: usize) -> Option<usize> {
    let mut depth = 1i32;
    for (i, ch) in s[start..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + i);
                }
            }
            _ => {}
        }
    }
    None
}

/// (2) ALL-CAPS fill-in tokens. Case-sensitive: requires a fill-in verb/possessive prefix or a
/// `_HERE` suffix, so ordinary constants like `DATABASE_URL`/`MAX_RETRIES` never match.
static RE_ALLCAPS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u:\b)(INSERT|PASTE|ADD|REPLACE|YOUR|SOURCE|EXAMPLE)_[A-Z0-9_]+(?-u:\b)|(?-u:\b)[A-Z0-9]+_HERE(?-u:\b)")
        .unwrap()
});

/// (3) Placeholder dates: literal XX stubs, or an `access-date=`/`date=` key still holding a
/// TBD/TODO/XXXX/stub value.
static RE_DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)20\d{2}-[Xx]{2}-[Xx]{2}(?-u:\b)|(?-u:\b)20\d{2}-\d{2}-[Xx]{2}(?-u:\b)|(?-u:\b)(access-?date|date)\s*=\s*(tbd|todo|xxxx|20\d{2}-[Xx]{2}-[Xx]{2})(?-u:\b)")
        .unwrap()
});

/// (4) HTML-comment fill instructions, e.g. `<!-- Add citation -->`, and the `your … here` slot
/// generated pages leave in the body (`<!-- Your content here -->`, `<!-- put your logo here -->`).
/// The slot form is the whole comment: an optional verb, `your`, one to three words, `here`.
/// `<!-- if your build fails here, see docs -->` is prose and must not match.
static RE_HTML_COMMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<!--\s*(?:(?:add|insert|todo|fill in|replace|describe)(?-u:\b)[^>]*|(?:\w+\s+)?your\s+(?:\w+\s+){1,3}here[.!]?\s*)-->").unwrap()
});

/// Brazilian-Portuguese twin of `RE_BRACKET_OPEN`. Same close-bracket rejection applies below --
/// a Markdown link `[link para o site](url)` must stay quiet exactly like `[link to ...](url)`
/// does in English.
static RE_BRACKET_OPEN_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\[(?:inserir|insira|descrever|descreva|adicionar|adicione|substituir|substitua|colar|cole|seu nome|sua empresa|nome da empresa|nome d[oa] (?:artista|cliente|autor|autora|produto|projeto)|link para|a definir|a preencher|preencher|preencha|espa[çc]o reservado)(?-u:\b)")
        .unwrap()
});

/// Brazilian-Portuguese twin of `RE_ALLCAPS`.
static RE_ALLCAPS_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u:\b)(?:INSERIR|INSIRA|COLAR|COLE|ADICIONAR|ADICIONE|SUBSTITUIR|SUBSTITUA|SEU|SUA|FONTE|EXEMPLO)_[A-Z0-9_]+(?-u:\b)|(?-u:\b)[A-Z0-9]+_AQUI(?-u:\b)")
        .unwrap()
});

/// Brazilian-Portuguese twin of `RE_DATE`. The `20\d{2}-XX-XX` stub is already language-neutral
/// in `RE_DATE`, so this only adds the `a definir` value after a `data`/`data de acesso` key.
/// "data" is also an ordinary English word (a plural of "datum"), so nothing looser than the
/// literal `a definir` is accepted after it -- a bare `data = ...` would false-positive on
/// English prose discussing data.
static RE_DATE_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)data(?:[ _-]?(?:de[ _-]?)?acesso)?\s*[=:]\s*a definir(?-u:\b)")
        .unwrap()
});

/// Brazilian-Portuguese twin of `RE_HTML_COMMENT`.
static RE_HTML_COMMENT_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<!--\s*(?:(?:adicionar|adicione|inserir|insira|preencher|preencha|substituir|substitua|descrever|descreva)(?-u:\b)[^>]*|(?:\w+\s+)?(?:seu|sua)\s+(?:\w+\s+){1,3}aqui[.!]?\s*)-->").unwrap()
});

/// One language panel's bracket-placeholder contribution: same close-bracket rejection as the
/// call site in `check` below, factored out so the English and Portuguese panels share it.
fn bracket_open_bytes(masked: &str, re: &Regex, bytes: &mut Vec<usize>) {
    for m in re.find_iter(masked) {
        let Some(close) = find_bracket_close(masked, m.end()) else {
            continue;
        };
        // No lookaround in the `regex` crate: inspect the char right after the real closing `]`
        // by hand and reject it if this is really a markdown inline link `[text](url)`,
        // reference link `[text][ref]`, or reference/footnote definition `[text]:` rather than a
        // placeholder.
        let after = masked[close + 1..].chars().next();
        if matches!(after, Some('(') | Some('[') | Some(':')) {
            continue;
        }
        bytes.push(m.start());
    }
}

/// Scope: headings in scope, frontmatter IN SCOPE (placeholder dates commonly appear as
/// `date: 2025-XX-XX` in YAML frontmatter), URLs in scope. Only code (already blanked in
/// `doc.masked`) is excluded. One diagnostic per matching line, first byte wins. Each language's
/// panel only runs when `ctx.natlangs` enables it; the default enables both (union).
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let masked = doc.masked.as_str();
    let mut bytes: Vec<usize> = Vec::new();
    let en = ctx.natlangs.contains(&NatLang::En);
    let pt = ctx.natlangs.contains(&NatLang::PtBr);

    if en {
        bracket_open_bytes(masked, &RE_BRACKET_OPEN, &mut bytes);
        for re in [&*RE_ALLCAPS, &*RE_DATE, &*RE_HTML_COMMENT] {
            bytes.extend(re.find_iter(masked).map(|m| m.start()));
        }
    }
    if pt {
        bracket_open_bytes(masked, &RE_BRACKET_OPEN_PT_BR, &mut bytes);
        for re in [&*RE_ALLCAPS_PT_BR, &*RE_DATE_PT_BR, &*RE_HTML_COMMENT_PT_BR] {
            bytes.extend(re.find_iter(masked).map(|m| m.start()));
        }
    }

    let by_line = first_byte_per_line(doc, bytes.into_iter());
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "unfilled template placeholder",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::prose::ProseDoc;

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        diagnostics_for_natlangs(src, crate::lang::ALL_NATLANGS)
    }

    fn diagnostics_for_natlangs(src: &str, natlangs: &'static [NatLang]) -> Vec<Diagnostic> {
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
    fn flags_bracketed_placeholder() {
        let diags = diagnostics_for("Written by [Your Name], a contributor to the blog.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP013");
    }

    #[test]
    fn flags_allcaps_insert_token() {
        let diags = diagnostics_for("Source link: INSERT_SOURCE_URL_30\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_your_content_here_comment() {
        let diags = diagnostics_for(
            "Intro.\n\n<!-- Your content here -->\n\n<!-- put your logo here -->\n",
        );
        assert_eq!(diags.len(), 2);
        assert!(diagnostics_for("<!-- here is where your ideas go -->\n").is_empty());
        assert!(diagnostics_for("<!-- if your build fails here, see docs -->\n").is_empty());
    }

    #[test]
    fn flags_allcaps_here_token() {
        let diags = diagnostics_for("Now playing: PASTE_SPOTIFY_TRACK_URL_HERE\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_placeholder_date_in_frontmatter() {
        let diags = diagnostics_for("---\ntitle: Draft\ndate: 2025-XX-XX\n---\nBody text here.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_html_comment_instruction() {
        let diags =
            diagnostics_for("Some intro text.\n<!-- Add citation -->\nMore text follows.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn ignores_markdown_link_even_with_keyword_text() {
        let diags = diagnostics_for(
            "Background is in the [link to our changelog](https://example.com/changelog) below.\n", // ai-slop-ignore
        );
        assert!(
            diags.is_empty(),
            "bracket-then-paren must be rejected as a real link"
        );
    }

    #[test]
    fn ignores_nested_bracket_markdown_link() {
        // The inner `[our repo]` must not fool the rejection check into inspecting the wrong
        // (inner) closing bracket's successor -- the real outer close is followed by `(`.
        let diags = diagnostics_for(
            "See the [link to [our repo]](https://github.com/example/repo) for details.\n",
        );
        assert!(
            diags.is_empty(),
            "nested-bracket link text must still be rejected as a real link"
        );
    }

    #[test]
    fn ignores_ordinary_link_and_ref_def() {
        let diags = diagnostics_for(
            "See [click here](https://example.com/docs) or [note]: https://example.com/ref\n", // ai-slop-ignore
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn ignores_ordinary_screaming_case_constants() {
        let diags =
            diagnostics_for("Runtime config reads DATABASE_URL and MAX_RETRIES at startup.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_pt_br_bracketed_placeholder() {
        let diags = diagnostics_for("Escrito por [seu nome], colaborador do blog.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP013");
    }

    #[test]
    fn ignores_pt_br_markdown_link_even_with_keyword_text() {
        let diags = diagnostics_for(
            "Veja o [link para o site](https://example.org) para mais detalhes.\n", // ai-slop-ignore
        );
        assert!(
            diags.is_empty(),
            "bracket-then-paren must be rejected as a real link"
        );
    }

    #[test]
    fn flags_pt_br_allcaps_insert_token() {
        let diags = diagnostics_for("Link da fonte: INSERIR_URL_FONTE_30\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_pt_br_allcaps_aqui_token() {
        let diags = diagnostics_for("Tocando agora: COLAR_LINK_SPOTIFY_AQUI\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_pt_br_placeholder_date() {
        let diags = diagnostics_for("Publicado em: data de acesso = a definir\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn ignores_pt_br_ordinary_data_sentence() {
        // "data" alone is not a fill-in key -- only the literal "a definir" value is a stub.
        assert!(diagnostics_for("A tabela de dados foi publicada em 2024-03-01.\n").is_empty());
    }

    #[test]
    fn flags_pt_br_html_comment_instruction() {
        let diags =
            diagnostics_for("Texto introdutório.\n<!-- Adicionar citação -->\nMais texto segue.\n");
        assert_eq!(diags.len(), 1);
        let hint = diagnostics_for(
            "Introdução.\n\n<!-- Seu conteúdo aqui -->\n\n<!-- coloque sua logo aqui -->\n",
        );
        assert_eq!(hint.len(), 2);
    }

    #[test]
    fn pt_br_gate_silences_portuguese_panel_when_only_english_selected() {
        let src_pt = "Escrito por [seu nome], colaborador do blog.\n";
        assert!(diagnostics_for_natlangs(src_pt, &[NatLang::En]).is_empty());

        let src_en = "Written by [Your Name], a contributor to the blog.\n";
        assert!(diagnostics_for_natlangs(src_en, &[NatLang::PtBr]).is_empty());
    }
}
