use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose::ProseDoc;
use crate::prose_words::{CLAUSE_MARKER, NEGATIVE_PARALLELISM, RULE_OF_THREE, TRAILING_PARTICIPLE};
use crate::registry::RuleDef;
use regex::Regex;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP017",
    name: "Rhetorical parallelism / false-depth scaffolding density",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: true,
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

    let (a, a_first) = count_scoped_filtered(doc, &RULE_OF_THREE, is_rhetorical_tricolon);
    let (b, b_first) = count_scoped(doc, &NEGATIVE_PARALLELISM);
    let (c, c_first) = count_scoped(doc, &TRAILING_PARTICIPLE);

    let t = a + b + c;
    let s = b + c;

    // Replaces `t>=t_floor || b>=3 || c>=3`: t_floor>=4 and t=a+s make those disjuncts provably
    // unreachable unless a>=3 or s>=2 already holds, so removal is proven dead code, not missing
    // coverage.
    let flagged = s >= 2 || a >= 3;
    if !flagged {
        return;
    }

    // Anchor and hint follow whichever signal fired: the old min-byte anchor plus always-print
    // participial hint could point a tricolon run at an enumeration with a nonexistent
    // participle to cut.
    let (first_byte, fix) = if s >= 2 {
        (
            [b_first, c_first].into_iter().flatten().min().unwrap(),
            "cut the participial tail or make it its own sentence",
        )
    } else {
        (
            a_first.unwrap(),
            "three-item lists used as rhetorical scaffolding; drop one or write them out",
        )
    };
    let (line, col) = doc.line_col(first_byte);
    out.push(Diagnostic::at_fix(
        rule,
        ctx,
        line,
        col,
        format!("rhetorical parallelism / false-depth scaffolding density high ({t} occurrences)"),
        fix,
    ));
}

/// Counts matches of `re` over `doc.masked`, skipping any in frontmatter/heading/URL spans.
/// Returns the count and the byte offset of the first counted match (if any).
fn count_scoped(doc: &ProseDoc<'_>, re: &Regex) -> (usize, Option<usize>) {
    count_scoped_filtered(doc, re, |_, _| true)
}

/// `count_scoped` with an extra per-match predicate, given `(masked, match_start..match_end)`.
fn count_scoped_filtered(
    doc: &ProseDoc<'_>,
    re: &Regex,
    keep: impl Fn(&str, std::ops::Range<usize>) -> bool,
) -> (usize, Option<usize>) {
    let mut n = 0usize;
    let mut first = None;
    for m in re.find_iter(&doc.masked) {
        let byte = m.start();
        if doc.in_frontmatter(byte) || doc.in_heading(byte) || doc.in_url(byte) {
            continue;
        }
        if !keep(&doc.masked, m.range()) {
            continue;
        }
        n += 1;
        if first.is_none() {
            first = Some(byte);
        }
    }
    (n, first)
}

