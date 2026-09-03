use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::registry::RuleDef;
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP034",
    name: "Synonym rotation across a closed concept set",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

struct Member {
    name: &'static str,
    re: Regex,
}

/// `pat` may itself be a top-level alternation (e.g. "run(?:s|ning)?|ran"), so the boundary
/// anchors must wrap the whole thing in a non-capturing group -- `(?-u:\b){pat}(?-u:\b)` would
/// only bind `(?-u:\b)` to the first alternative.
fn set(members: &[(&'static str, &str)]) -> Vec<Member> {
    members
        .iter()
        .map(|&(name, pat)| Member {
            name,
            re: Regex::new(&format!(r"(?i)(?-u:\b)(?:{pat})(?-u:\b)")).unwrap(),
        })
        .collect()
}

/// Ten closed concept sets, each member word-bounded/case-insensitive and stem-tolerant where
/// sensible (verb inflections; bare adjectives left uninflected). The catalog's "use / utilize /
/// leverage / employ" set is narrowed to "use / employ": "utilize" and "leverage" are already
/// matched bare by prose_words.rs's VOCAB_TIER2 (SLOP016) -- keeping them here would double-flag
/// the same span under two rules.
static SETS: LazyLock<Vec<Vec<Member>>> = LazyLock::new(|| {
    vec![
        set(&[
            ("check", r"check(?:s|ed|ing)?"),
            ("verify", r"verif(?:y|ies|ied|ying)"),
            ("confirm", r"confirm(?:s|ed|ing)?"),
            ("validate", r"validat(?:e|es|ed|ing)"),
        ]),
        set(&[
            ("config", r"config"),
            ("configuration", r"configurations?"),
            ("settings", r"settings"),
        ]),
        set(&[
            ("delete", r"delet(?:e|es|ed|ing)"),
            ("remove", r"remov(?:e|es|ed|ing)"),
        ]),
        set(&[
            ("run", r"run(?:s|ning)?|ran"),
            ("execute", r"execut(?:e|es|ed|ing)"),
            ("invoke", r"invok(?:e|es|ed|ing)"),
            ("launch", r"launch(?:es|ed|ing)?"),
        ]),
        set(&[
            ("show", r"show(?:s|ed|ing|n)?"),
            ("display", r"display(?:s|ed|ing)?"),
        ]),
        // "utilize"/"leverage" dropped: see module doc comment above.
        set(&[
            ("use", r"us(?:e|es|ed|ing)"),
            ("employ", r"employ(?:s|ed|ing)?"),
        ]),
        set(&[
            ("fast", r"fast"),
            ("quick", r"quick(?:ly)?"),
            ("rapid", r"rapid(?:ly)?"),
            ("speedy", r"speedy"),
        ]),
        set(&[
            ("start", r"start(?:s|ed|ing)?"),
            ("begin", r"begin(?:s|ning)?|began|begun"),
            ("commence", r"commenc(?:e|es|ed|ing)"),
            ("initiate", r"initiat(?:e|es|ed|ing)"),
        ]),
        set(&[
            ("create", r"creat(?:e|es|ed|ing)"),
            ("generate", r"generat(?:e|es|ed|ing)"),
            ("produce", r"produc(?:e|es|ed|ing)"),
        ]),
        set(&[
            ("change", r"chang(?:e|es|ed|ing)"),
            ("modify", r"modif(?:y|ies|ied|ying)"),
            ("alter", r"alter(?:s|ed|ing)?"),
            ("adjust", r"adjust(?:s|ed|ing)?"),
        ]),
    ]
});

/// Portuguese twin of [`SETS`], the five concept sets that stay quiet on human Portuguese,
/// consulted only when `NatLang::PtBr` is enabled. `config` is spelled the same in both
/// languages and stays in [`SETS`] only, so a Portuguese-only run never claims it under a
/// Portuguese label.
///
/// Files with a finding per set, measured on the 129 translated Python-docs pages and 143
/// featured pt.wikipedia articles of the human corpus against 94 generated summaries, ceiling
/// 10% of either human subset: `verificar`/`validar`/`conferir`/`checar` 3 / 0 / 1,
/// `configuração`/`ajustes`/`definições` 1 / 0 / 2, `excluir`/`remover`/`apagar`/`deletar`
/// 7 / 1 / 0, `executar`/`rodar`/`invocar`/`disparar` 6 / 0 / 0, `rápido`/`veloz` 0 / 0 / 0.
/// Dropped over the ceiling: `mostrar`/`exibir`/`apresentar` 32 / 47 / 7, `iniciar`/`começar`/
/// `inicializar` 11 / 31 / 0, `criar`/`gerar`/`produzir` 28 / 45 / 36, `alterar`/`modificar`/
/// `mudar`/`ajustar` 32 / 12 / 5, `usar`/`utilizar`/`empregar` 23 pydocs pages. Each of those is
/// ordinary Portuguese narrative or technical prose ("iniciou ... começou", "cria ... gera"),
/// not one author rotating a term. `ágil` left the speed set: it starts on an accented letter,
/// so the ASCII leading boundary cannot anchor it and the bare pattern also matched `frágil`.
static SETS_PT_BR: LazyLock<Vec<Vec<Member>>> = LazyLock::new(|| {
    vec![
        set(&[
            ("verificar", r"verific(?:ar|a|am|ou|ando|ado|ada)"),
            ("validar", r"valid(?:ar|a|am|ou|ando|ado|ada)"),
            ("conferir", r"confer(?:ir|e|em|iu|indo|ido|ida)"),
            ("checar", r"chec(?:ar|a|am|ou|ando|ado|ada)"),
        ]),
        set(&[
            ("configuração", r"configura[çc](?:[ãa]o|[õo]es)"),
            ("ajustes", r"ajustes?"),
            ("definições", r"defini[çc][õo]es"),
        ]),
        set(&[
            ("excluir", r"exclu(?:ir|i|em|iu|indo|[íi]do|[íi]da)"),
            ("remover", r"remov(?:er|e|em|eu|endo|ido|ida)"),
            ("apagar", r"apag(?:ar|a|am|ou|ando|ado|ada)"),
            ("deletar", r"delet(?:ar|a|am|ou|ando|ado|ada)"),
        ]),
        set(&[
            ("executar", r"execut(?:ar|a|am|ou|ando|ado|ada)"),
            // Bare `a` and `ada` dropped: they'd match the common nouns "roda" (wheel) and
            // "rodada" (a round/lap) instead of a conjugation of "rodar" (to run).
            ("rodar", r"rod(?:ar|am|ou|ando|ado)"),
            ("invocar", r"invoc(?:ar|a|am|ou|ando|ado|ada)"),
            ("disparar", r"dispar(?:ar|a|am|ou|ando|ado|ada)"),
        ]),
        set(&[("rápido", r"r[áa]pid[oa]s?"), ("veloz", r"veloz(?:es)?")]),
    ]
});

/// `(?-u:\b)` is an ASCII-only boundary, so an accented letter counts as a word edge under it:
/// "verificação" matched the bare `a` alternative of `verific(?:ar|a|...)` (leftmost-first stops
/// at "verifica" and the `ç` that follows passes as a boundary), and "validação" and "invocação"
/// did the same. A letter on either edge means the match sits inside a longer word. Only letters
/// count: a curly quote or an em dash next to the match is a real edge, and a Unicode `\b` in the
/// pattern itself is the PikeVM trap AGENTS.md names.
fn touches_letter(masked: &str, start: usize, end: usize) -> bool {
    let before = masked[..start].chars().next_back();
    let after = masked[end..].chars().next();
    before.is_some_and(char::is_alphabetic) || after.is_some_and(char::is_alphabetic)
}

/// True when `byte` falls inside some block's `[first_byte, end_byte)`. `blocks` comes from
/// `fragmentation::paragraph_blocks`, which returns blocks in ascending, non-overlapping byte
/// order: the Markdown scan walks `line_spans` once from byte 0 forward, pushing one `Block` per
/// contiguous non-blank line run in the order it meets them (`fragmentation.rs`, the `while i <
/// spans.len()` loop around line 120); the HTML scan (`html_blocks`) maps `doc.paragraphs` 1:1,
/// and those ranges come from a single forward walk over `scan.blocks`, each truncated at its
/// first child so siblings never overlap. That lets this binary-search with `partition_point`
/// instead of scanning every block per byte -- the same idiom as `section_of` above. Was an
/// O(matches x blocks) linear scan, measured at ~50% of the default lint wall time on a 20 MB
/// file (issue #21 phase-2: 3.68s -> 1.82s with this rule ignored).
fn in_prose_blocks(blocks: &[super::fragmentation::Block], byte: usize) -> bool {
    let idx = blocks.partition_point(|b| b.first_byte <= byte);
    idx > 0 && byte < blocks[idx - 1].end_byte
}

/// For each closed concept set, counts occurrences per member within one SECTION's running prose.
/// A member "qualifies" once it occurs `>= 2` times there. Fires at most one diagnostic per set,
/// only when two or more distinct members qualify in the same section, anchored at the first
/// occurrence of the second-seen qualifying member (chronologically, by first occurrence).
///
/// Scope is a section rather than the whole file because a document that enumerates
/// differently-named things gets its "competing" words from unrelated entries -- a skill catalog
/// flagged `generate, create` where the two words described two different skills. Two scope
/// decisions, and both are load-bearing:
///
/// - **Sections, not paragraphs.** Genuine rotation is one author drifting across a passage; the
///   rule's own fixture spreads `check` x2 and `verify` x2 over four consecutive paragraphs.
///   Paragraph scoping would delete that true positive.
/// - **Only `fragmentation::paragraph_blocks`.** That helper already drops headings, list items,
///   tables, rules, link-reference definitions, and comment lines -- "bullet lists, tables, and
///   headings must never be treated as sentences", per its own doc comment. Both known false
///   positives were list items, and a 17-bullet catalog under one heading is not fixed by section
///   scoping alone. This also subsumes the separate "skip tokens in headings" concern.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    let blocks = super::fragmentation::paragraph_blocks(doc);
    if blocks.is_empty() {
        return;
    }
    let heading_starts: Vec<usize> = doc.headings.iter().map(|h| h.byte_start).collect();
    let section_of = |byte: usize| heading_starts.partition_point(|&s| s <= byte);
    let in_prose = |byte: usize| in_prose_blocks(&blocks, byte);

    let en = ctx.natlangs.contains(&NatLang::En);
    let pt_br = ctx.natlangs.contains(&NatLang::PtBr);
    let sets = SETS
        .iter()
        .filter(|_| en)
        .chain(SETS_PT_BR.iter().filter(|_| pt_br));

    for set in sets {
        // section -> [(member name, first byte in that section)]
        let mut by_section: BTreeMap<usize, Vec<(&str, usize)>> = BTreeMap::new();
        for member in set {
            let mut per_section: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
            for m in member.re.find_iter(&doc.masked) {
                let byte = m.start();
                if doc.in_frontmatter(byte)
                    || doc.in_url(byte)
                    || !in_prose(byte)
                    || touches_letter(&doc.masked, byte, m.end())
                {
                    continue;
                }
                per_section.entry(section_of(byte)).or_insert((0, byte)).0 += 1;
            }
            for (section, (count, first_byte)) in per_section {
                if count >= 2 {
                    by_section
                        .entry(section)
                        .or_default()
                        .push((member.name, first_byte));
                }
            }
        }
        // At most one diagnostic per set, as before: the earliest section that rotates.
        let Some(mut qualifying) = by_section.into_values().find(|members| members.len() >= 2)
        else {
            continue;
        };
        qualifying.sort_by_key(|&(_, byte)| byte);
        let first_member = qualifying[0].0;
        let anchor_byte = qualifying[1].1;
        let names: Vec<&str> = qualifying.iter().map(|&(name, _)| name).collect();
        let (line, col) = doc.line_col(anchor_byte);
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            format!(
                "synonym rotation across a closed concept set: {}",
                names.join(", ")
            ),
            format!("pick one term and use it throughout: {first_member}"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::prose::ProseDoc;

    /// `in_prose_blocks` must agree with a linear scan for every byte of a fixture that mixes
    /// headings, list items, a table, and prose paragraphs across several blocks -- the same
    /// brute-force-comparison pattern `prose.rs` uses for `in_heading`/`url_span_at`.
    #[test]
    fn in_prose_blocks_matches_brute_force_scan_for_every_byte() {
        let src = "# Heading\n\nFirst paragraph spans one line.\n\nSecond paragraph\nspans two lines.\n\n- list item one\n- list item two\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nTail paragraph after the table.\n";
        let doc = ProseDoc::parse(src);
        let blocks = crate::rules::fragmentation::paragraph_blocks(&doc);
        assert!(
            blocks.len() >= 3,
            "fixture should contain multiple prose blocks"
        );

        fn brute(blocks: &[crate::rules::fragmentation::Block], byte: usize) -> bool {
            blocks
                .iter()
                .any(|b| byte >= b.first_byte && byte < b.end_byte)
        }

        for byte in 0..=src.len() {
            assert_eq!(
                in_prose_blocks(&blocks, byte),
                brute(&blocks, byte),
                "mismatch at byte {byte}"
            );
        }
    }

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
    fn flags_rotation_between_two_qualifying_members() {
        let src = "Check the response. Check it twice. Now verify the response. Verify it again.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP034");
        assert!(diags[0].message.contains("check"));
        assert!(diags[0].message.contains("verify"));
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("pick one term and use it throughout: check")
        );
    }

    /// A catalog of differently-named things: the competing words describe different entries, so
    /// they are not one author rotating terms. Both are real strings from a 130-document corpus.
    #[test]
    fn list_items_are_not_pooled_together() {
        let catalog = "## Development\n\n- **ci-generate**: Generate a production-ready CI/CD pipeline config\n- **docs-check**: Check documentation against the codebase and report drift\n- **db-migrate**: Create, validate, and manage database migrations across any framework\n- **test-suite**: Generate test suites by analyzing coverage gaps\n";
        assert!(diagnostics_for(catalog).is_empty());

        let changelog = "## Security\n\n- Phase 3: Parallel Vulnerability Scanning\n  - Agent 1: Access Control & Authentication (A01, A07)\n  - Agent 2: Configuration & Insecure Design (A02, A06)\n  - Agent 3: Injection & Data Integrity (A05, A08)\n\nThe config file is read once at startup.\n";
        assert!(diagnostics_for(changelog).is_empty());
    }

    /// Rotation split across two sections is two authors' vocabularies, not one drifting.
    #[test]
    fn sections_are_counted_separately() {
        let src = "## Setup\n\nCheck the response. Check it twice.\n\n## Teardown\n\nNow verify the response. Verify it again.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// The rule's own fixture spreads the two members over four consecutive paragraphs under one
    /// heading -- paragraph scoping would have deleted this true positive.
    #[test]
    fn rotation_pools_across_paragraphs_within_one_section() {
        let src = "# Deployment Checklist\n\nCheck the health endpoint before promoting.\n\nCheck it again after five minutes.\n\nNow verify the endpoint reports steady latency.\n\nVerify it once more before closing.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP034");
    }

    #[test]
    fn anchors_at_second_seen_member_first_occurrence() {
        // "check" is seen first (word 1), "verify" second (first appears at word 3); anchor
        // must land on that second member's first occurrence, not the very first word.
        let src = "Check it. Verify it. Check it again. Verify it again.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        let verify_byte = src.find("Verify").unwrap();
        let doc = ProseDoc::parse(src);
        let (line, col) = doc.line_col(verify_byte);
        assert_eq!((diags[0].line, diags[0].col), (line, col));
    }

    #[test]
    fn clean_single_member_repeated_alone() {
        // Only one member of the set qualifies; a lone repeated word choice is not rotation.
        let src = "Check the config. Check it again. Check it once more.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_second_member_appears_only_once() {
        // "verify" only appears once, so it never qualifies (needs >= 2).
        let src = "Check the response. Check it twice. Now verify the response once.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn does_not_claim_utilize_or_leverage() {
        // Both belong to VOCAB_TIER2 (SLOP016) already; this rule's "use/employ" set must stay
        // silent on them even when repeated.
        let src = "We utilize the cache. We utilize it again. We leverage the queue. We leverage it again.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// One positive per PT-BR set: two of its members, each repeated twice, in one section.
    #[test]
    fn flags_rotation_for_each_pt_br_set() {
        let cases = [
            ("Vamos verificar a resposta agora. Depois verificar de novo mais tarde. Agora vamos validar a resposta com calma. Depois validar outra vez para garantir.\n", "verificar", "validar"),
            ("A configuração está pronta. A configuração foi revisada. Os ajustes finais chegaram ontem. Os ajustes foram aplicados hoje.\n", "configuração", "ajustes"),
            ("Vamos excluir o arquivo agora. Depois vamos excluir o registro antigo. Então remove a entrada duplicada. Por fim remove o registro extra.\n", "excluir", "remover"),
            ("Vamos executar o script agora. Depois vamos executar o job noturno. Então rodar o teste completo. Por fim rodar o pipeline inteiro.\n", "executar", "rodar"),
            ("O processo é muito rápido hoje. O sistema ficou rápido depois da mudança. Além disso o motor é veloz na subida. O carro permanece veloz na reta.\n", "rápido", "veloz"),
        ];
        for (src, name_a, name_b) in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP034");
            assert!(
                diags[0].message.contains(name_a),
                "{src}: missing {name_a} in {}",
                diags[0].message
            );
            assert!(
                diags[0].message.contains(name_b),
                "{src}: missing {name_b} in {}",
                diags[0].message
            );
        }
    }

    #[test]
    fn clean_pt_br_single_member_repeated_alone() {
        // Only "verificar" qualifies; a lone repeated word choice is not rotation.
        let src = "Vamos verificar o arquivo agora. Verificar de novo. Verificar mais uma vez. Verificar outra vez ainda.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// The dropped catalog sets (see `SETS_PT_BR`'s doc comment) must stay silent: one pair per
    /// dropped set, invented sentences, each member repeated twice in the same section.
    #[test]
    fn clean_pt_br_dropped_sets_do_not_fire() {
        let src = "O painel vai mostrar o resultado. Depois vai mostrar o gráfico. Em seguida vai exibir o relatório. Por fim vai exibir o resumo. O comando vai criar o arquivo. Depois vai criar a pasta. Em seguida vai gerar o relatório. Por fim vai gerar o log. O sistema é ágil. A equipe é ágil. O vidro é frágil. O copo é frágil. Vamos iniciar o processo agora. Depois vamos iniciar outra etapa. Em seguida vamos começar a revisão. Por fim vamos começar o teste. O time vai alterar o arquivo hoje. Depois vai alterar a configuração. Em seguida vai modificar o código legado. Por fim vai modificar o teste. Vamos usar a ferramenta padrão. Depois vamos usar o mesmo script. Em seguida vamos utilizar o novo recurso. Por fim vamos utilizar a biblioteca.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// "roda" (wheel) and "rodada" (a round/lap) are common nouns, not a conjugation of "rodar"
    /// (to run); the trimmed pattern must not treat their bare `a`/`ada` endings as a match.
    #[test]
    fn clean_pt_br_rodar_pattern_does_not_match_wheel_or_round_nouns() {
        let src = "A roda do carro furou hoje. A segunda rodada começa amanhã. A primeira roda foi trocada ontem. A rodada anterior já terminou.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// "verificação"/"validação" continue past the bare `a` alternative into accented letters;
    /// the ASCII `(?-u:\b)` boundary must not treat that continuation as a real word edge.
    #[test]
    fn clean_pt_br_ascii_boundary_does_not_match_accented_continuations() {
        let src = "A verificação de dados é feita aqui. A verificação continua depois. A validação do formulário ocorre em seguida. A validação final confirma tudo.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn natlang_gate_silences_pt_br_sets_under_en_only() {
        let pt_positive = "Vamos verificar a resposta agora. Depois verificar de novo mais tarde. Agora vamos validar a resposta com calma. Depois validar outra vez para garantir.\n";
        assert!(diagnostics_for_natlangs(pt_positive, &[NatLang::En]).is_empty());

        let en_positive =
            "Check the response. Check it twice. Now verify the response. Verify it again.\n";
        assert!(diagnostics_for_natlangs(en_positive, &[NatLang::PtBr]).is_empty());
    }
}
