use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, PROSE_LANGS};
use crate::prose::ProseDoc;
use crate::registry::RuleDef;
use std::collections::HashMap;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP018",
    name: "Mid-prose em/en dash",
    tier: Tier::B,
    langs: PROSE_LANGS,
    natlangs: lang::ALL_NATLANGS,
    default_on: true,
    path_gated: false,
    check,
};

/// The concrete replacement hint attached to every finding this rule emits, regardless of which
/// of the three dash forms below tripped it.
const FIX: &str = "rewrite the sentence, or use a comma, colon, or parentheses";

/// Flags every U+2014 (em dash), U+2013 (en dash), and spaced ASCII double hyphen (` -- `) in the
/// masked prose stream that isn't block-initial -- both the tight form (word--word, for the
/// em/en-dash characters) and the spaced form (word -- word) count. Frontmatter is skipped
/// (metadata, not prose); headings are in scope. Code is already blanked in `doc.masked`, so a
/// dash inside a fence or inline span never reaches this scan. `---` thematic-break/frontmatter
/// fence lines are bare hyphen runs with no interior space, so they never match U+2014/U+2013 (not
/// the same character at all) and never match the SPACED ascii form either (see
/// `spaced_ascii_dashes` below for why a longer hyphen run can't produce a false ` -- ` match). The
/// en dash gets one extra exemption: a numeric range (`2020--2024`, `120--140ms`) is legitimate
/// typography, not a rewritten em dash, so an en dash flanked on BOTH sides by an ASCII digit is
/// silently allowed. The one allowed dash form (all three characters/forms alike) is the
/// attribution/quote convention -- a dash that opens a *block* (after optional
/// whitespace/blockquote `>` markers), as in `-- Oscar Wilde` -- since that's a typographic
/// convention, not mid-sentence punctuation. A dash opening a line that merely continues the
/// preceding one (wrapped prose, list-item continuation) is ordinary punctuation that happened to
/// land at a wrap point, so it is flagged. Each qualifying occurrence gets its own diagnostic (not
/// deduped per line): a line with two mid-prose dashes is two problems to fix, not one.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let mut block_initial = HashMap::new();

    for (byte, ch) in doc.masked.char_indices() {
        let is_en_dash_numeric_range = ch == '\u{2013}' && digit_flanked(&doc.masked, byte, ch);
        if !matches!(ch, '\u{2014}' | '\u{2013}') || is_en_dash_numeric_range {
            continue;
        }
        if doc.in_frontmatter(byte) || is_block_initial(doc, byte, &mut block_initial) {
            continue;
        }
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            "em/en dash in prose; rewrite the sentence",
            FIX,
        ));
    }

    for byte in spaced_ascii_dashes(&doc.masked) {
        if doc.in_frontmatter(byte) || is_block_initial(doc, byte, &mut block_initial) {
            continue;
        }
        let (line, col) = doc.line_col(byte);
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            "em/en dash in prose; rewrite the sentence",
            FIX,
        ));
    }
}

/// True if the character immediately before and immediately after the char at `byte` (whose
/// encoded length is `ch.len_utf8()`) are both ASCII digits -- the numeric-range exemption for the
/// en dash.
fn digit_flanked(masked: &str, byte: usize, ch: char) -> bool {
    let before = masked[..byte].chars().next_back();
    let after = masked[byte + ch.len_utf8()..].chars().next();
    matches!(before, Some(c) if c.is_ascii_digit())
        && matches!(after, Some(c) if c.is_ascii_digit())
}

/// Byte offsets (pointing at the first of the two hyphens) of every SPACED ascii double hyphen
/// (literal ` -- `, i.e. space-hyphen-hyphen-space) in `masked`. A search over the literal 4-byte
/// needle, advancing past just the two hyphens (not the full 4-byte match) each time, so a chained
/// run like `a -- -- b` still finds both dashes -- the naive non-overlapping `match_indices` would
/// consume the second dash's leading space along with the first match and miss it. A longer hyphen
/// run (`---`, `----`, a thematic break or frontmatter fence) never produces this exact 4-byte
/// needle: splitting a 3+-hyphen run into any two adjacent hyphens always leaves a THIRD hyphen
/// immediately before or after that pair, never a space, so the needle search can't land on it.
fn spaced_ascii_dashes(masked: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut search_from = 0usize;
    while search_from < masked.len() {
        let Some(rel) = masked[search_from..].find(" -- ") else {
            break;
        };
        let match_start = search_from + rel;
        let dash_byte = match_start + 1;
        out.push(dash_byte);
        search_from = dash_byte + 2; // past the two hyphens; the trailing space is reusable
    }
    out
}