/// True when a RULE_OF_THREE match is a genuine rhetorical tricolon rather than the tail of an
/// ordinary enumeration.
///
/// `RULE_OF_THREE` matches any comma series of three-or-more items, including the *tail* of a
/// longer one -- which is why its reported column used to land mid-list ("...race, caste,
/// >>>color, religion, or sexual<<<"). Two gates, both cheap:
///
/// 1. **Exactly three items.** A rule of three is three; a five- or seven-item list is just a
///    list. The regex matches a longer list's tail, and a tail is always immediately preceded by
///    the previous item's comma -- so rejecting a match that a comma runs straight into drops the
///    Contributor Covenant's protected-characteristics enumeration, a list of agent product
///    names, and "lint, test, build, security scan, and deploy", while keeping "clear, concise,
///    and correct" and "solo developers, growing startups, or established enterprises". Done by
///    looking backwards, since the `regex` crate has no lookbehind.
///
///    ponytail: known false negative -- a tricolon behind a leading adverbial ("In practice, it
///    is clear, concise, and correct") reads as a tail and is skipped. Acceptable on a density
///    rule that needs three matches to fire; tighten only if it shows up in practice.
///
/// 2. **Items are phrases, not clauses.** An item carrying a finite verb or subject pronoun
///    (`prose_words::CLAUSE_MARKER`) makes this a compound sentence, not an enumeration:
///    "...findings from one rule, it absorbs exactly three, and a fourth is reported" is three
///    clauses joined by commas.
///
/// 3. **Not a proper-noun list.** Two or more of the three items *beginning* with a capitalized
///    word is a list of names ("GitHub, GitLab, and Bitbucket"), not rhetoric. Counting items
///    rather than tokens keeps acronyms mid-item ("the API is clear, concise, and correct") and
///    sentence-initial capitals ("Clear, concise, and correct prose wins") from tripping it.
fn is_rhetorical_tricolon(masked: &str, range: std::ops::Range<usize>) -> bool {
    // `\w` excludes `-`/`/`, so a match can start mid-token ("weak-verb" matches at "verb");
    // walk back over the rest of the token before checking for the list-tail comma.
    let before = masked[..range.start]
        .trim_end_matches(|c: char| c.is_alphanumeric() || c == '-' || c == '/' || c == '_');
    if before.trim_end().ends_with(',') {
        return false;
    }

    let items: Vec<&str> = masked[range].splitn(3, ',').collect();

    // Only the middle item is checked: it alone is comma-bounded on both sides. The outer two
    // spill into the surrounding sentence (the regex's 4-word run), so judging them would
    // reject ordinary enumerations.
    if items
        .get(1)
        .is_some_and(|item| CLAUSE_MARKER.is_match(item))
    {
        return false;
    }

    let capitalized_items = items
        .iter()
        .filter(|item| {
            item.split_whitespace()
                .find(|w| !matches!(*w, "and" | "or"))
                .and_then(|w| w.chars().next())
                .is_some_and(char::is_uppercase)
        })
        .count();
    capitalized_items < 2
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Real strings from a 130-document corpus that the ungated rule-of-three counted. None is
    /// rhetorical; each is an ordinary enumeration or a list of names.
    #[test]
    fn plain_enumerations_do_not_count_as_tricolons() {
        for src in [
            "We do not tolerate discrimination on the basis of nationality, personal appearance, race, caste, color, religion, or sexual orientation.\nThe policy covers age, body size, visible disability, ethnicity, and level of experience.\nIt also covers education, socio-economic status, gender identity, and expression.\n",
            "It works with Claude Code, Codex, Cursor, OpenCode, Gemini CLI, and other agents.\nIt ships configs for GitHub Actions, GitLab CI, CircleCI, Jenkins, and Buildkite.\nIt reads pyproject.toml, package.json, Cargo.toml, go.mod, and Gemfile.\n",
            "The pipeline defines stages for lint, test, build, security scan, and deploy.\nEach stage reports status, duration, artifacts, cache hits, and exit codes.\nFailures capture logs, environment, git metadata, timing, and the command line.\n",
            "We support GitHub, GitLab, and Bitbucket.\nWe test on Linux, macOS, and Windows.\nWe target Python, Ruby, and Rust.\n",
        ] {
            assert!(
                diagnostics_for(src).is_empty(),
                "plain enumeration flagged: {src:?}"
            );
        }
    }

    /// `\w` excludes `-`, so the match starts at "verb" inside "weak-verb" and the byte right
    /// before it is `-`, not the comma that makes this a longer list's tail. Found in this
    /// repo's own README, which enumerates fourteen rule families in one sentence.
    #[test]
    fn hyphenated_token_does_not_hide_a_list_tail() {
        let src = "It covers hedging, overused vocabulary, boldface overuse, smart quotes, heading formatting, colon reveals, filler and adverb density, weak-verb phrasing, dramatic fragmentation, and mechanical uniformity.\nIt also covers alpha, beta, gamma, delta, epsilon, zeta-eta phrasing, theta density, and iota.\nAnd it covers one, two, three, four, five, six-seven forms, eight kinds, and nine.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// Commas joining clauses are not an enumeration. Found in this repo's own README.
    #[test]
    fn compound_sentences_are_not_tricolons() {
        let src = "If a file has three accepted findings from one rule, it absorbs exactly three, and a fourth is reported.\nWhen the budget runs out for that rule, we report the surplus, and the count ratchets down.\nOnce the cache is warm for a run, it reuses the parse, and a rescan is skipped.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    /// Only the middle item is judged: the outer two spill into the surrounding sentence, so
    /// judging them would reject ordinary enumerations that merely sit next to a verb.
    #[test]
    fn clause_words_outside_the_middle_item_do_not_reject() {
        let src = "The output should be clear, concise, and correct.\nA resolver can miss workspace dependencies, dynamic imports, and unusual build setups.\nIt must handle fences, placeholder credentials, and package imports.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
    }

    #[test]
    fn flags_three_abstract_tricolons() {
        let src = "The output should be clear, concise, and correct.\n\nThe architecture is scalable, maintainable, and robust.\n\nIt suits solo developers, growing startups, or established enterprises.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
    }

    /// The hint must describe the signal that fired: a tricolon run has no participial tail to cut.
    #[test]
    fn tricolon_run_gets_the_tricolon_hint() {
        let src = "The output should be clear, concise, and correct.\n\nThe architecture is scalable, maintainable, and robust.\n\nIt suits solo developers, growing startups, or established enterprises.\n";
        let diags = diagnostics_for(src);
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("three-item lists used as rhetorical scaffolding; drop one or write them out")
        );
    }

    /// An acronym mid-item and a sentence-initial capital are not a proper-noun list.
    #[test]
    fn acronyms_and_sentence_initial_capitals_still_count() {
        let src = "The API is clear, concise, and correct.\n\nClear, concise, and correct prose wins.\n\nThe CLI stays small, sharp, and predictable.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
    }

    #[test]
    fn flags_two_strong_subsignals() {
        // S = b + c >= 2: one negative-parallelism + one trailing participle.
        let src = "The API is not only fast but also simple to use, and it favors clarity, underscoring its focus on developer experience.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP017");
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("cut the participial tail or make it its own sentence")
        );
    }

    #[test]
    fn flags_two_new_trailing_participle_verbs() {
        // S = c = 2: two of the newly added trailing-participle verbs, no rule-of-three and no
        // negative parallelism anywhere, isolating the widened TRAILING_PARTICIPLE panel.
        let src = "The change reworks the retry path, ensuring every request lands exactly once. It also simplifies the config loader, resulting in a much smaller startup file.\n";
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
