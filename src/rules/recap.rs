use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::Lang;
use crate::prose::ProseDoc;
use crate::registry::RuleDef;
use regex::Regex;
use std::sync::LazyLock;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP029",
    name: "Summary-recap ending / fake-profound kicker",
    tier: Tier::B,
    langs: &[Lang::Md, Lang::Mdx, Lang::Txt, Lang::Rst],
    default_on: true,
    path_gated: false,
    check,
};

/// A final block that legitimately restates real content (as `clean_hedging.md` does: a long,
/// fact-dense closing paragraph that happens to start with "In conclusion") must not fire --
/// only a genuinely SHORT recap kicker is the AI-slop pattern this targets. Chosen with a
/// comfortable margin under real long-form conclusions (a plain restatement is usually a single
/// short sentence, well under this).
const RECAP_WORD_CAP: usize = 20;
/// Per spec: the fake-profound kicker sub-check only applies to a block this short or shorter.
const KICKER_WORD_CAP: usize = 12;

/// (a) anchored to the very start of the final block (case-insensitive).
static RECAP_OPENER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(in conclusion|ultimately|overall|in summary|to sum up|to summarize|all in all|the bottom line is|at the end of the day|in closing)\b",
    )
    .unwrap()
});

/// (b) opens with a conjunction/deictic mic-drop word.
static KICKER_OPENER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(and|because|that'?s)\b").unwrap());
/// (b) one of the stock mic-drop phrases, anywhere in the block.
static KICKER_PHRASE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)changes everything|the real question|that'?s the whole game|welcome to the future",
    )
    .unwrap()
});
/// (b) binary contrast: a short negative clause immediately followed by a short affirming one,
/// e.g. "The future isn't coming. It's already here."
static BINARY_CONTRAST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:isn'?t|is not|doesn'?t|does not|won'?t|will not|wasn'?t|weren'?t|can'?t|cannot|never)\b[^.!?\n]*[.!?]\s+(?:it'?s|it is|this is|that'?s|that is|they'?re|they are)\b[^.!?\n]*[.!?]$",
    )
    .unwrap()
});

/// (c) vague, generic positive-note ending: a stock upbeat closer with no concrete outcome
/// behind it, e.g. "The future looks bright." / "Watch this space." Scoped to the same short
/// final block as (b) (see `check`) since a long paragraph that happens to mention one of these
/// phrases mid-content is ordinary prose, not a vacuous kicker.
static POSITIVE_CONCLUSION_PHRASE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)the future looks bright|exciting times (?:lie ahead|are ahead)|a step in the right direction|the possibilities are endless|only time will tell|one thing is clear|the sky'?s the limit|watch this space",
    )
    .unwrap()
});

/// Final prose block: text (non-comment lines joined with spaces) plus the byte offset of its
/// first non-whitespace character, for the diagnostic anchor.
struct Block {
    text: String,
    first_byte: usize,
}

fn line_spans(masked: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = 0usize;
    for (i, b) in masked.bytes().enumerate() {
        if b == b'\n' {
            spans.push((start, i));
            start = i + 1;
        }
    }
    spans.push((start, masked.len()));
    spans
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// A line consisting only of `-`/`*`/`_` repeated (optionally spaced), e.g. `---`, `* * *`.
fn is_horizontal_rule(line: &str) -> bool {
    let stripped: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    stripped.len() >= 3
        && (stripped.bytes().all(|b| b == b'-')
            || stripped.bytes().all(|b| b == b'*')
            || stripped.bytes().all(|b| b == b'_'))
}

static REF_DEF_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s{0,3}\[[^\]]+\]:\s*\S").unwrap());
static COMMENT_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*<!--.*-->\s*$").unwrap());
static HEADING_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s{0,3}#{1,6}\s+\S").unwrap());