/// True if this em dash opens a block: nothing but whitespace and blockquote `>` markers precede
/// it on its line, *and* the line itself starts a block: it is the document's first line (or the
/// first after frontmatter), it carries a `>` marker, or the line above is blank or itself a
/// blockquote line. Opening a line is not enough -- prose wraps, so a dash that merely landed at a
/// wrap point or on a list-item continuation line still needs flagging. One deliberate exception:
/// a `>`-marked line is always treated as block-initial, so a wrapped continuation *inside* a
/// blockquote stays exempt -- `> -- Author` under a quote is indistinguishable from attribution,
/// and attribution is the likelier reading. A second exception covers the travessão: Portuguese-
/// (and English-) language fiction repeats a line-opening dash on every line of a dialogue
/// exchange, the same shape as attribution, so a run of such lines is exempt down its whole
/// length -- see the walk-back below. `memo` caches the verdict per dash byte: `check` visits
/// dashes in byte order, so a run of N dialogue lines costs N single-step walks instead of the
/// N^2 a fresh walk per dash would (the quadratic shape `7d502e5` removed from this rule).
fn is_block_initial(doc: &ProseDoc, byte: usize, memo: &mut HashMap<usize, bool>) -> bool {
    // An HTML dash that opens its `<p>`/`<td>` is the same attribution shape with the tag blanked.
    let verdict = doc.block_initial(byte) || walk_back(doc, byte, memo);
    memo.insert(byte, verdict);
    verdict
}

fn walk_back(doc: &ProseDoc, byte: usize, memo: &HashMap<usize, bool>) -> bool {
    let masked = &doc.masked;
    let mut byte = byte;
    loop {
        let line_start = doc.line_span(byte).0;
        let prefix = &masked[line_start..byte];
        if !prefix
            .trim_start_matches(|c: char| c.is_whitespace() || c == '>')
            .is_empty()
        {
            return false;
        }
        if prefix.contains('>') {
            return true; // attribution inside a blockquote
        }
        let Some(before) = masked[..line_start].strip_suffix('\n') else {
            return true; // first line of the document
        };
        let prev_start = before.rfind('\n').map_or(0, |i| i + 1);
        if doc.in_frontmatter(prev_start) {
            return true; // first prose line after frontmatter -- the body's real first line
        }
        let prev_line = &before[prev_start..];
        let prev = prev_line.trim();
        // Blank line above => this dash opens a block. `>` line above => this is the lazy
        // continuation of a blockquote, where an unmarked `-- Author` line is the same
        // attribution convention.
        if prev.is_empty() || prev.starts_with('>') {
            return true;
        }
        // A previous line that itself opens with a dash extends the exemption down the run.
        // Walk back onto that dash and retest it in a loop, not recursion (see
        // context::walk_tree), so a non-block-initial dash can't launder the line below it.
        match dash_start_byte(prev_line, prev_start) {
            Some(prev_dash_byte) => match memo.get(&prev_dash_byte) {
                Some(&decided) => return decided,
                None => byte = prev_dash_byte,
            },
            None => return false,
        }
    }
}

