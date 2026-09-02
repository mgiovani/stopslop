# Changelog

Notable changes per release. The README describes current behavior only;
migration notes live here.

## Unreleased

### Added

- **SLOP042** (`verbosity`, Tier B, on, path-gated): flags a plain source
  comment of 2 to 12 words whose content words all already appear in the
  single statement it annotates, or only name that statement's construct
  (`// increment the counter` over `counter += 1`). More than half the words
  must literally occur in the code, inflection aside (`parsed` meets
  `parse_header`). Never flagged: doc comments, pragmas, questions, comments
  containing code symbols or an identifier the statement lacks, and comments
  carrying a *why*, a constraint, or a warning word. Also exempt are the plain
  comments that serve as docs where a language has no doc syntax (Go file
  scope and struct fields, Python module and class attributes). Consecutive
  comment lines count as one block, and the anchor is exactly one statement.
  Usefulness check only: it makes no claim about who wrote the comment.
- **SLOP043** (`verbosity`, Tier B, on, path-gated): flags a plain comment
  block of more than 40 words. Steidl, Hummel and Jürgens (ICPC 2013) found
  developers keep 30-plus-word inline comments because they carry global
  information; that is what a doc comment, a README section or a commit
  message is for, so past three full lines a plain comment is either
  narrating the code or misfiling a design note. Doc comments, godoc, license
  headers, generated files and commented-out code are exempt. On human code
  (cargo registry, CPython, Go stdlib) 6 to 10 percent of plain comment
  blocks exceed the cap; this repository's own rate was 21 percent before the
  rule went in, which is the kind of drift it exists to catch.
- Outdated installs print `stopslop: X is installed, Y is available` to
  stderr once per day, text format only, and only when stderr is a terminal
  and `CI` is unset. `STOPSLOP_NO_UPDATE_CHECK=1` (or `NO_UPDATE_NOTIFIER`)
  disables it. Network and cache errors are swallowed and never affect the
  exit code.
- `--stats` reports files scanned, files skipped, lines scanned, wall time and
  lines/s. Text and markdown modes print it to stderr, leaving stdout
  untouched; `--format json` wraps the array as `{findings, stats}` only when
  the flag is given, and `--format sarif` puts it in
  `runs[0].properties.stats`. Library: `lint_paths` now returns
  `(Vec<Diagnostic>, Stats)`.
- `fail-on-tier` (config key and `--fail-on-tier` flag) sets the lowest tier
  that exits 1. Previously nothing could put a Tier B rule on the exit-1 path.
- `--staged`, `--changed`, and `--since REF` (mutually exclusive) lint a
  git-selected file list instead of walking the tree: staged index content,
  staged-plus-unstaged changes against `HEAD`, or everything since the merge
  base with `REF`. Positional paths act as git pathspecs.
- **Natural-language axis.** `NatLang` (`En`, `PtBr`) is a new axis,
  orthogonal to `Lang`: which phrase-panel lexicons a rule is validated on,
  separate from which file syntax it lints. Every `RuleDef` now declares
  `natlangs`, an optional `language` key in `stopslop.toml` (string or array,
  default every supported language) restricts a run to specific tags, and
  `--list-rules` gained a `NATLANG` column. A new `tests/natlang_witness.rs`
  harness requires a `tests/fixtures/markdown/pt-br/` fixture before a rule
  can declare `PtBr`. No rule ships a pt-BR phrase panel yet; this lands the
  axis and the neutral fixes below. See
  [#30](https://github.com/mgiovani/stopslop/issues/30).

- `-j N` / `--threads N` picks the walk's worker count (`0`, the default,
  chooses automatically); `-j 1` makes per-rule timings add up for perf work.
- `bench/gen_inputs.py` and `bench/run.sh` generate the stress inputs from
  issue #21 and print a base-vs-new hyperfine table; `tests/stress.rs` bounds
  the wall clock on them and runs in CI as `cargo test --release -- --ignored`.
- **SLOP044** (`artifact`, Tier B, on, HTML only): flags `<title>Document</title>`,
  the editor `!` expansion a generated page ships with, and an empty `<title>`.
  Any other title is the author's call.
- HTML attribute values reach `ctx.strings` as `name="value"` entries, so
  **SLOP009** now runs on HTML: its existing panel (`example.com`, `YOUR_X`,
  `John Doe`, secret shapes) plus a placeholder-image host
  (`via.placeholder.com`, `placehold.co`, `placekitten.com`, `dummyimage.com`)
  or an `alt` whose whole value is a generic word (`alt="image"`). `alt=""` is
  correct for a decorative image and `picsum.photos` serves real photos; neither
  matches. **SLOP013** also flags a `<!-- your content here -->` comment.