/// Scans backward from the end of the document for the final real prose block, skipping
/// trailing code fences (already blanked to whitespace-only lines in `doc.masked`, so they
/// simply read as blank runs), link-reference-definition blocks, HTML-comment-only blocks,
/// horizontal-rule lines, and heading-only lines. Lines that are individually pure HTML
/// comments but sit inside an otherwise-real block (e.g. an `ai-slop-ignore` comment directly
/// above a paragraph, with no blank line between them) are dropped from the returned text rather
/// than causing the whole block to be skipped.
fn final_prose_block(doc: &ProseDoc) -> Option<Block> {
    let masked = &doc.masked;
    let spans = line_spans(masked);
    let mut idx = spans.len();
    loop {
        while idx > 0 && is_blank(&masked[spans[idx - 1].0..spans[idx - 1].1]) {
            idx -= 1;
        }
        if idx == 0 {
            return None;
        }
        let end = idx;
        let mut start = idx;
        while start > 0 && !is_blank(&masked[spans[start - 1].0..spans[start - 1].1]) {
            start -= 1;
        }
        let lines: Vec<&str> = (start..end)
            .map(|i| &masked[spans[i].0..spans[i].1])
            .collect();
        let all_hr = lines.len() == 1 && is_horizontal_rule(lines[0]);
        let all_heading = lines.len() == 1 && HEADING_LINE.is_match(lines[0]);
        let all_ref_def = lines.iter().all(|l| REF_DEF_LINE.is_match(l));
        let all_comment = lines.iter().any(|l| !is_blank(l))
            && lines
                .iter()
                .all(|l| is_blank(l) || COMMENT_LINE.is_match(l));
        let all_frontmatter = lines
            .iter()
            .enumerate()
            .all(|(i, _)| doc.in_frontmatter(spans[start + i].0));
        if all_hr || all_heading || all_ref_def || all_comment || all_frontmatter {
            idx = start;
            continue;
        }

        let mut text = String::new();
        let mut first_byte = None;
        for (i, line) in lines.iter().enumerate() {
            if is_blank(line) || COMMENT_LINE.is_match(line) {
                continue;
            }
            if first_byte.is_none() {
                let offset = line.len() - line.trim_start().len();
                first_byte = Some(spans[start + i].0 + offset);
            }
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(line.trim());
        }
        let Some(first_byte) = first_byte else {
            idx = start;
            continue;
        };
        return Some(Block { text, first_byte });
    }
}

