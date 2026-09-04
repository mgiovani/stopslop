//! SLOP045 — the code twin of SLOP041. SLOP041 measures how little a document's *sentences*
//! vary; this measures how little a source file's *lines and blocks* vary. Both read dispersion
//! through the same `coefficient_of_variation` helper, so the two never drift apart.
//!
//! Two whitespace studies put formatting uniformity at the top of the surface features that
//! separate machine-written from human-written code (Nirob et al. 2026, *Whitespaces Don't Lie*,
//! arXiv:2601.19264; Shi et al. 2024, *DetectCodeGPT*, arXiv:2401.06461). A model emits code
//! sampled from a distribution of already-formatted code, so its output tends toward one block
//! shape repeated: same-length functions, same-length lines, one blank line between each.
//!
//! Two signals, BOTH required:
//!   - line-length variation: stddev/mean of trimmed line lengths, in chars, over lines of at
//!     least `MIN_CONTENT_CHARS`.
//!   - block-length variation: stddev/mean of the lengths of runs of consecutive non-blank
//!     lines. This is the direct analogue of SLOP041's burstiness — SLOP041 varies sentence
//!     length, SLOP045 varies block length.
//!
//! # The ceiling, stated plainly
//!
//! Formatters make human code uniform too, and no threshold here can tell a disciplined author
//! from a model. What the measurement below shows is narrower: these two signals are what a
//! formatter does NOT move, and the thresholds are set where human code essentially never
//! reaches. Go is excluded outright because gofmt is not optional.
//!
//! The thresholds are fitted to the HUMAN side only. They bound the false-positive rate; they
//! say nothing yet about the true-positive rate, because no labelled machine corpus has been
//! run against them. That is why the rule opens at Tier C (advisory, always off by default).
//! Issue #39 is what replaces these numbers with measured ones and turns the rule on.

use crate::context::LintContext;
use crate::diagnostic::{Diagnostic, Tier};
use crate::lang::{self, Lang};
use crate::registry::RuleDef;
use crate::rules::comment_length::is_generated;
use crate::rules::uniformity::coefficient_of_variation;

pub static RULE: RuleDef = RuleDef {
    code: "SLOP045",
    name: "Mechanical uniformity (code formatting)",
    tier: Tier::C,
    // Go is absent on purpose: gofmt ships with the toolchain and is not optional, so every Go
    // file in the wild has already been through it and the signals below describe the tool.
    langs: &[Lang::Ts, Lang::Tsx, Lang::Python, Lang::Rust],
    natlangs: lang::ALL_NATLANGS,
    default_on: false,
    path_gated: true,
    check,
};

/// Evidence for every constant below, measured with this module's own implementation (not a
/// prototype) over four human-written corpora. "Eligible" means the file passed every floor.
///
/// | corpus                                   | scanned | eligible | fires |
/// |------------------------------------------|---------|----------|-------|
/// | crates.io Rust (`~/.cargo/registry/src`) | 9874    | 4222     | 1     |
/// | CPython `site-packages`                  | 534     | 352      | 0     |
/// | `node_modules` TypeScript (non-`.d.ts`)  | 778     | 450      | 0     |
/// | stopslop's own `src/`                    | 66      | 61       | 0     |
/// | **total**                                |         | **5085** | **1** |
///
/// 1 in 5085, or 0.02%. The one hit is a file of near-identical trait-impl forwarders, five
/// lines each: mechanically uniform boilerplate, which is what the rule says it found.
/// stopslop's own worst file sits at line-length variation 0.484 and block-length variation
/// 0.500, 1.6x and 2.0x the thresholds, so the dogfood run is quiet with margin, not by luck.
///
/// Quoting either marginal alone would mislead: line-length variation alone is under threshold
/// on 3.43% of eligible crates.io Rust files, and the two signals are anti-correlated there
/// (Pearson -0.33) while near-independent in Python (+0.02) and TypeScript (+0.04). The AND is
/// what makes the rule quiet; neither half is usable on its own.
///
/// These thresholds are fitted to the HUMAN side ONLY. They bound the false-positive rate and
/// say nothing yet about the true-positive rate, because no labelled machine corpus has been
/// run against them. That is exactly why the rule opens at Tier C. Issue #39 is the work that
/// replaces these numbers with measured ones and decides whether the rule turns on.
const MIN_NONBLANK_LINES: usize = 60;
/// Below five blocks the block-length statistic is a handful of samples and swings on one edit.
const MIN_BLOCKS: usize = 5;
/// The flat-file exclusion, and the floor that earns its keep most. A file that is one long
/// list at a single indent -- a module table, a re-export list, a const table -- is uniform by
/// nature, and such files are the entire low tail of both signals in human code. This crate's
/// own `src/rules/mod.rs` is exactly that shape: 45 lines, zero blank lines, line-length
/// variation 0.166, and hand-written.
const MIN_INDENT_DEPTHS: usize = 3;
/// Delimiter-only lines (`}`, `};`, `),`) measure brace density, not uniformity, and TypeScript
/// has far more of them than Python. Counting them gives TS a structural floor no single
/// cross-language threshold can clear: TS line-length variation p01 is 0.424 with them and
/// 0.322 without, against 0.211 for Rust and 0.342 for Python. Dropping them lines the three
/// languages up, which is what makes one threshold honest for all of them.
const MIN_CONTENT_CHARS: usize = 4;
/// Human p01 per corpus: 0.190 Rust, 0.342 Python, 0.322 TypeScript.
const LINE_LENGTH_CV_THRESHOLD: f64 = 0.30;
/// Human p01 per corpus: 0.347 Rust, 0.416 Python, 0.249 TypeScript. TypeScript sets this one,
/// and it is the tighter of the two constraints for that language.
const BLOCK_LENGTH_CV_THRESHOLD: f64 = 0.25;

