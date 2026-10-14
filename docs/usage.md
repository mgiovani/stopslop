# Usage

## Install

```bash
cargo install stopslop
```

Once a day, an outdated install prints a one-line update notice to stderr
(never in CI, never for `--format json|sarif|markdown`, never when stderr is
not a terminal). Set `STOPSLOP_NO_UPDATE_CHECK=1` (or `NO_UPDATE_NOTIFIER=1`)
to turn it off.

**Alternatives:**

```bash
# latest main, for a merged-but-unreleased fix
cargo install --git https://github.com/mgiovani/stopslop

# local checkout, for hacking on stopslop itself
git clone https://github.com/mgiovani/stopslop
cd stopslop
cargo install --path .
```

## Usage

```bash
stopslop                          # lint the current directory
stopslop src/ lib/                 # lint specific paths
stopslop --format json .           # machine-readable output
stopslop --stats .                 # throughput and per-rule finding counts (stderr)
stopslop -j 1 .                    # single worker thread (perf work; 0 = automatic)
stopslop --select SLOP001          # run only the elision rule
stopslop --select rhetoric         # run one rule group (see [Rule groups](rules.md#rule-groups))
stopslop --select ALL              # every rule (SLOP010 still needs --check-imports)
stopslop --ignore SLOP008          # run everything except stub detection
stopslop --select artifact --extend-select SLOP033  # add a rule on top of a narrower select
stopslop --list-rules              # print every rule with its group, tier, default, and active state
stopslop --check-imports .         # also run SLOP010 (unresolved import) — opt-in
stopslop --config path.toml        # use a specific config file instead of the discovered one
stopslop --no-config               # ignore any project or user-level stopslop.toml, CLI flags only
stopslop --write-baseline .        # record today's findings (see [Baseline](#baseline))
stopslop --baseline .              # report only findings that aren't in the baseline
stopslop --staged                  # lint what is about to be committed (index content, not disk)
stopslop --changed                 # staged + unstaged changes against HEAD
stopslop --since origin/main       # files changed on this branch, for PR checks
```

Example output:

![stopslop findings across Python and TypeScript files](https://raw.githubusercontent.com/mgiovani/stopslop/main/assets/findings.png)

`--format json` emits a flat array of findings; `--format sarif` emits a
SARIF 2.1.0 document for GitHub code scanning and similar tools;
`--format markdown` emits the findings grouped under one heading per tier, for
pasting into a PR comment or a report. `--stats` prints a
files/lines/wall-time/throughput summary, followed by one row per rule with
findings (count, code, tier, name, distinct files), to stderr in text and
markdown modes; with `--format json` the output becomes
`{"findings": [...], "stats": {...}}`, and with `--format sarif` the same
object lands in `runs[0].properties.stats`. In both, `stats` also carries
`findings` (total) and `rules`, an array of `{code, tier, name, count, files}`. Wall time runs from startup
through the walk, so it lines up with an external `time`. "skipped" counts
files the walk reached but could not lint (unsupported extension or
unreadable); paths dropped by `.gitignore` or `exclude` are never walked and
are not counted.

![stopslop --format json output](https://raw.githubusercontent.com/mgiovani/stopslop/main/assets/formats.png)

## Selecting by group

`SLOP0NN` numbers are chronological, not thematic, so a numeric prefix can't
express "just the rhetoric rules". Named groups can. They work anywhere a rule
code or prefix does, including `--select`, `--ignore`, and the
`select`/`ignore` keys in `stopslop.toml`:

| Group | What it covers |
|-------|----------------|
| `artifact` | Mechanical leftovers from a generation session: chat turns, tool tokens, unfilled slots |
| `structure` | Structural code smells: swallowed errors, escaped types, stubs, speculative abstraction |
| `stdlib` | Code that rebuilds something the standard library or the platform already provides |
| `rhetoric` | Formulaic rhetorical shapes: clichés, staged reveals, manufactured significance |
| `verbosity` | Words that cost the reader something and return nothing: hedging, filler, padding |
| `sourcing` | Claims with no checkable source behind them |
| `format` | Typographic and Markdown affectations |

```bash
stopslop --select artifact,structure   # the mechanical, high-confidence rules only
stopslop --select SLOP --ignore verbosity  # everything except the density rules
```

Every rule belongs to exactly one group; a test enforces that the table stays
exhaustive, so a new rule can't quietly escape group selection. `--list-rules`
prints the full mapping.

`ALL` is a reserved selector, not a group: `--select ALL` (or `select =
["ALL"]` in config) runs every registered rule plus any `[[custom-rule]]`
entries from your config. SLOP010 is the one exception: it stays off until
`--check-imports` is passed.
## Baseline

Turning stopslop on for an existing codebase usually surfaces findings nobody
plans to fix today. A baseline grandfathers them so CI only fails on findings
that are **new**:

```bash
stopslop --write-baseline .   # writes .stopslop-baseline.json, exits 0
stopslop --baseline .         # later runs report only what isn't in it
```

Use `--baseline=path.json` / `--write-baseline=path.json` for a different file
(the `=` is required, so a bare `--baseline` doesn't swallow your scan path),
or set `baseline = ".stopslop-baseline.json"` in `stopslop.toml`.

Findings are matched by a fingerprint of `code + path + message` with digits
normalized out, not by line number, so editing above a finding doesn't
resurrect it and a density count drifting by one doesn't either. Paths are
normalized as well, with a leading `./` stripped, so a baseline written from
`git ls-files` (`foo.md`) still matches a `stopslop .` run (`./foo.md`). The file
stores a **count** per fingerprint: if a file has three accepted findings from
one rule, it absorbs exactly three, and a fourth is reported. Fix one and the
budget shrinks with it. The count ratchets down on its own; raising it takes a
deliberate `--write-baseline`.

Commit the baseline file so CI and local runs agree.

## Git-aware selection

`--staged`, `--changed`, and `--since REF` (mutually exclusive) replace the walk
with a git-selected file list, so linting scales with the size of a diff
instead of the whole tree. Positional paths, if given, act as git pathspecs
(`stopslop --staged src/` lints only staged files under `src`).

- `--staged` reads each file's **staged** content (`git show :./path`), so a
  partially staged file is checked as it will be committed, not as it sits on
  disk.
- `--changed` includes both staged and unstaged changes against `HEAD`;
  untracked files are not included.
- `--since REF` diffs against the merge base of `REF` and `HEAD` (git 2.30+),
  so `REF` advancing after the branch forked doesn't pull in its changes.
  Built for `stopslop --since origin/main` on a PR.
- An empty selection exits 0. All three compose with `--baseline`, `exclude`,
  and `[per-file-ignores]` exactly like a normal scan.

Pre-commit hook: `stopslop --staged`.

## Exit codes

- `0`: nothing at or above the failing tier.
- `1`: at least one finding at or above the failing tier.
- `2`: usage error, a path that couldn't be scanned, or a config error
  (bad glob, bad regex, unknown key, invalid `[[custom-rule]]` or
  `fail-on-tier` value).

The failing tier is `A` by default, so Tier B and Tier C findings (including
any custom rule declared `tier = "B"` or `tier = "C"`) print without blocking.
Set `fail-on-tier = "B"` in `stopslop.toml`, or pass `--fail-on-tier B`, to add
Tier B to the exit-1 path, and `"C"` to gate on every finding; the CLI flag
wins over the config. A baseline is applied
before the exit code is computed, so baselined findings never fail a run
whatever the tier.

