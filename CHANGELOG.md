# Changelog

Notable changes per release. The README describes current behavior only;
migration notes live here.

## Unreleased

### Added

- **SLOP042** (`verbosity`, Tier B, on, path-gated): flags a plain source
  comment of 2 to 12 words whose content words all already appear in the
  single statement it annotates, or only name that statement's construct
  (`// increment the counter` over `counter += 1`). At least half the words
  must literally occur in the code. Doc comments, pragmas, questions,
  comments containing code symbols, trailing comments outside a function
  body, and comments carrying a *why*, a condition, or a warning word are
  never flagged; consecutive comment lines count as one block, and the anchor
  is exactly one statement. Usefulness check only: it makes no claim about
  who wrote the comment.
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

### Changed

- **AST rules no longer re-walk the tree.** Every source file used to get one
  full tree traversal per rule call site (up to 11 for a TypeScript file with
  all rules on); `extract` now indexes every named node by kind in its single
  pass and rules query that index. Diagnostics are unchanged. Library:
  `LintContext.tree` is now `index` and `context::extract` returns the index
  as a third element.

### Fixed

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