/// Trimmed length in CHARS of every line long enough to carry content. Chars, not bytes: a line
/// of accented text is not twice as long as the same line in ASCII, and byte length would make
/// the statistic depend on the language the identifiers are written in.
fn content_line_lengths(lines: &[&str]) -> Vec<usize> {
    lines
        .iter()
        .map(|l| l.trim().chars().count())
        .filter(|&n| n >= MIN_CONTENT_CHARS)
        .collect()
}

/// Lengths of the runs of consecutive non-blank lines. A run of two or more blank lines is one
/// separator, not two empty blocks.
fn block_lengths(lines: &[&str]) -> Vec<usize> {
    let mut blocks = Vec::new();
    let mut run = 0usize;
    for line in lines {
        if line.trim().is_empty() {
            if run > 0 {
                blocks.push(run);
                run = 0;
            }
        } else {
            run += 1;
        }
    }
    if run > 0 {
        blocks.push(run);
    }
    blocks
}

/// How many distinct leading-whitespace widths the file uses. A tab counts as one, like any
/// other leading character: expanding it would need a tab width the file does not carry, and
/// this feeds a floor rather than a score, so the exact number never reaches a threshold.
fn indent_depths(lines: &[&str]) -> usize {
    let mut widths = std::collections::HashSet::new();
    for line in lines {
        if !line.trim().is_empty() {
            widths.insert(line.len() - line.trim_start().len());
        }
    }
    widths.len()
}