- **HTML prose linting** (`.html`, `.htm`, new `Lang::Html`). A tree-sitter-html
  parse restores only what a reader sees into the masked stream: visible
  text plus HTML comments and `href`/`src` attribute values; `<script>`,
  `<style>`, `<pre>`, `<code>`, `<textarea>`, `<template>`, `<svg>`,
  `<math>`, and `<noscript>` subtrees are skipped, and Django/Jinja
  `{{ … }}`/`{% … %}`/`{# … #}` are blanked before the parse. `<h1>`-`<h6>`
  map to headings, and the suppression comments work unchanged. Each leaf
  block element (`<p>`, `<div>`, `<blockquote>`, `<td>`) is one paragraph, so
  SLOP029, SLOP030, SLOP034, and SLOP041 run on HTML as they do on Markdown.
  List items, table cells, headings, and form controls are never paragraphs.
  A paragraph inside `<footer>`, `<aside>`, or `<nav>` is never the ending
  SLOP029 reads.
  `<strong>`/`<b>` count toward SLOP019, inline `<code>` counts as words the
  way a backtick span does, SLOP021 checks heading case, and `&mdash;`,
  `&#8212;`, and the curly-quote entities feed SLOP018 and SLOP020 as if
  typed. `&ndash;` stays undecoded: a numeric range is its common use, and
  the range exemption needs the raw neighbours. Every prose rule now runs on
  HTML. New dependency: `tree-sitter-html`, because neither std
  nor regex can express "blank everything that is not a text node" (a `>`
  inside an attribute value or a `</script>` inside a JS string defeats a
  regex strip). See
  [#29](https://github.com/mgiovani/stopslop/issues/29).

### Changed

- Config discovery walks from the current directory up to the filesystem root
  for `stopslop.toml` and takes the nearest one, so a run from a nested cwd no
  longer silently lints with defaults. When no project file exists it falls
  back to `$XDG_CONFIG_HOME/stopslop/stopslop.toml` (`~/.config/...` when the
  variable is unset or empty, macOS included). The user-level file is a
  fallback, never merged with a project config. `--config` and `--no-config`
  behave as before.
- Regex word boundaries are ASCII-scoped (`(?-u:\b)`), which keeps the regex
  crate on its lazy DFA for documents with em dashes, curly quotes or accents:
  an 8 MB prose file went from 11.3 s to 0.83 s, the cargo-registry corpus
  from 3.0 s to 1.35 s. A unit test fails on the next unscoped boundary.
- `ProseDoc` answers heading, URL and fence lookups by binary search and
  keeps one line index for every rule, so SLOP033 is linear in headings: a
  20 MB file with 78k headings went from 35 s to 3.3 s.
- Enabled prose rules compile their regexes in parallel with the file walk;
  a run over a three-line file went from 28 ms to 13 ms.
- **AST rules no longer re-walk the tree.** Every source file used to get one
  full tree traversal per rule call site (up to 11 for a TypeScript file with
  all rules on); `extract` now indexes every named node by kind in its single
  pass and rules query that index. Diagnostics are unchanged. Library:
  `LintContext.tree` is now `index` and `context::extract` returns the index
  as a third element.
- **SLOP026** now also matches a colon reveal that opens a line after leading
  whitespace, since a blanked HTML tag always leaves some; an indented
  Markdown continuation line that starts the shape is caught the same way.
- HTML-specific rule behavior: **SLOP033** closes a sentence at every HTML
  block element, so a `<select>` of sixty `<option>`s doesn't read as one
  60-word sentence; **SLOP011** treats a `Step N:` that opens a heading or
  list item as structure rather than residue, and finds a block-opening
  `You're asking about …`; **SLOP018** keeps its attribution-dash exemption
  for `<p>— Author</p>`.

### Fixed

- **SLOP018** treated every line-initial dash as attribution only when the
  line above it was blank or a blockquote marker, so the second and later
  lines of a multi-line dialogue run (each opening with its own attribution
  dash, e.g. pt-BR travessão) were flagged as mid-prose punctuation. A
  line-initial dash is now also exempt when the line above it opens with a
  dash too, extending the exemption down the whole run; a dash that
  interrupts the middle of a dialogue line still fires.
- **SLOP022**'s self-answered question/answer check required an ASCII
  `[A-Za-z]` sentence-initial letter, so a self-answered pair opening on an
  accented word, common in Portuguese, never matched. The leading-letter
  class is now `[^\W\d_]` (any Unicode letter).

- **SLOP017** counted any comma series of three-or-more items, including the
  tail of a longer one, so plain enumerations tripped it (a protected-
  characteristics list, a list of agent product names). It now requires
  exactly three items and rejects lists of proper nouns. The anchor and fix
  hint follow whichever signal actually fired instead of always naming a
  participial tail.
- **SLOP034** counted across the whole file, so a catalog of differently-named
  things was flagged for words describing separate entries. Scope is now one
  section's running prose, with bullet lists and tables excluded.
- **SLOP011** matched `step 1:` anywhere, flagging numbered procedural
  headings. It is now exempt when it heads a section or list item, and still
  reported mid-sentence.
- **SLOP021** read blanked inline code as a marker position, so
  ``- `feat` → **Features** `` looked like a bullet followed by an arrow.
  Emoji and technical symbols are now counted and reported separately, and the
  emoji class covers text-default emoji that `\p{Emoji_Presentation}` misses.
- **Baselines** embedded the path exactly as passed, so one written from
  `git ls-files` re-reported every finding under `stopslop .`. Paths are now
  normalized on both write and load, so existing baselines keep matching.
- **Column lookup** rescanned from the line start for every diagnostic, so a
  single-line file with hundreds of thousands of findings degraded
  quadratically (a 1.8 MB one-line file with 200k em dashes took 18s). Columns
  now resume from the previous answer on the same line, which brings that file
  to under 3s. Output is unchanged.

## 0.5.0

### Changed

- **SLOP014, SLOP018, SLOP022, SLOP023, SLOP024, SLOP025, and SLOP029 moved
  from Tier A to Tier B.** They stay on by default, so you still see them, but
  a run that used to exit 1 on a stray em dash now exits 0 and prints a
  warning.

  To get the old blocking behavior back, either set `fail-on-tier = "B"`
  (added in Unreleased, above) to gate on every finding, or keep using
  `--format json` / `--format sarif`, whose output still carries each
  finding's code, and fail a CI step that greps for one of these seven.

- Every rule is enabled by default except SLOP010, which stays behind
  `--check-imports`.
- Three-state severity, `[[custom-rule]]` entries, and SLOP041.

## 0.2.0

- Rule groups, baseline gating, fix hints, and rules SLOP031-040.
