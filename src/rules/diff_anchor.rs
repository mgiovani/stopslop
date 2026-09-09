use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::{first_byte_per_line, ProseDoc};
use crate::registry::RuleDef;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP036",
    name: "Diff-anchored documentation",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En, NatLang::PtBr],
    default_on: true,
    path_gated: false,
    check,
};

/// Documentation that narrates a change instead of describing the current state.
///
/// The catalog's `no longer (uses|needs|requires|supports)` alternative is deliberately DROPPED:
/// it can't be implemented deterministically. Unlike every other alternative here, "no longer
/// requires X" is indistinguishable by regex from an ordinary PRESENT-TENSE capability
/// description ("a traffic spike no longer requires an emergency deploy") that never narrates a
/// change at all -- it just describes what's true now. Concretely, it false-positives on real,
/// non-slop prose elsewhere in this repo's own fixture corpus (clean_hedging.md), with no nearby
/// textual signal (no "before"/"previously"/"used to") to disambiguate the two readings.
static DIFF_ANCHOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)((?:was|were) (?:added|introduced|removed|renamed|replaced|changed|updated) (?:to|in|for|with)|this (?:replaces|supersedes|deprecates) the (?:old|previous|former|legacy)|previously,? (?:this|it|the \w+) (?:was|used|had|would)|we(?:'ve| have) (?:changed|updated|switched|migrated|moved) (?:this|it|the)|now uses \w+ instead of|used to (?:be|use|have|require)|in the old (?:version|implementation|code))(?-u:\b)")
        .unwrap()
});

/// Brazilian-Portuguese twin of `DIFF_ANCHOR_RE`. Measured against a 318-document,
/// 1.3-million-word human corpus and dropped above 2 hits: `foi/foram adicionado/introduzido/removido/... em/para` (18 human hits,
/// 14 docs -- ordinary encyclopedic history), `costumava ser` (3), `mudamos/atualizamos isso` (1).
/// Every alternative ends on an ASCII letter, so the trailing `(?-u:\b)` is valid throughout.
static DIFF_ANCHOR_PT_BR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?-u:\b)(?:(?:isso|isto|este|esta) substitui (?:o|a) (?:antig|anterior|legad|velh)[oa]s?|anteriormente,? (?:isso|isto|ele|ela|o \w+|a \w+) (?:era|usava|tinha|fazia)|agora usa \w+ em vez de|na (?:vers[ãa]o|implementa[çc][ãa]o) (?:antiga|anterior)|no c[óo]digo (?:antigo|anterior|legado))(?-u:\b)")
        .unwrap()
});

/// Headings under which diff-anchored narration is expected and exempt.
const EXEMPT_HEADINGS: &[&str] = &[
    "changelog",
    "release notes",
    "migration",
    "migration guide",
    "upgrading",
    "what's new",
    "breaking changes",
    "deprecations",
];

/// Brazilian-Portuguese twin of `EXEMPT_HEADINGS`. File-prefix exemptions
/// (`EXEMPT_FILE_PREFIXES`) stay as-is: CHANGELOG/RELEASE/... basenames are the same in
/// Portuguese repos.
const EXEMPT_HEADINGS_PT_BR: &[&str] = &[
    "changelog",
    "histórico de mudanças",
    "histórico de alterações",
    "notas de versão",
    "notas da versão",
    "notas de lançamento",
    "migração",
    "guia de migração",
    "atualizando",
    "atualização",
    "novidades",
    "o que há de novo",
    "mudanças incompatíveis",
    "descontinuações",
    "depreciações",
];

/// Basenames (uppercased) that exempt the whole file.
const EXEMPT_FILE_PREFIXES: &[&str] = &[
    "CHANGELOG",
    "RELEASE",
    "NEWS",
    "HISTORY",
    "MIGRATION",
    "UPGRADING",
];