fn check(rule: &'static RuleDef, ctx: &LintContext, out: &mut Vec<Diagnostic>) {
    let lines: Vec<&str> = ctx.source.lines().collect();

    // Trailing whitespace is the one thing a formatter reliably erases, so a file that still
    // has any never met one -- and a model essentially never emits it. Its ABSENCE is worthless
    // (99.7% of crates.io Rust files, 100% of this crate) which is why it is a gate here and
    // not a fourth signal: as a signal it would be a free point on almost every file.
    if lines.iter().any(|l| l.trim_end().len() != l.len()) {
        return;
    }
    if lines.iter().filter(|l| !l.trim().is_empty()).count() < MIN_NONBLANK_LINES {
        return;
    }
    if indent_depths(&lines) < MIN_INDENT_DEPTHS {
        return;
    }
    if is_generated(ctx) {
        return;
    }

    let blocks = block_lengths(&lines);
    if blocks.len() < MIN_BLOCKS {
        return;
    }
    let (Some(line_cv), Some(block_cv)) = (
        coefficient_of_variation(&content_line_lengths(&lines)),
        coefficient_of_variation(&blocks),
    ) else {
        return;
    };

    if line_cv < LINE_LENGTH_CV_THRESHOLD && block_cv < BLOCK_LENGTH_CV_THRESHOLD {
        out.push(Diagnostic::at(
            rule,
            ctx,
            1,
            1,
            format!(
                "formatting uniformity: line-length variation {line_cv:.3} \
                 (< {LINE_LENGTH_CV_THRESHOLD}), block-length variation {block_cv:.3} \
                 (< {BLOCK_LENGTH_CV_THRESHOLD})"
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context;
    use tree_sitter::Parser;

    fn run(lang: Lang, src: &str) -> Vec<Diagnostic> {
        let mut p = Parser::new();
        p.set_language(&crate::lang::ts_language(lang)).unwrap();
        let tree = p.parse(src, None).unwrap();
        let (comments, strings, index) = context::extract(&tree, src, lang);
        let ctx = LintContext {
            display_path: "t".into(),
            source: src,
            index: Some(&index),
            lang,
            comments: &comments,
            strings: &strings,
            is_test_path: false,
            is_stub_file: false,
            deps: None,
            prose: None,
            natlangs: crate::lang::ALL_NATLANGS,
        };
        let mut out = Vec::new();
        check(&RULE, &ctx, &mut out);
        out
    }

    /// `blocks` blocks of the same six-line shape, one blank line apart. Both signals sit far
    /// under threshold by construction, so a test that expects silence is proving its own gate
    /// and not an accident of the body.
    fn uniform_rust(blocks: usize) -> String {
        (0..blocks)
            .map(|i| {
                format!(
                    "fn normalize_field{i:02}(raw: &str) -> String {{\n    \
                     let value = raw.trim().to_lowercase();\n    \
                     if value.is_empty() || value == \"-\" {{\n        \
                     return String::from(\"unnamed-field{i:02}\");\n    }}\n    \
                     value.replace(char::is_whitespace, \"-\")\n}}\n"
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn lines_of(src: &str) -> Vec<&str> {
        src.lines().collect()
    }

    #[test]
    fn content_line_lengths_ignores_delimiter_only_lines() {
        let src = "fn main() {\n    let x = 1;\n}\n";
        assert_eq!(content_line_lengths(&lines_of(src)), vec![11, 10]);
    }

    #[test]
    fn content_line_lengths_counts_chars_not_bytes() {
        // "ações" is 5 chars but 7 bytes; byte length would make the statistic depend on the
        // language the identifiers are written in.
        assert_eq!(content_line_lengths(&["  ações"]), vec![5]);
    }

    #[test]
    fn content_line_lengths_matches_a_hand_computed_cv() {
        // lengths 10, 20, 30 -> mean 20, population variance 200/3, stddev 8.165, cv 0.408
        let cv = coefficient_of_variation(&[10, 20, 30]).unwrap();
        assert!((cv - 0.4082483).abs() < 1e-6, "got {cv}");
    }

    #[test]
    fn block_lengths_split_on_blank_runs() {
        // A run of two blank lines is one separator, not two empty blocks.
        let src = "a\nb\nc\n\nd\n\n\ne\nf\n";
        assert_eq!(block_lengths(&lines_of(src)), vec![3, 1, 2]);
    }

    #[test]
    fn block_lengths_ignores_leading_and_trailing_blanks() {
        assert_eq!(block_lengths(&lines_of("\n\na\n\n")), vec![1]);
    }

    #[test]
    fn indent_depths_counts_distinct_leading_widths() {
        let src = "a\n  b\n    c\n  d\n\n";
        assert_eq!(indent_depths(&lines_of(src)), 3);
    }

    #[test]
    fn fires_once_at_line_one_and_names_both_values() {
        let out = run(Lang::Rust, &uniform_rust(12));
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].code, "SLOP045");
        assert_eq!((out[0].line, out[0].col), (1, 1));
        assert!(
            out[0].message.contains("line-length variation"),
            "{}",
            out[0].message
        );
        assert!(
            out[0].message.contains("block-length variation"),
            "{}",
            out[0].message
        );
    }

    #[test]
    fn one_signal_alone_stays_silent() {
        // Uniform block lengths, wildly varied line lengths: block-length variation trips and
        // line-length variation does not. This is the test that fails if anyone ever loosens
        // the AND into an either-or.
        let src: String = (0..14)
            .map(|i| {
                format!(
                    "fn f{i:02}() {{\n    let value = {};\n    println!(\"{{value}}\");\n}}\n\n",
                    "1 + ".repeat(i * 3) + "1"
                )
            })
            .collect();
        assert!(
            run(Lang::Rust, &src).is_empty(),
            "{:?}",
            run(Lang::Rust, &src)
        );
    }

    #[test]
    fn short_file_stays_silent_even_if_perfectly_uniform() {
        // Same body, below MIN_NONBLANK_LINES: 5 blocks x 6 lines = 30.
        assert!(run(Lang::Rust, &uniform_rust(5)).is_empty());
    }

    #[test]
    fn flat_file_stays_silent() {
        // A one-indent-depth list -- a module table, a re-export list, a const table -- is
        // uniform by nature. This crate's own src/rules/mod.rs is exactly this shape and is
        // hand-written, which is what MIN_INDENT_DEPTHS exists to protect.
        let src: String = (0..80)
            .map(|i| format!("pub mod feature{i:03};\n"))
            .collect();
        assert!(run(Lang::Rust, &src).is_empty());
    }

    #[test]
    fn few_blocks_stays_silent() {
        // 72 uniform non-blank lines, but only 4 blocks: block-length variation has too few
        // samples to mean anything.
        let block = "fn f() {\n    let value = raw.trim().to_lowercase();\n".to_string()
            + &"    let other = raw.trim().to_lowercase();\n".repeat(16)
            + "}\n";
        let src = [block.clone(), block.clone(), block.clone(), block].join("\n");
        assert!(run(Lang::Rust, &src).is_empty());
    }

    #[test]
    fn trailing_whitespace_anywhere_stays_silent() {
        let clean = uniform_rust(12);
        assert!(
            !run(Lang::Rust, &clean).is_empty(),
            "precondition: the same body fires without trailing whitespace"
        );
        let mut with_ws = clean.clone();
        with_ws.insert_str(with_ws.find('\n').unwrap(), "   ");
        assert!(run(Lang::Rust, &with_ws).is_empty());
    }

    #[test]
    fn generated_header_stays_silent() {
        let src = format!(
            "// Code generated by protoc-gen-go. DO NOT EDIT.\n\n{}",
            uniform_rust(12)
        );
        assert!(run(Lang::Rust, &src).is_empty());
    }

    #[test]
    fn crlf_source_measures_the_same_as_lf() {
        // str::lines() strips the \r, but a Windows checkout would otherwise shift every
        // length by one and quietly move both statistics.
        let lf = uniform_rust(12);
        let crlf = lf.replace('\n', "\r\n");
        assert_eq!(run(Lang::Rust, &lf).len(), run(Lang::Rust, &crlf).len());
    }

    #[test]
    fn python_and_typescript_fire_on_their_own_uniform_shapes() {
        let py: String = (0..14)
            .map(|i| {
                format!(
                    "def normalize_field{i:02}(raw: str) -> str:\n    \
                     value = raw.strip().lower()\n    \
                     if len(value) == 0:\n        \
                     raise ValueError(\"field{i:02} is empty\")\n    \
                     return value.replace(\" \", \"-\")\n\n"
                )
            })
            .collect();
        assert_eq!(
            run(Lang::Python, &py).len(),
            1,
            "{:?}",
            run(Lang::Python, &py)
        );

        let ts: String = (0..12)
            .map(|i| {
                format!(
                    "export function normalizeField{i:02}(raw: string): string {{\n  \
                     const value = raw.trim().toLowerCase();\n  \
                     if (value.length === 0) {{\n    \
                     throw new Error(\"field{i:02} is empty\");\n  }}\n  \
                     return value.split(\" \").join(\"-\");\n}}\n\n"
                )
            })
            .collect();
        assert_eq!(run(Lang::Ts, &ts).len(), 1, "{:?}", run(Lang::Ts, &ts));
    }

    #[test]
    fn go_is_not_a_declared_lang() {
        // gofmt ships with the toolchain and is not optional, so every Go file in the wild has
        // already been through it; the signals would describe the tool, not the author.
        assert!(!RULE.langs.contains(&Lang::Go));
    }

    #[test]
    fn rule_is_tier_c_and_opt_in() {
        assert_eq!(RULE.tier, Tier::C);
        assert!(!RULE.default_on);
        assert!(RULE.path_gated, "test files are uniform by construction");
    }
}
