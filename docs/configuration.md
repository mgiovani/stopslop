# Configuration

[Docs index](README.md)

## Path exemptions

Rules that are path-gated (SLOP005, 006, 008, 009, 010, 037, 038, 039, 040,
042, 043, 045) don't run inside directories or files that look like tests or
generated/vendored code, since empty catches, broad excepts, unresolved
imports, the stdlib/wrapper/single-implementation heuristics, and
`// Check nfkd`-style test-section labels are all legitimately common there:

- Any path segment named `tests`, `test`, `__tests__`, `testdata`,
  `fixtures`, `fixture`, `mocks`, `mock`, `examples`, `example`, `vendor`,
  `node_modules`, or `generated`.
- Filenames matching `test_*`, `conftest.py`, `*_test.go`, `*_test.py`,
  `*.test.ts(x)`, `*.spec.ts(x)`, `*.pyi`, `*.pb.go`, `*_pb2.py`, `*.min.js`.

SLOP001–004 (the junk-text rules) are **not** path-gated: a leaked chat
preamble or elision comment is junk in a test file too.

## Config file

`stopslop --help-config` prints an annotated `stopslop.toml` covering every key.

The nearest `stopslop.toml` walking up from the current directory to the
filesystem root, so a run from a nested cwd (an editor task runner, a
pre-commit hook) still finds the repo-root file. When no project file exists,
stopslop reads `$XDG_CONFIG_HOME/stopslop/stopslop.toml`, which is
`~/.config/stopslop/stopslop.toml` when the variable is unset or empty, on
every platform including macOS. The user-level file is a fallback, never a
merge layer: a project that ships its own `stopslop.toml` is unaffected by
whatever sits in `~/.config`, so CI and local runs stay reproducible. Pass
`--config path` to use a specific file instead:

```toml
select = []                 # rule codes/prefixes/groups to run (empty = every rule except SLOP010)
ignore = ["SLOP009", "verbosity"]  # rule codes/prefixes/groups to subtract
extend-select = ["SLOP033"] # adds on top of `select`, instead of replacing it
extend-ignore = ["SLOP016"] # adds on top of `ignore`, same relationship
exclude = ["**/generated/**"]      # extra walker excludes, on top of .gitignore
check-imports = false
baseline = ".stopslop-baseline.json"  # subtract findings recorded here (omit to disable)
fail-on-tier = "A"          # lowest tier that exits 1; "B" adds Tier B, "C" adds every finding
language = ["en", "pt-BR"]  # restrict which natural-language panels run (omit = every language)

[per-file-ignores]
"docs/**" = ["SLOP036"]     # codes and/or group names; applied after linting, before baseline
```

`select` and `ignore` follow Ruff's composition rules: a CLI `--select`
replaces the config's `select` outright (rather than adding to it), and the
same for `--ignore`. `extend-select` and `extend-ignore` never replace
anything: the config's `extend-select` and any CLI `--extend-select` are
unioned together on top of whatever `select` resolved to, and `extend-ignore`
does the same for `ignore`, subtracted last so an extend-ignore always wins
over an extend-select.

`[per-file-ignores]` keys are globs, matched against the file's display path
with a leading `./` stripped from both the glob and the path, so
`"docs/**"` matches whether you run `stopslop .` (paths print as
`./docs/...`) or `stopslop docs` (paths print as `docs/...`). Values are
lists of codes and/or group names, expanded the same way `select`/`ignore`
are. It's a post-lint filter, applied after the walk and before baseline
filtering, so it composes with a baseline instead of fighting it. An invalid
glob is a config error (exit 2).

Use `--no-config` to ignore any project or user-level `stopslop.toml`. CLI flags for
`select`/`ignore`/`check-imports`/`baseline` override the config file; see
[User-defined rules](#user-defined-rules) for `[[custom-rule]]`.

`language` restricts which natural-language lexicons run, as a string or an
array (`language = "pt-BR"` or `language = ["en", "pt-BR"]`); absent or empty
means every supported language, the default. `pt` and `pt-PT` both resolve
to the same Brazilian lexicon as `pt-BR`. An unrecognized tag is a config
error (exit 2) naming the supported tags. See [Prose
linting](prose.md) for what this axis actually gates.

## User-defined rules

House-specific banned phrases don't need a Rust module. A `[[custom-rule]]`
array of tables in `stopslop.toml` compiles straight to a regex rule:

```toml
[[custom-rule]]
pattern = '(?i)\bsynergy\b'          # required: a regex, matched against comments/strings
                                       # (code files) or masked prose (Markdown/MDX/Text/reST/HTML)
message = "banned house phrase: synergy"  # required: the finding text
tier = "B"                            # optional, "A" or "B", defaults to "B"
fix = "say what the teams actually do"    # optional: printed as a second "fix:" line
files = ["docs/**"]                   # optional glob list; omit for every supported file
```

Codes are auto-assigned `SLOP900`, `SLOP901`, ... in declaration order. Each
`[[custom-rule]]` entry gets one code. They are grouped as `custom` in
`--list-rules`. Every
custom rule is on by default, but still subject to `select`/`ignore` and
suppressible with `ai-slop-ignore: SLOP900` like any built-in rule. An
invalid regex, an invalid `tier`, or an invalid `files` glob is a config
error (exit 2) naming the offending entry's index.

`files` globs match the file's display path, with a leading `./` stripped
from both the glob and the path exactly as `[per-file-ignores]` does, so
`"docs/**"` behaves the same whether you run `stopslop .` or `stopslop docs`.

### File rules

`kind = "file"` judges a whole file instead of a phrase. It requires `files`
and exactly one predicate; `pattern` is rejected, and the predicate keys are
rejected on the default `kind = "phrase"`.

| Key | Fires when | Anchor |
|---|---|---|
| `max-lines = N` | the file has more than N lines | line N+1, col 1 |
| `forbid = "regex"` | the regex matches the raw source | first match |
| `require = "regex"` | the regex matches nowhere | line 1, col 1 |

```toml
[[custom-rule]]
kind = "file"
files = ["**/*.py", "**/*.ts", "**/*.tsx"]
max-lines = 300
message = "file is over 300 lines; split by bounded context"

[[custom-rule]]
kind = "file"
files = ["**/__init__.py"]
forbid = '\S'
message = "__init__.py must be empty; import from the defining module"
```

A file rule yields at most one finding per file. Regexes have no lookaround.
Write `(?-u:\b)` instead of `\b`: a Unicode word boundary slows every
non-ASCII input. Keep the line count out of `message` so a baselined file that
grows stays baselined.

## `--check-imports`

SLOP010 cross-references each import against the project's manifest
(`pyproject.toml`/`requirements.txt`, `package.json`/`tsconfig.json`,
`go.mod`, `Cargo.toml`) plus an embedded stdlib/builtin list per language.

Honest caveats:

- If no manifest is found for a language, that language's checks are
  skipped silently rather than risk false positives.
- It's a static namecheck, not a resolver. It can still false-positive or
  false-negative on workspace/path dependencies and dynamic imports. Unusual
  build setups can do the same.
- It is Tier B, so it fails the build only under `fail-on-tier = "B"` or
  `"C"`: treat it as a lead to investigate, not a hard gate.

