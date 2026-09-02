use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{NatLang, PROSE_LANGS};
use crate::prose::first_byte_per_line;
use crate::prose_words::CLICHE_PHRASES;
use crate::registry::RuleDef;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP014",
    name: "Formulaic cliché phrase",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: &[NatLang::En],
    default_on: true,
    path_gated: false,
    check,
};

/// Scans the masked prose stream for stock marketing/narrative clichés ("unlock the power of",
/// "embark on a journey", "a testament to", ...). Headings are in scope (a clichéd heading like
/// "## Unlock the Power of X" is a prime target); frontmatter and URL spans are skipped. Emits
/// one diagnostic per matching line, anchored at that line's first (leftmost) match.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let bytes = CLICHE_PHRASES
        .find_iter(&doc.masked)
        .map(|m| m.start())
        .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte));
    let by_line = first_byte_per_line(doc, bytes);
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "formulaic cliché phrase; rewrite it",
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
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn flags_cliche_in_heading_and_body() {
        let src = "## Unlock the Power of Your Data\n\nOur tool helps you navigate the complexities of deployment.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.code == "SLOP014"));
    }

    #[test]
    fn clean_ordinary_prose() {
        let src = "stopslop lints markdown for formulaic phrasing before it ships.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn skips_cliche_inside_url() {
        // The phrase regex requires literal spaces ("on a journey"); a hyphenated URL slug
        // never matches on text alone, and is additionally masked via in_url as a
        // belt-and-suspenders check.
        let src = "Read more at https://example.com/your-journey-to-mastery today.\n"; // ai-slop-ignore
        assert!(diagnostics_for(src).is_empty());
    }
}
