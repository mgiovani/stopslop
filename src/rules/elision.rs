use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, CODE_LANGS};
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP001",
    name: "Elision / \"rest unchanged\" comment",
    tier: Tier::A,
    langs: CODE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

static RE_A: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?im)^\s*(?://|#|\*+)\s*\.\.\.?\s*(rest|existing|other|remaining|unchanged|keep)(?-u:\b)",
    )
    .unwrap()
});
static RE_B: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\.\.\.\s*(existing code|rest of|unchanged|other methods|remaining)").unwrap()
});

/// Brazilian-Portuguese twin of `RE_A`: a line-initial comment opening with an ellipsis then a
/// "rest/remaining/unchanged" word or a "no change" phrase. `mant[ée]m`/`continua`/`permanece`
/// and the `sem altera[çc][ãa]o`/`sem mudan[çc]as?` phrases are added on top of the bare nouns
/// (`resto`, `restante`) because "kept as-is" is the more common way an assistant phrases this in
/// Portuguese. ASCII-only panel, so the trailing `(?-u:\b)` is safe (issue #21) -- every
/// alternative ends on an ASCII letter, none borders an accented one.
static RE_A_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?im)^\s*(?://|#|\*+)\s*\.\.\.?\s*(?:o |a |os |as )?(?:resto|restante|demais|existente|inalterad[oa]s?|manter|mant[ée]m|continua|permanece|sem altera[çc][ãa]o|sem altera[çc][õo]es|sem mudan[çc]as?)(?-u:\b)",
    )
    .unwrap()
});

/// Brazilian-Portuguese twin of `RE_B`: the phrase anywhere in the comment, same scope as
/// `RE_A_PT_BR`.
static RE_B_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\.\.\.\s*(?:c[óo]digo existente|resto d[oa]|restante d[oa]|demais m[ée]todos|o restante|sem altera[çc][õo]es|inalterad[oa])",
    )
    .unwrap()
});

/// True iff everything from the start of `start_byte`'s line up to `start_byte` is whitespace,
/// i.e. the node is the first non-whitespace thing on its line (kills trailing-after-code FPs
/// and TS `...rest` spread, which is code with no comment node at all).
fn is_first_on_line(source: &str, start_byte: usize) -> bool {
    let line_start = source[..start_byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
    source[line_start..start_byte]
        .chars()
        .all(char::is_whitespace)
}

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let en = ctx.natlangs.contains(&NatLang::En);
    let pt = ctx.natlangs.contains(&NatLang::PtBr);
    for c in ctx.comments {
        if c.is_doc || !is_first_on_line(ctx.source, c.start_byte) {
            continue;
        }
        let hit = (en && (RE_A.is_match(c.text) || RE_B.is_match(c.text)))
            || (pt && (RE_A_PT_BR.is_match(c.text) || RE_B_PT_BR.is_match(c.text)));
        if hit {
            out.push(Diagnostic::at(
                rule,
                ctx,
                c.line,
                c.col,
                "elision comment may have replaced real code",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use tree_sitter::Parser;

    #[test]
    fn matches_rest_unchanged() {
        assert!(RE_A.is_match("// ... rest of code unchanged"));
        assert!(RE_A.is_match("# ... existing code"));
        assert!(RE_B.is_match("// ... other methods unchanged"));
    }

    #[test]
    fn does_not_match_prose() {
        assert!(!RE_A.is_match("// process the rest of the data"));
        assert!(!RE_B.is_match("// process the rest of the data"));
    }

    #[test]
    fn first_on_line_detection() {
        let src = "let x = 1; // ...rest\n// ...rest\n";
        // trailing-after-code comment starts at the position right after "let x = 1; "
        let trailing_start = src.find("// ...rest").unwrap();
        assert!(!is_first_on_line(src, trailing_start));
        let leading_start = src.rfind("// ...rest").unwrap();
        assert!(is_first_on_line(src, leading_start));
    }

    #[test]
    fn matches_pt_br_rest_unchanged() {
        assert!(RE_A_PT_BR.is_match("// ... resto do código sem alteração"));
        assert!(RE_A_PT_BR.is_match("# ... o restante permanece"));
        assert!(RE_A_PT_BR.is_match("// ... mantém o restante"));
        assert!(RE_B_PT_BR.is_match("// ... resto da lógica inalterado"));
        assert!(RE_B_PT_BR.is_match("// ... demais métodos sem alteração"));
    }

    #[test]
    fn ignores_pt_br_comment_without_ellipsis() {
        // No leading "..." -- ordinary Portuguese comment describing what's elsewhere, not an
        // elision marker.
        assert!(!RE_A_PT_BR.is_match("// resto da lógica fica no módulo de auth"));
        assert!(!RE_B_PT_BR.is_match("// resto da lógica fica no módulo de auth"));
    }

    fn diagnostics_for_natlangs(src: &str, natlangs: &'static [NatLang]) -> Vec<Diagnostic> {
        let mut p = Parser::new();
        p.set_language(&crate::lang::ts_language(Lang::Ts)).unwrap();
        let tree = p.parse(src, None).unwrap();
        let (comments, strings, index) = crate::context::extract(&tree, src, Lang::Ts);
        let ctx = LintContext {
            display_path: "test.ts".to_string(),
            source: src,
            index: Some(&index),
            lang: Lang::Ts,
            comments: &comments,
            strings: &strings,
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: None,
            natlangs,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn pt_br_gate_silences_portuguese_panel_when_only_english_selected() {
        let src_pt = "// ... resto do código sem alteração\nfunction f() {}\n";
        assert!(diagnostics_for_natlangs(src_pt, &[NatLang::En]).is_empty());

        let src_en = "// ... rest of code unchanged\nfunction f() {}\n";
        assert!(diagnostics_for_natlangs(src_en, &[NatLang::PtBr]).is_empty());
    }

    #[test]
    fn pt_br_panel_fires_through_check_under_default_union() {
        let src = "// ... resto do código sem alteração\nfunction f() {}\n";
        let diags = diagnostics_for_natlangs(src, crate::lang::ALL_NATLANGS);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP001");
    }
}