/// Position-gated to the FINAL prose block only (see `final_prose_block`). (a) fires when that
/// block both opens with a recap phrase and stays under `RECAP_WORD_CAP` words (a real,
/// fact-dense closing paragraph that happens to start with "In conclusion" -- see
/// `clean_hedging.md` -- must not fire; a short vacuous restatement is the actual target). (b)
/// fires when the block is at most `KICKER_WORD_CAP` words AND matches one of three narrow
/// mic-drop shapes. (c) fires on that same word-count cap when the block matches one of the
/// stock vague-positive-conclusion phrases instead (distinct wording from (b)'s mic-drop set, so
/// the two never compete for the same match). At most one diagnostic per file.
fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let Some(doc) = ctx.prose else { return };
    let Some(block) = final_prose_block(doc) else {
        return;
    };
    if block.text.is_empty() {
        return;
    }

    let words = block.text.split_whitespace().count();

    if let Some(m) = RECAP_OPENER.find(&block.text) {
        if words <= RECAP_WORD_CAP {
            let (line, col) = doc.line_col(block.first_byte);
            out.push(Diagnostic::at(
                rule,
                ctx,
                line,
                col,
                format!(
                    "summary-recap ending (\"{}\"); let the content stand without restating it",
                    m.as_str()
                ),
            ));
            return;
        }
    }

    if words <= KICKER_WORD_CAP
        && (BINARY_CONTRAST.is_match(block.text.trim())
            || KICKER_OPENER.is_match(&block.text)
            || KICKER_PHRASE.is_match(&block.text))
    {
        let (line, col) = doc.line_col(block.first_byte);
        out.push(Diagnostic::at(
            rule,
            ctx,
            line,
            col,
            "fake-profound kicker ending; cut the mic-drop line",
        ));
        return;
    }

    if words <= KICKER_WORD_CAP && POSITIVE_CONCLUSION_PHRASE.is_match(&block.text) {
        let (line, col) = doc.line_col(block.first_byte);
        out.push(Diagnostic::at_fix(
            rule,
            ctx,
            line,
            col,
            "vague, generic positive-note ending; say the concrete outcome instead",
            "end on the last concrete fact",
        ));
    }
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

    #[test]
    fn flags_short_recap_opener() {
        let diags =
            diagnostics_for("Ultimately, this update saves the team real time every day.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP029");
    }

    #[test]
    fn clean_long_factual_conclusion_not_flagged() {
        // Mirrors a real fixture risk: a genuine, fact-dense closing paragraph that happens to
        // start with a recap phrase must not fire just because of the opener.
        let src = "In conclusion, the migration met its latency and reliability goals, and the tooling built along the way should make the next infrastructure change easier to test and roll out safely.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_recap_phrase_outside_final_block_is_ignored() {
        // Position-gated: "Overall" opens a non-final paragraph, so it must not fire even
        // though it's a recap-opener phrase and short.
        let src = "Overall, the rollout went fine this week.\n\nThe team also fixed a minor dashboard bug unrelated to the rollout itself.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn flags_binary_contrast_kicker() {
        let diags = diagnostics_for("The future isn't coming. It's already here.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP029");
    }

    #[test]
    fn flags_thats_opener_kicker() {
        let diags = diagnostics_for("That's the whole game.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_listed_kicker_phrase() {
        let diags = diagnostics_for("This changes everything.\n");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn clean_short_factual_final_line_not_a_kicker() {
        // Short (<=12 words) but matches none of the narrow kicker shapes: a legitimate short
        // concluding sentence with a concrete fact must not fire.
        let src = "The service ships as version 2.4 starting next Tuesday.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn skips_trailing_comment_and_horizontal_rule_to_find_final_block() {
        let src = "Some ordinary opening paragraph goes here to start the document.\n\nThat's the whole game.\n\n---\n\n<!-- a trailing note -->\n";
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP029");
    }

    #[test]
    fn skips_trailing_link_reference_block() {
        let src = "In conclusion, ship it now.\n\n[ref]: https://example.com\n"; // ai-slop-ignore
        let diags = diagnostics_for(src);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn flags_vague_positive_conclusion() {
        let diags = diagnostics_for("The future looks bright for the whole team.\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "SLOP029");
        assert_eq!(
            diags[0].fix.as_deref(),
            Some("end on the last concrete fact")
        );
    }

    #[test]
    fn flags_each_vague_positive_conclusion_phrase() {
        let cases = [
            "Exciting times lie ahead for the platform team.\n",
            "This rollout was a step in the right direction.\n",
            "From here, the possibilities are endless.\n",
            "Only time will tell how this plays out.\n",
            "One thing is clear: the team shipped on time.\n",
            "For this project, the sky's the limit.\n",
            "Watch this space for what comes next.\n",
        ];
        for src in cases {
            let diags = diagnostics_for(src);
            assert_eq!(diags.len(), 1, "expected exactly one finding for: {src}");
            assert_eq!(diags[0].code, "SLOP029");
        }
    }

    #[test]
    fn clean_positive_conclusion_phrase_outside_final_block_is_ignored() {
        let src = "The future looks bright for the whole team this quarter.\n\nThe team also fixed a minor dashboard bug unrelated to that.\n";
        assert!(diagnostics_for(src).is_empty());
    }

    #[test]
    fn clean_long_paragraph_mentioning_watch_this_space_mid_content() {
        // The phrase appears, but the final block is well over KICKER_WORD_CAP words, so this is
        // ordinary prose referencing the phrase, not the short vacuous-kicker shape.
        let src = "Watch this space is a phrase the team explicitly banned from release notes after an old announcement used it without ever following up with a real update for the next six months.\n";
        assert!(diagnostics_for(src).is_empty());
    }
}
