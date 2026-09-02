use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose::first_byte_per_line;
use crate::registry::RuleDef;
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP025",
    name: "Unsourced weasel attribution",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: true,
    path_gated: false,
    check,
};

/// Anonymous-authority attribution phrases: an appeal to an unnamed "expert"/"study"/"critic"
/// standing in for a real, checkable source.
static WEASEL_ATTRIBUTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(experts (?:agree|say)|studies (?:show|suggest)|research (?:shows|suggests|indicates)|industry reports suggest|many (?:argue|believe)|some (?:say|argue)|it is widely regarded as|widely considered|widely regarded as|it is believed that|critics argue|analysts predict|reports indicate|sources say|it is often said|observers have (?:cited|noted)|several sources|several publications|many have (?:argued|noted|suggested)|it has been (?:suggested|argued|noted)|commentators (?:say|note|argue)|proponents (?:argue|say)|detractors (?:argue|say))\b")
        .unwrap()
});

/// Notability by name-dropping: three or more bare, comma-chained capitalized outlet/publication
/// names right after "cited/covered/featured in/by", with no per-citation context (no link, no
/// date, no quote -- just a list of names standing in for real sourcing). An optional "and "
/// before the final name covers the ordinary English list form ("X, Y, and Z"), not just a bare
/// comma splice.
static NOTABILITY_NAME_DROP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(?:cited|covered|featured) (?:in|by) [A-Z][\w& ]+(?:, (?:and )?[A-Z][\w& ]+){2,}",
    )
    .unwrap()
});

/// A citation on the SAME LINE as a weasel phrase means the claim is actually sourced, not
/// unsourced: a markdown link `[text](target)`, a bare URL, a footnote reference (`[^1]`), a
/// reST-style citation/footnote reference (`[label]_`), or a parenthetical with a year
/// ("(Smith 2023)").
static LINE_CITATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\[[^\]\n]*\]\([^)\n]*\)|https?://\S+|\[\^[^\]\n]+\]|\[[^\]\n]+\]_|\([^()\n]*\b(?:19|20)\d{2}\b[^()\n]*\)",
    )
    .unwrap()
});

/// Scans the masked prose stream for unsourced weasel attribution and notability name-dropping
/// (headings in scope, frontmatter and URL spans skipped). Any match whose LINE also carries a
/// citation signal is suppressed -- the rule targets bare appeals to unnamed/uncontextualized
/// authority, not attributed claims. One diagnostic per matching line, anchored at the first
/// (leftmost) match; the message and fix differ by which sub-pattern produced that match.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    let cited_lines: HashSet<usize> = LINE_CITATION_RE
        .find_iter(&doc.masked)
        .map(|m| doc.line_col(m.start()).0)
        .collect();

    let attribution: HashSet<usize> = WEASEL_ATTRIBUTION_RE
        .find_iter(&doc.masked)
        .map(|m| m.start())
        .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte))
        .collect();
    let name_drop: HashSet<usize> = NOTABILITY_NAME_DROP_RE
        .find_iter(&doc.masked)
        .map(|m| m.start())
        .filter(|&byte| !doc.in_frontmatter(byte) && !doc.in_url(byte))
        .collect();

    let bytes = attribution
        .iter()
        .chain(name_drop.iter())
        .copied()
        .filter(|&byte| !cited_lines.contains(&doc.line_col(byte).0));
    let by_line = first_byte_per_line(doc, bytes);
    for &byte in by_line.values() {
        let (line, col) = doc.line_col(byte);
        let (message, fix) = if attribution.contains(&byte) {
            (
                "unsourced weasel attribution; name the source or cite it",
                "name the source, or cut the claim",
            )
        } else {
            (
                "notability by name-dropping outlets with no per-citation context",
                "cite each claim individually, or cut the list",
            )
        };
        out.push(Diagnostic::at_fix(rule, ctx, line, col, message, fix));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    #[test]
    fn flags_experts_agree() {
        let src = "Experts agree that the migration reduced downtime.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP025");
    }

    #[test]
    fn flags_critics_argue_and_reports_indicate() {
        let src =
            "Critics argue the pricing change will hurt small teams.\n\nReports indicate a slowdown in adoption.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.code == "SLOP025"));
    }

    #[test]
    fn diagnostic_carries_a_fix_hint() {
        let diags = diagnostics_for("Experts agree that the migration reduced downtime.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("name the source, or cut the claim")
        );
    }

    #[test]
    fn flags_new_weasel_attribution_markers() {
        let cases = [
            "Observers have noted a slowdown in release cadence.\n",
            "Several sources describe the outage as preventable.\n",
            "Several publications covered the pricing backlash.\n",
            "Many have argued the migration was rushed.\n",
            "It has been suggested that the rollback was unnecessary.\n",
            "Commentators say the redesign missed the mark.\n",
            "Proponents argue the change reduces on-call load.\n",
            "Detractors argue the change adds needless complexity.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP025");
        }
    }

    #[test]
    fn clean_ordinary_prose() {
        let src = "Users reported a crash on startup, and the team shipped a fix the same day.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn silent_when_markdown_link_present() {
        let src = "Studies show a 40% drop in latency, per [the benchmark report](https://example.com/report).\n"; // ai-slop-ignore
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn silent_when_bare_url_present() {
        let src = "Research indicates a measurable improvement; see https://example.com/data for the full set.\n"; // ai-slop-ignore
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn silent_when_parenthetical_year_present() {
        let src = "Analysts predict slower growth next quarter (Chen 2024).\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn silent_when_footnote_reference_present() {
        let src = "Sources say the rollout was delayed[^1].\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_notability_name_drop() {
        let src = "The project was featured in The New York Times, The Guardian, and Wired.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP025");
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("cite each claim individually, or cut the list")
        );
    }

    #[test]
    fn flags_notability_name_drop_with_bare_comma_splice() {
        let src = "The tool was cited in TechCrunch, VentureBeat, Ars Technica.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP025");
    }

    #[test]
    fn clean_single_outlet_mention() {
        // One outlet, no chain of names -- an ordinary, checkable attribution.
        let src = "The report was featured in The New York Times.\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