fn file_exempt(display_path: &str) -> bool {
    let base = Path::new(display_path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("")
        .to_uppercase();
    EXEMPT_FILE_PREFIXES.iter().any(|p| base.starts_with(p))
}

/// True if the nearest heading at or before `byte` is one of the exempt "this document narrates
/// change on purpose" section titles.
fn under_exempt_heading(doc: &ProseDoc, byte: usize) -> bool {
    doc.headings
        .iter()
        .filter(|h| h.byte_start <= byte)
        .max_by_key(|h| h.byte_start)
        .is_some_and(|h| {
            let title = h.text.trim().to_lowercase();
            EXEMPT_HEADINGS.contains(&title.as_str())
                || EXEMPT_HEADINGS_PT_BR.contains(&title.as_str())
        })
}

/// Scans the masked prose stream for diff-anchored narration (headings in scope, frontmatter
/// and URLs skipped). The whole file is exempt when its basename looks like a changelog/release
/// note; a match is exempt when it falls under an exempt heading (a changelog embedded in
/// ordinary docs). One diagnostic per matching line.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    if file_exempt(&ctx.display_path) {
        return;
    }

    let mut bytes: Vec<usize> = Vec::new();
    if ctx.natlangs.contains(&NatLang::En) {
        bytes.extend(
            DIFF_ANCHOR_RE
                .find_iter(&doc.masked)
                .map(|m| m.start())
                .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte))
                .filter(|&byte| !under_exempt_heading(doc, byte)),
        );
    }
    if ctx.natlangs.contains(&NatLang::PtBr) {
        bytes.extend(
            DIFF_ANCHOR_PT_BR
                .find_iter(&doc.masked)
                .map(|m| m.start())
                .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte))
                .filter(|&byte| !under_exempt_heading(doc, byte)),
        );
    }
    let by_line = first_byte_per_line(doc, bytes.into_iter());
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            "documentation narrates a change instead of describing current behavior",
            "describe what the code does now",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::prose::ProseDoc;

    fn diagnostics_for_path_natlangs(
        src: &str,
        display_path: &str,
        natlangs: &[NatLang],
    ) -> Vec<Diagnostic> {
        let doc = ProseDoc::parse(src);
        let ctx = LintContext {
            display_path: display_path.to_string(),
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

    fn diagnostics_for_path(src: &str, display_path: &str) -> Vec<Diagnostic> {
        diagnostics_for_path_natlangs(src, display_path, crate::lang::ALL_NATLANGS)
    }

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        diagnostics_for_path(src, "test.md")
    }

    fn diagnostics_for_natlangs(src: &str, natlangs: &[NatLang]) -> Vec<Diagnostic> {
        diagnostics_for_path_natlangs(src, "test.md", natlangs)
    }

    #[test]
    fn flags_was_added_to() {
        let src = "The retry option was added to the client last quarter.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP036");
    }

    #[test]
    fn clean_no_longer_requires_is_deliberately_unclaimed() {
        // "no longer requires" is dropped from the panel (see the static's doc comment): it
        // reads just as naturally as a present-tense capability description as it does diff
        // narration, and this rule must stay silent on it either way.
        let src = "A traffic spike no longer requires an emergency deploy.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_used_to_be() {
        let src = "The endpoint used to be synchronous before this release.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn clean_present_tense_description() {
        let src = "The client retries once on a timeout and logs the failure.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn file_exempt_by_changelog_basename() {
        let src = "The retry option was added to the client last quarter.\n";
        assert!(diagnostics_for_path(src, "docs/CHANGELOG.md").is_empty());
        assert!(diagnostics_for_path(src, "release-notes/RELEASE.md").is_empty());
    }

    #[test]
    fn exempt_under_changelog_heading() {
        let src = "# Changelog\n\nThe retry option was added to the client last quarter.\n\n# Usage\n\nThe client retries once on a timeout.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_when_outside_the_exempt_heading() {
        let src = "# Changelog\n\nThe retry option was added to the client last quarter.\n\n# Usage\n\nThe endpoint used to require manual configuration.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 7);
    }

    /// One sample per `DIFF_ANCHOR_PT_BR` top-level alternative.
    #[test]
    fn flags_each_pt_br_diff_anchor_alternative() {
        let cases = [
            "Isso substitui o antigo sistema de filas usado antes.\n",
            "Anteriormente, isso era feito manualmente pela equipe de suporte.\n",
            "Agora usa Redis em vez de Memcached para o cache de sessão.\n",
            "Na versão anterior, o timeout padrão era de dez segundos.\n",
            "No código antigo, cada requisição abria uma nova conexão.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP036");
        }
    }

    /// Every one of these has human hits in the pt-BR corpus (see `DIFF_ANCHOR_PT_BR`'s doc
    /// comment) and stays out of the panel.
    #[test]
    fn clean_pt_br_dropped_shapes() {
        for src in [
            "A opção de retry foi adicionada ao cliente no trimestre passado.\n",
            "O endpoint costumava ser síncrono antes deste lançamento.\n",
            "Nós atualizamos isso depois de receber feedback dos usuários.\n",
        ] {
            assert!(
                diagnostics_for(src).is_empty(),
                "unexpectedly flagged: {src:?}"
            );
        }
    }

    #[test]
    fn exempt_under_pt_br_changelog_heading() {
        let src = "# Histórico de Mudanças\n\nNa versão anterior, o timeout padrão era de dez segundos.\n\n# Uso\n\nO cliente tenta novamente uma vez.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_when_outside_the_pt_br_exempt_heading() {
        let src = "# Histórico de Mudanças\n\nNa versão anterior, o timeout padrão era de dez segundos.\n\n# Uso\n\nNo código antigo, cada requisição abria uma nova conexão.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn natlang_gate_silences_the_other_languages_panel() {
        let pt_positive = "Isso substitui o antigo sistema de filas usado antes.\n";
        assert!(diagnostics_for_natlangs(pt_positive, &[NatLang::En]).is_empty());

        let en_positive = "The retry option was added to the client last quarter.\n";
        assert!(diagnostics_for_natlangs(en_positive, &[NatLang::PtBr]).is_empty());
    }
}
