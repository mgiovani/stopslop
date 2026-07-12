use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose::ProseDoc;
use crate::prose_words::{NEGATIVE_PARALLELISM, RULE_OF_THREE, TRAILING_PARTICIPLE};
use crate::registry::RuleDef;
use regex::Regex;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP017",
    name: "Rhetorical parallelism / false-depth scaffolding density",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: false,
    path_gated: false,
    check,
};

/// Counts matches of the shared RULE_OF_THREE (a), NEGATIVE_PARALLELISM (b), and
/// TRAILING_PARTICIPLE (c) sub-patterns from `prose_words` over the masked prose stream. Each
/// device is legitimate once; the tell is the shape recurring across a document, so this is a
/// density rule, not a per-match one. Scope: skip headings/frontmatter/URLs (code is already
/// masked).
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };

    let (a, a_first) = count_scoped(doc, &RULE_OF_THREE);
    let (b, b_first) = count_scoped(doc, &NEGATIVE_PARALLELISM);
    let (c, c_first) = count_scoped(doc, &TRAILING_PARTICIPLE);

    let t = a + b + c;
    let s = b + c;

    // s>=2 and a>=3 are the only two independently reachable trigger conditions. A prior
    // `t >= t_floor || ... || b >= 3 || c >= 3` formulation was dead weight: t_floor is always
    // >= 4 (`max(4, ...)`), and t = a + s, so t >= t_floor while a <= 2 and s <= 1 would need
    // t <= 3 >= 4 -- impossible; i.e. t >= t_floor can only ever be true when a >= 3 or s >= 2
    // already is. Likewise b >= 3 implies s = b + c >= 3 >= 2, and c >= 3 implies the same. Every
    // one of those three disjuncts was unreachable except through a>=3/s>=2, so it's the same
    // truth table with three fewer, untestable-in-isolation branches. ponytail: deleted as dead
    // disjuncts (proof, not just missing test coverage).
    let flagged = s >= 2 || a >= 3;
    if !flagged {
        return;
    }

    let first_byte = [a_first, b_first, c_first]
        .into_iter()
        .flatten()
        .min()
        .unwrap();
    let (line, col) = doc.line_col(first_byte);
    out.push(Diagnostic::at(
        rule,
        ctx,
        line,
        col,
        format!("rhetorical parallelism / false-depth scaffolding density high ({t} occurrences)"),
    ));
}

/// Counts matches of `re` over `doc.masked`, skipping any in frontmatter/heading/URL spans.
/// Returns the count and the byte offset of the first counted match (if any).
fn count_scoped(doc: &ProseDoc<'_>, re: &Regex) -> (usize, Option<usize>) {
    let mut n = 0usize;
    let mut first = None;
    for m in re.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_heading(byte) || doc.in_url(byte) {
            continue;
        }
        n += 1;
        if first.is_none() {
            first = Some(byte);
        }
    }
    (n, first)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        let doc = ProseDoc::parse(src);
        let ctx = LintContext {
            display_path: "test.md".to_string(),
            source: src,
            tree: None,
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
    fn flags_two_strong_subsignals() {
        // S = b + c >= 2: one negative-parallelism + one trailing participle.
        let src = "The API is not only fast but also simple to use, and it favors clarity, underscoring its focus on developer experience.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
    }

    #[test]
    fn flags_three_rule_of_three_lists_alone() {
        // a=3, s=0: three independent rule-of-three lists, no negative-parallelism or trailing
        // participle anywhere -- isolates the a>=3 disjunct from s>=2.
        let src = "The plan is clear, simple, and complete.\n\nThe team is fast, careful, and thorough.\n\nThe result is neat, clean, and correct.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
    }

    #[test]
    fn clean_single_rule_of_three_and_factual_gerund() {
        // T=1 (one rule-of-three), S=0 (the gerund isn't in the evaluative participle list).
        let src = "The style guide asks for code that is clear, concise, and correct, running on port 8080 by default.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn skips_headings() {
        let src = "# Fast, Simple, and Reliable\n\nBody text with nothing special going on here at all.\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
