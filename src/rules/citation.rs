use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, PROSE_LANGS};
use crate::prose::first_byte_per_line;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP012",
    name: "LLM tool / citation artifact tokens",
    tier: Tier::A,
    langs: PROSE_LANGS,
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

/// Literal leftover strings from an LLM's internal search/citation/rendering pipeline.
/// Mechanical proof of an unedited paste -- case-sensitive where the catalog is distinctive
/// about casing (no global `(?i)`: these tokens only ever appear in this exact shape).
static RE_TOKENS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"turn\d+(search|view|news|forecast|image|product)\d+|:?contentReference\[oaicite:\s*\d+\]|(?-u:\b)oai_citation(?-u:\b)|\[attached_file:\d+\]|(?-u:\b)grok_card(?-u:\b)|(?-u:\b)grok_render_citation_card_json(?-u:\b)|【\d+†[Ll]\d+(?:-L?\d+)?】|\[cite:\s*\d+(?:\s*,\s*\d+)*\]|utm_source=chatgpt\.com|:::writing|(?-u:\b)attributableIndex(?-u:\b)").unwrap()
});

/// Citation-count "+N" artifact (lower confidence, so require the repeated form or an
/// end-of-line anchor to avoid a version-string false positive like `v2+1`): either two or
/// more capitalized-base "+digits" units chained back to back, or one such unit as the very
/// last thing on the line.
static RE_CITATION_COUNT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)(?-u:\b)[A-Z][A-Za-z.]{2,}\+\d+(?:[A-Z][A-Za-z.]{2,}\+\d+)+(?-u:\b)|(?-u:\b)[A-Z][A-Za-z.]{2,}\+\d+\s*$")
        .unwrap()
});

/// Scope: headings in scope, frontmatter in scope, URLs IN SCOPE -- `utm_source=chatgpt.com`
/// lives inside a URL, so this rule must NOT skip `doc.in_url`. Only code (already blanked in
/// `doc.masked`) is excluded. One diagnostic per matching line, first byte wins.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let bytes = [&*RE_TOKENS, &*RE_CITATION_COUNT]
        .into_iter()
        .flat_map(|re| re.find_iter(&doc.masked).map(|m| m.start()));
    let by_line = first_byte_per_line(doc, bytes);
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "LLM tool/citation artifact token left in text",
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
    fn flags_search_tool_token() {
        let diags = diagnostics_for("The figures were pulled from turn0search0 directly.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP012");
    }

    #[test]
    fn flags_content_reference_token() {
        let diags = diagnostics_for("Latency dropped 18%.:contentReference[oaicite:1]{index=1}\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_bracketed_source_marker() {
        let diags = diagnostics_for("This was confirmed in the thread [cite: 4].\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_utm_source_inside_url() {
        let diags = diagnostics_for(
            "See https://example.com/blog?utm_source=chatgpt.com for the announcement.\n", // ai-slop-ignore
        );
        assert_eq!(
            diags.len(),
            1,
            "utm_source must fire even inside a URL span"
        );
    }

    #[test]
    fn flags_repeated_citation_count_token() {
        let diags = diagnostics_for("Guidance references IT Governance+3ISO+3 among others.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn ignores_lowercase_version_string() {
        let diags = diagnostics_for("This release requires v2+1 of the client SDK or newer.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn skips_code_fence() {
        let diags = diagnostics_for(
            "Body text.\n```\n:contentReference[oaicite:9]\n```\nMore body text follows.\n",
        );
        assert!(diags.is_empty());
    }
}