/// If `line` (the raw, untrimmed content of one line, starting at absolute offset `line_offset`
/// in `doc.masked`) opens -- after optional whitespace and blockquote `>` markers -- with an em
/// dash, en dash, or the two-hyphen ASCII form, returns that dash's absolute byte offset.
fn dash_start_byte(line: &str, line_offset: usize) -> Option<usize> {
    let stripped = line.trim_start_matches(|c: char| c.is_whitespace() || c == '>');
    let lead = line.len() - stripped.len();
    if stripped.starts_with('\u{2014}')
        || stripped.starts_with('\u{2013}')
        || stripped.starts_with("--")
    {
        Some(line_offset + lead)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;
    use crate::prose::ProseDoc;

    fn diagnostics_for(src: &str) -> Vec<Diagnostic> {
        diagnostics_in(ProseDoc::parse(src), src, Lang::Md)
    }

    fn diagnostics_for_html(src: &str) -> Vec<Diagnostic> {
        diagnostics_in(ProseDoc::parse_html(src), src, Lang::Html)
    }

    fn diagnostics_in<'a>(doc: ProseDoc<'a>, src: &'a str, lang: Lang) -> Vec<Diagnostic> {
        let ctx = LintContext {
            display_path: "test.md".to_string(),
            source: src,
            index: None,
            lang,
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
    fn html_block_opening_dash_is_attribution() {
        assert!(diagnostics_for_html("<p>Quote.</p>\n<p>\u{2014} Author</p>\n").is_empty());
        assert_eq!(diagnostics_for_html("<p>a \u{2014} b</p>\n").len(), 1);
    }

    #[test]
    fn flags_tight_mid_sentence_dash() {
        let diags = diagnostics_for("The build pipeline\u{2014}stays simple across releases.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP018");
    }

    #[test]
    fn flags_spaced_mid_sentence_dash() {
        let diags = diagnostics_for("Deploys went smoothly \u{2014} no surprises this week.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_dash_in_heading() {
        let diags = diagnostics_for("# Release Notes \u{2014} Now Faster\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_single_occurrence_no_density_floor() {
        // Old rule needed >= 3 dashes at a density above 1%; the new rule fires on one.
        let diags = diagnostics_for(
            "This document is otherwise ordinary and reasonably long, but one clause here\u{2014}right here\u{2014}breaks the flow.\n",
        );
        assert!(!diags.is_empty());
    }

    #[test]
    fn allows_line_initial_attribution() {
        let diags = diagnostics_for("\u{2014} Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_blockquote_attribution() {
        let diags = diagnostics_for("> \u{2014} Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_attribution_after_blank_line() {
        let diags = diagnostics_for("> A quoted line of prose.\n\n\u{2014} Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_attribution_on_blockquote_lazy_continuation_line() {
        // CommonMark lazy continuation: the unmarked line still belongs to the blockquote, and
        // `-- Author` directly under a quote is the attribution convention, not mid-prose dash.
        let diags = diagnostics_for("> A quoted line of prose.\n\u{2014} Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_attribution_on_first_body_line_after_frontmatter() {
        // The closing `---` is not a blank line, but the line after it still opens the body.
        let diags = diagnostics_for("---\ntitle: Report\n---\n\u{2014} Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_attribution_after_whitespace_only_blank_line() {
        let diags = diagnostics_for("Body text sits above the quote.\n   \n\u{2014} Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_dash_starting_a_wrapped_continuation_line() {
        // The original false negative: prose wraps, and a dash landing at the wrap point escaped.
        let diags = diagnostics_for("a \u{2014} b\n      \u{2014} c\n\u{2014} d\n");
        assert_eq!(diags.len(), 3);
        assert_eq!((diags[0].line, diags[0].col), (1, 3));
        assert_eq!((diags[1].line, diags[1].col), (2, 7));
        assert_eq!((diags[2].line, diags[2].col), (3, 1));
    }

    #[test]
    fn flags_dash_at_column_one_of_a_continuation_line() {
        let diags = diagnostics_for(
            "The release went out on schedule and the rollout was uneventful\n\u{2014} apart from one flaky test.\n",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!((diags[0].line, diags[0].col), (2, 1));
    }

    #[test]
    fn flags_dash_on_list_item_continuation_line() {
        let diags = diagnostics_for(
            "- The first bullet runs long enough to wrap onto a second line\n  \u{2014} which continues it.\n",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!((diags[0].line, diags[0].col), (2, 3));
    }

    #[test]
    fn allows_horizontal_rule_and_hyphens() {
        let src = "Body text.\n\n---\n\nMore body text, using a hyphen-range like 10-20 items.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn skips_frontmatter() {
        let src = "---\ntitle: Report \u{2014} Q3\n---\n\nBody text with no dash issue here.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn skips_dashes_inside_code_fence() {
        let src = "Body text.\n```\nword\u{2014}word \u{2014} word\n```\nMore body text here now today.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn skips_inline_code_dash() {
        let diags = diagnostics_for("Inline code stays silent: `a\u{2014}b`.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn each_occurrence_on_a_line_gets_its_own_diagnostic() {
        let diags = diagnostics_for("One\u{2014}two\u{2014}three mid-sentence dashes here.\n");
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn diagnostic_carries_a_fix_hint() {
        let diags = diagnostics_for("Deploys went smoothly \u{2014} no surprises this week.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("rewrite the sentence, or use a comma, colon, or parentheses")
        );
    }

    #[test]
    fn flags_mid_sentence_en_dash() {
        let diags = diagnostics_for("The plan changed \u{2013} nobody expected that.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP018");
    }

    #[test]
    fn allows_en_dash_numeric_range() {
        let src = "Latency dropped from 120\u{2013}140ms to 95\u{2013}110ms this week.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_en_dash_with_one_digit_and_one_word_neighbor() {
        // Only ONE side is a digit, so the numeric-range exemption doesn't apply.
        let diags = diagnostics_for("See page 5\u{2013}onward for the rest of the appendix.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn allows_en_dash_block_initial_attribution() {
        let diags = diagnostics_for("\u{2013} Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn flags_spaced_ascii_double_hyphen() {
        let diags = diagnostics_for("The rollout went fine -- nobody expected any issues.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP018");
    }

    #[test]
    fn flags_two_chained_ascii_double_hyphens() {
        let diags =
            diagnostics_for("The rollout went fine -- truly fine -- across every region.\n");
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn allows_ascii_double_hyphen_block_initial_attribution() {
        let diags = diagnostics_for("-- Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_ascii_double_hyphen_blockquote_attribution() {
        let diags = diagnostics_for("> -- Oscar Wilde\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn allows_thematic_break_and_hyphen_range() {
        let src = "Body text.\n\n---\n\nMore body text, using a hyphen range like 10-20 items.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn allows_longer_hyphen_rule_surrounded_by_spaces() {
        // A longer hyphen run flanked by spaces must not be mistaken for the exact two-hyphen
        // spaced-dash needle: splitting any 3+ run always leaves a third hyphen adjacent to any
        // two-hyphen slice, so no ` -- ` substring can ever be found inside it.
        let src = "Section divider below.\n\n ---- \n\nMore body text follows here today.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn allows_dialogue_run_but_flags_its_mid_line_dash() {
        // A pt-BR travessão dialogue exchange: every line opens with the same attribution-style
        // dash, so only the mid-line dash (the narrator's "— respondeu ela" aside) is prose.
        let src = "\u{2014} Voc\u{ea} viu?\n\u{2014} Vi. \u{2014} respondeu ela.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!((diags[0].line, diags[0].col), (2, 7));
    }

    #[test]
    fn allows_dialogue_runs_of_any_length_in_any_dash_form() {
        assert!(diagnostics_for("\u{2014} Oi.\n\u{2014} Tudo bem?\n\u{2014} Tudo.\n").is_empty());
        // An ASCII `--` opener above extends the exemption to the em dash below it.
        assert!(diagnostics_for("-- Oi.\n\u{2014} Tudo bem?\n").is_empty());
        assert!(diagnostics_for("> \u{2014} Oi.\n> \u{2014} Tudo bem?\n").is_empty());
    }

    #[test]
    fn wrapped_dash_line_does_not_launder_the_dash_line_below_it() {
        // Line 2's dash is a wrap point (line 1 is prose), so line 3 inherits nothing from it.
        let src = "Prose that wraps right here\n\u{2014} continued\n\u{2014} and again\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 2);
        assert_eq!((diags[0].line, diags[1].line), (2, 3));
    }

    #[test]
    fn flags_wrapped_dash_line_under_a_non_dash_opening_line() {
        // A line that starts with a dash purely because prose wrapped at that point (the line
        // above it does NOT itself open with a dash) is still ordinary mid-prose punctuation.
        let src = "The rollout finished cleanly and every check passed without issue at all\n\u{2014} except for one flaky integration test.\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!((diags[0].line, diags[0].col), (2, 1));
    }
}
