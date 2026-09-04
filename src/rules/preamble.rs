use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, CODE_LANGS};
use crate::prose_words::REASONING_CHAIN_FRAGMENT;
use crate::registry::RuleDef;
use crate::rules::residue::REASONING_CHAIN_FRAGMENT_PT_BR;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP002",
    name: "Chat preamble leaked into code",
    tier: Tier::A,
    langs: CODE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

// Shares REASONING_CHAIN_FRAGMENT with rules::residue (SLOP011) so a new phrase needs adding
// once. "step 1:" stays local: real precision/recall tradeoff, since ordinary numbered-step
// comments look similar and each consumer resolves the position differently.
static RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?im)^\s*(?://|#|\*+)\s*(certainly[!,]|sure[!,]|here'?s the (updated|revised|complete|new|fixed)|below is the (updated|complete|full)|as an ai(?-u:\b)|i hope this helps|step 1:|{REASONING_CHAIN_FRAGMENT})",
    ))
    .unwrap()
});

/// Portuguese chat-preamble panel, mirroring `RE`: line-initial code-comment openers, self-ID,
/// closers, and reasoning-chain leakage (shared fragment, see `rules::residue`). Bare "como ia
/// dizendo" ("as I was saying" -- the verb *ir*'s imperfect tense "ia") is excluded on purpose:
/// the panel requires the article "uma" before "ia" (self-identifying AS an AI), which "como ia
/// dizendo" never has. `passo 1:`/`etapa 1:` mirror the English line-initial `step 1:` shape --
/// SLOP011 (the prose rule) drops `passo N:` entirely because bulleted step lists are structure
/// there, but a line-initial CODE COMMENT is exactly the shape the English panel already flags
/// here, so parity holds. The self-ID article is `um[a]?` rather than a fixed `uma`: "modelo de
/// linguagem" is masculine ("como um modelo de linguagem"), "inteligência artificial" is feminine
/// ("como uma IA"), so the fixed feminine article missed the masculine phrasing entirely.
static RE_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?im)^\s*(?://|#|\*+)\s*(?:claro[!,]|com certeza[!,]|certamente[!,]|aqui est[áa] [oa] \w+ (?:atualizad|revisad|complet|nov|corrigid)|segue (?:abaixo )?[oa] \w+ (?:atualizad|revisad|complet|nov|corrigid)|abaixo est[áa] [oa] \w+ (?:atualizad|complet)|como uma ia(?-u:\b)|como (?:um[a]? )?(?:intelig[êe]ncia artificial|modelo de linguagem)|espero que (?:isso )?ajude|espero ter ajudado|passo 1:|etapa 1:|{REASONING_CHAIN_FRAGMENT_PT_BR})",
    ))
    .unwrap()
});

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let en = ctx.natlangs.contains(&NatLang::En);
    let pt = ctx.natlangs.contains(&NatLang::PtBr);
    for c in ctx.comments {
        if c.is_doc {
            continue;
        }
        let hit = (en && RE.is_match(c.text)) || (pt && RE_PT_BR.is_match(c.text));
        if hit {
            out.push(Diagnostic::at(
                rule,
                ctx,
                c.line,
                c.col,
                "chat-preamble text leaked into a source comment",
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
    fn matches_preamble() {
        assert!(RE.is_match("// Certainly! Here's the fix"));
        assert!(RE.is_match("# Sure, below is the complete solution:"));
        assert!(RE.is_match("// As an AI language model, here's the solution:"));
    }

    #[test]
    fn does_not_match_explanation() {
        assert!(!RE.is_match("# Here's a breakdown of the parser logic:"));
    }

    #[test]
    fn matches_reasoning_chain_leakage() {
        assert!(RE.is_match("// Let's think about this differently"));
        assert!(RE.is_match("// Step 1: parse the config file"));
        assert!(RE.is_match("# Breaking this down into smaller pieces"));
    }

    #[test]
    fn does_not_match_ordinary_numbered_comment() {
        assert!(!RE.is_match("// The first step here validates the config."));
    }

    #[test]
    fn matches_pt_br_preamble() {
        assert!(RE_PT_BR.is_match("// Claro! Aqui está o código atualizado"));
        assert!(RE_PT_BR.is_match("# Segue abaixo a versão revisada:"));
        assert!(RE_PT_BR.is_match("// Como uma IA, revisei o código automaticamente"));
        assert!(RE_PT_BR.is_match("// Passo 1: validar entrada"));
        assert!(RE_PT_BR.is_match("# Etapa 1: preparar ambiente"));
    }

    #[test]
    fn matches_pt_br_reasoning_chain_leakage() {
        assert!(RE_PT_BR.is_match("// Vamos pensar passo a passo antes de aplicar a correção"));
        assert!(RE_PT_BR.is_match("# Vamos pensar sobre isso com calma"));
    }

    #[test]
    fn re_pt_br_alternatives() {
        let samples: &[&str] = &[
            "// claro!",
            "// com certeza,",
            "// certamente!",
            "// aqui está o código atualizado",
            "// segue abaixo a versão revisada",
            "// abaixo está a solução completa",
            "// como uma IA",
            "// como inteligência artificial",
            "// como um modelo de linguagem",
            "// espero que isso ajude",
            "// espero ter ajudado",
            "// passo 1:",
            "// etapa 1:",
            "// vamos pensar passo a passo",
        ];
        for s in samples {
            assert!(RE_PT_BR.is_match(s), "{s}");
        }
    }

    #[test]
    fn ignores_pt_br_como_ia_dizendo() {
        // "como ia dizendo" is the verb *ir* ("as I was saying"), not AI self-ID; the panel
        // requires the article "uma" before "ia" to tell the two apart.
        assert!(!RE_PT_BR.is_match("// Como ia dizendo, o cache expira em uma hora"));
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
            image: None,
            natlangs,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn pt_br_gate_silences_portuguese_panel_when_only_english_selected() {
        let src_pt = "// Claro! Aqui está o código atualizado\nfunction f() {}\n";
        assert!(diagnostics_for_natlangs(src_pt, &[NatLang::En]).is_empty());

        let src_en = "// Certainly! Here's the fix\nfunction f() {}\n";
        assert!(diagnostics_for_natlangs(src_en, &[NatLang::PtBr]).is_empty());
    }

    #[test]
    fn pt_br_panel_fires_through_check_under_default_union() {
        let src = "// Claro! Aqui está o código atualizado\nfunction f() {}\n";
        let diags = diagnostics_for_natlangs(src, crate::lang::ALL_NATLANGS);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP002");
    }
}
