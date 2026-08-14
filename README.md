<p align="center">
  <img src="https://raw.githubusercontent.com/mgiovani/ai-stop-slop/main/assets/hero.png" alt="stopslop: Like Ruff, but for AI slop. Catch the junk AI leaves in your code.">
</p>

# stopslop

Like Ruff, but for AI slop.

[![CI](https://github.com/mgiovani/ai-stop-slop/actions/workflows/ci.yml/badge.svg)](https://github.com/mgiovani/ai-stop-slop/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/mgiovani/ai-stop-slop/blob/main/LICENSE)
[![Crates.io](https://img.shields.io/crates/v/stopslop.svg)](https://crates.io/crates/stopslop)

![stopslop demo](https://raw.githubusercontent.com/mgiovani/ai-stop-slop/main/assets/demo.gif)

Your linter can't see `// ... rest of code unchanged`. ESLint, Ruff, and
Clippy throw away comment and string content before analysis: exactly
where AI coding artifacts live. stopslop reads what they discard: leaked
chat preambles, elision comments that silently deleted code, stray markdown
fences, placeholder credentials, and package imports that don't resolve to
anything you declared. It reads your prose too: Markdown, MDX, plain text,
and reST get the same deterministic treatment. One static binary, no LLM at
scan time: same input, same output, every run.

- One fast static binary, zero config to get started.
- Deterministic: no LLM calls, no API calls, no network access at scan time.
- Built for CI: exit codes, SARIF, JSON, and inline suppressions.

## Why your existing linter misses this

ESLint, Ruff, and Clippy parse comments and strings as trivia their default
rule sets don't inspect, which is exactly where copy-pasted chat output and
truncated edits land. A few examples:

| Artifact | ESLint / Ruff / Clippy | stopslop |
|---|---|---|
| `// ... rest of code unchanged` | Not linted (comment content is ignored by default rules) | SLOP001 |
| Leaked chat preamble ("Certainly! Here's...") | Not linted (same reason) | SLOP002 |
| `x as unknown as T` type-escape chain | No stock rule (`as unknown` alone is valid, idiomatic TS) | SLOP007 |
| `YOUR_API_KEY`, `sk-...`-shaped secret | Not linted (string literal content is ignored by default rules) | SLOP009 |

`stopslop` is a deterministic linter for TypeScript, Python, Go, and Rust.
Every rule is a tree-sitter AST match or a regex over extracted
comments/strings: same input, same output, every time. This is a
**quality gate, not an AI-origin detector**: it doesn't try to prove a human
didn't write the code, it flags patterns that are junk regardless of who (or
what) produced them.

## Install

```bash
cargo install stopslop
```

**Alternatives:**

```bash
# latest main, for a merged-but-unreleased fix
cargo install --git https://github.com/mgiovani/ai-stop-slop

# local checkout, for hacking on stopslop itself
git clone https://github.com/mgiovani/ai-stop-slop
cd ai-stop-slop
cargo install --path .
```

## Usage

```bash
stopslop                          # lint the current directory
stopslop src/ lib/                 # lint specific paths
stopslop --format json .           # machine-readable output
stopslop --select SLOP001          # run only the elision rule
stopslop --select rhetoric         # run one rule group (see "Rule groups" below)
stopslop --select ALL              # every rule (SLOP010 still needs --check-imports)
stopslop --ignore SLOP008          # run everything except stub detection
stopslop --select artifact --extend-select SLOP033  # add a rule on top of a narrower select
stopslop --list-rules              # print every rule with its group, tier, and default
stopslop --check-imports .         # also run SLOP010 (unresolved import) — opt-in
stopslop --config path.toml        # use a specific config file instead of ./stopslop.toml
stopslop --no-config               # ignore any stopslop.toml, CLI flags only
stopslop --write-baseline .        # record today's findings (see "Baseline" below)
stopslop --baseline .              # report only findings that aren't in the baseline
```

Example output:

![stopslop findings across Python and TypeScript files](https://raw.githubusercontent.com/mgiovani/ai-stop-slop/main/assets/findings.png)

`--format json` emits a flat array of findings; `--format sarif` emits a
SARIF 2.1.0 document for GitHub code scanning and similar tools.

![stopslop --format json output](https://raw.githubusercontent.com/mgiovani/ai-stop-slop/main/assets/formats.png)

## Rule groups

`SLOP0NN` numbers are chronological, not thematic, so a numeric prefix can't
express "just the rhetoric rules". Named groups can, and work anywhere a rule
code or prefix does — `--select`, `--ignore`, and the `select`/`ignore` keys in
`stopslop.toml`:

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
resurrect it and a density count drifting by one doesn't either. The file
stores a **count** per fingerprint: if a file has three accepted findings from
one rule, it absorbs exactly three, and a fourth is reported. Fix one and the
budget shrinks with it. The count ratchets down on its own; raising it takes a
deliberate `--write-baseline`.

Commit the baseline file so CI and local runs agree.

## Rules

| Code | Group | Name | Tier | Langs | Description |
|------|-------|------|------|-------|--------------|
| SLOP001 | artifact | Elision / "rest unchanged" comment | A, on | TS, TSX, Python, Go, Rust | A comment like `// ... rest unchanged` may mark code an AI dropped while truncating an edit |
| SLOP002 | artifact | Chat preamble leaked into code | A, on | TS, TSX, Python, Go, Rust | Chat-assistant preamble ("Certainly! Here's the updated...") pasted straight into a source comment |
| SLOP003 | artifact | Stray markdown code fence in source | A, on | TS, TSX, Python, Go, Rust | A bare ` ``` ` fence line left in source, usually from a whole file pasted out of a chat reply |
| SLOP004 | artifact | AI attribution / chat-share artifact | A, on | TS, TSX, Python, Go, Rust | "Generated by ChatGPT", a `claude.ai/share` link, or other chat-export junk in a comment |
| SLOP005 | structure | Empty / log-only catch | A, on | TS, TSX, Go, Rust | A `catch`/error branch that swallows the error with an empty body or just a log call |
| SLOP006 | structure | Broad / swallowing except | A, on | Python | A bare or overly broad `except:` that swallows the exception instead of handling it |
| SLOP007 | structure | Type-escape (`as any`/`as unknown`/`@ts-ignore`) | A, on | TS, TSX | `as any`, an `x as unknown as T` chain that fully escapes the type checker, or `@ts-ignore`/`@ts-nocheck` |
| SLOP008 | structure | Stub-only / unimplemented body | A, on | TS, TSX, Python, Go, Rust | A function whose entire body is `pass`/`...`/`throw new Error("not implemented")`/`todo!()`/empty |
| SLOP009 | structure | Placeholder / sample credential value | A, on | TS, TSX, Python, Go, Rust | A hardcoded `YOUR_API_KEY`, `example.com`, `sk-...`-shaped secret, or other sample value |
| SLOP010 | structure | Unresolved package import | B, off | TS, TSX, Python, Go, Rust | An imported package that isn't declared in the project's manifest or stdlib (opt-in, `--check-imports`) |
| SLOP011 | artifact | Assistant-response residue in prose | A, on | Markdown, MDX, Text, reST | A leftover chat-turn phrase (self-ID disclaimer, refusal boilerplate, a line-initial `Certainly!` opener, a trailing `let me know if you have` closer, reasoning-chain scaffolding like `let's think about this`/`Step 1:`, or a paragraph-initial acknowledgment loop like `To answer your question, ...`) left unedited in prose |
| SLOP012 | artifact | LLM tool / citation artifact tokens | A, on | Markdown, MDX, Text, reST | A leftover search/citation-tool token (`turn0search0`, `:contentReference[oaicite:1]`, a `【12†L3】` marker, `utm_source=chatgpt.com`) left in text |
| SLOP013 | artifact | Unfilled template placeholder text | A, on | Markdown, MDX, Text, reST | An unfilled placeholder (`[Your Name]`, `INSERT_SOURCE_URL_30`, a `date: 2025-XX-XX` stub) left in place of real content |
| SLOP014 | rhetoric | Formulaic cliché phrase | B, on | Markdown, MDX, Text, reST | A stock marketing/narrative cliché (`unlock the power of`, `in today's fast-paced world`, `a testament to`) |
| SLOP015 | verbosity | Hedging & filler-phrase density | B, on  | Markdown, MDX, Text, reST | A document-wide density of hedging/filler phrases (`it's worth noting that`, `in conclusion`, `first and foremost`); an adjacent hedge stack like `might potentially` fires on its own, without waiting for the density threshold |
| SLOP016 | verbosity | Overused-vocabulary density | B, on  | Markdown, MDX, Text, reST | A document-wide density of overused vocabulary (`delve`, `tapestry`, `robust`, `leverage`) across enough distinct terms to read as filler |
| SLOP017 | rhetoric | Rhetorical parallelism / false-depth scaffolding density | B, on  | Markdown, MDX, Text, reST | A document-wide density of rule-of-three lists, `not only X but also Y` phrasing, and trailing `, underscoring its...` participles |
| SLOP018 | format | Mid-prose em/en dash | B, on | Markdown, MDX, Text, reST | A mid-sentence em dash (`—`), en dash (`–`), or spaced ASCII `--` that should be rewritten out of the sentence (numeric ranges like `2020–2024`, and an attribution dash opening a block or blockquote like `— Oscar Wilde`, are exempt) |
| SLOP019 | format | Boldface & bold-lead-in list overuse | B, on  | Markdown, MDX | Boldface overuse in body prose, or 3+ consecutive `- **Term**: ...` bold-lead-in list items |
| SLOP020 | format | Typographic (smart) quotes in source | B, on  | Markdown, MDX, Text, reST | Curly quotes/apostrophes in source where straight ASCII quotes are expected |
| SLOP021 | format | Heading & marker formatting affectations | B, on  | Markdown, MDX | Emoji used as a heading/list marker, headings written in Title Case against an otherwise sentence-case document, or headings stacked over two-sentence sections |
| SLOP022 | rhetoric | Formulaic opener / rhetorical setup | B, on | Markdown, MDX, Text, reST | A throat-clearing or faux-insight opener (`Here's the thing`, `What nobody tells you`, `Plot twist:`), or a self-answered `Question? Answer.` pair opening a line |
| SLOP023 | rhetoric | Binary contrast / negative listing | B, on | Markdown, MDX, Text, reST | The `It's not X. It's Y.` / `The question isn't X, it's Y` shape, or a `Not a X. Not a Y.` fragment run |
| SLOP024 | rhetoric | Importance puffery / fake-strong verb | B, on | Markdown, MDX, Text, reST | An inflated significance claim (`marks a pivotal moment`, `solidifies its position`), a `serves as a centralized hub`-style linking verb where plain `is` reads better, or a faux-scale range (`from the singularity of the Big Bang to the enigmatic dance of dark matter`) standing in for an actual magnitude |
| SLOP025 | sourcing | Unsourced weasel attribution | B, on | Markdown, MDX, Text, reST | Anonymous authority (`experts agree`, `studies show`) with no link, footnote, or citation anywhere on the line, or notability by name-dropping three-plus outlets (`cited in TechCrunch, Forbes, and Wired`) with no per-citation context |
| SLOP026 | rhetoric | Dramatic colon reveal | B, on  | Markdown, MDX, Text, reST | A short noun phrase, a colon, then a lowercase dramatic reveal (`The best part: it learns`) |
| SLOP027 | verbosity | Empty filler phrase & adverb density | B, on  | Markdown, MDX, Text, reST | A document-wide density of empty phrases (`when it comes to`, `at its core`) and filler adverbs (`simply`, `actually`) |
| SLOP028 | verbosity | Weak verb phrase / vague quantifier | B, on  | Markdown, MDX, Text, reST | A document-wide density of nominalizations (`made a decision`, `has the ability to`) and vague quantifiers used where a number belongs (`significantly improves`) |
| SLOP029 | rhetoric | Summary-recap ending / fake-profound kicker | B, on | Markdown, MDX, Text, reST | A closing block that restates the piece (`In conclusion`, `Overall`) or lands a mic-drop line (`It's already here.`) |
| SLOP030 | rhetoric | Dramatic fragmentation / robotic rhythm | B, on  | Markdown, MDX, Text, reST | Stacked one-clause fragments (`That's it. That's the whole thing.`), consecutive `And`-initial sentences, or paragraph-wide repeated sentence shapes |
| SLOP031 | rhetoric | Promotional / advertisement language | B, on  | Markdown, MDX, Text, reST | Brochure register in technical prose (`boasts a`, `industry-leading`, `a hidden gem`) at document-wide density |
| SLOP032 | verbosity | Hyphenated-compound overuse | B, on  | Markdown, MDX, Text, reST | Stacked hyphenated modifiers used as filler (`end-to-end`, `data-driven`, `battle-tested`) at document-wide density |
| SLOP033 | verbosity | Overlong sentence | B, on  | Markdown, MDX, Text, reST | A sentence over 50 words, reported with its actual word count (URLs count as one word) |
| SLOP034 | verbosity | Synonym rotation across a closed concept set | B, on  | Markdown, MDX, Text, reST | One concept named two ways in one document (`check` and `verify`, `config` and `settings`) where technical writing should fix one term |
| SLOP035 | rhetoric | Outline-shaped filler section | B, on  | Markdown, MDX, Text, reST | A `Challenges and Future Prospects`-style section heading, or `despite these challenges` boilerplate, standing in for specifics |
| SLOP036 | rhetoric | Diff-anchored documentation | B, on  | Markdown, MDX, Text, reST | Docs narrating a change (`was added to replace`, `no longer requires`) instead of describing current behavior; changelogs and migration guides are exempt |
| SLOP037 | stdlib | Reinvented stdlib / native platform feature | B, on  | TS, TSX, Python, Go, Rust | Hand-rolled code where a standard-library or platform primitive exists (`JSON.parse(JSON.stringify(x))`, `for i in range(len(xs))`, `ioutil.ReadFile`) |
| SLOP038 | stdlib | Dependency with a stdlib equivalent | B, on  | TS, TSX | An import of a package the platform already covers (`moment`, `uuid`, `node-fetch`, `left-pad`) |
| SLOP039 | structure | Pass-through wrapper function | B, on  | TS, TSX, Python, Go, Rust | A function whose whole body forwards its own parameters, unchanged, to another function |
| SLOP040 | structure | Single-implementation interface / abstract | B, on  | TS, TSX, Python | An interface or abstract class with exactly one implementor in the same file: abstraction with no second user |
| SLOP041 | verbosity | Mechanical uniformity (templated prose) | B, on  | Markdown, MDX, Text, reST | A document of 200+ words where at least 2 of 3 document-level signals trip together: flat sentence-length burstiness, low type-token vocabulary ratio, and repeated word-trigrams |

Every rule is exactly one of three states, `--list-rules` prints the DEFAULT
column so you can check any given rule at a glance:

- **Tier A, on by default (12 rules)**: mechanical artifacts (SLOP001–009,
  SLOP011–013) with no legitimate reading. A finding here fails the run
  (exit 1) and blocks CI.
- **Tier B, on by default (28 rules)**: everything else except SLOP010.
  Judgment calls — density and style checks on prose, stdlib/structure
  heuristics — that warn without ever exiting 1. Expect some noise; silence
  what you don't want with `ignore`/`--ignore` by code or group.
- **Tier B, off by default (1 rule)**: SLOP010, gated behind
  `--check-imports` because of its false-positive risk with private
  registries and dynamic imports.

**Behavior change from earlier releases:** SLOP014, SLOP018, SLOP022,
SLOP023, SLOP024, SLOP025, and SLOP029 used to be Tier A and fail CI. They're
now Tier B: still on by default, so you'll still see them, but a run that
used to exit 1 on a stray em dash now exits 0 and just prints a warning.
Tier is a fixed property of each rule, not something select/ignore/config
can override: there's no flag that puts a Tier B rule back on the exit-1
path. If your CI relied on any of these seven blocking the build, the
`--format json`/`--format sarif` output still carries each finding's code, so
a CI step that greps for one of these seven codes and fails on a match gets
the old behavior back.

Findings that have one concrete replacement print it on a second line:

```
src/util.ts:14:11 SLOP037 deep clone via a JSON round trip
    fix: use `structuredClone(value)`
```

`--format json` carries the same text in a `fix` field, omitted when a rule
flags a document-wide pattern with no single substitutable span.

Note on SLOP007: a bare `x as unknown` is not flagged on its own: it's a
legitimate first step in TypeScript's narrowing idiom. Only the chained form,
`x as unknown as T`, is flagged, since that's the pattern that fully defeats
the type checker.

## Prose linting

stopslop also lints `.md`, `.mdx`, `.txt`, and `.rst` files: same binary,
same deterministic regex/structural matching, no LLM involved in the scan.
SLOP011–013 catch mechanical, high-confidence artifacts (leftover chat-turn
phrasing including reasoning-chain scaffolding and acknowledgment loops,
citation-tool tokens, unfilled placeholders) and are Tier A: a finding there
fails the run. SLOP014, SLOP018, and the formulaic-shape rules SLOP022–025
and SLOP029 are on by default too but Tier B, warn-only: judgment calls the
tool still surfaces every run, just without the power to fail CI. SLOP018
flags every mid-prose dash outright (the one exemption is a block-opening
attribution dash like `— Oscar Wilde`).

The rest are document-wide density and style checks, also on by default: hedging,
overused vocabulary, rhetorical parallelism, boldface overuse, smart quotes,
heading formatting, promotional register, hyphen stacking, sentence length,
synonym rotation, outline filler, change-narrating docs, colon reveals,
filler and adverb density, weak-verb phrasing, dramatic fragmentation, and
mechanical uniformity. SLOP041 is the one rule in this list that measures statistics
(burstiness, vocabulary diversity, trigram repetition) instead of matching a
phrase. It catches templated prose that rotates its vocabulary just enough to
slide past every phrase-based rule. These are judgment calls rather than
mechanical certainties, so they're warn-only. Silence the ones you don't want
by code (`--ignore SLOP015`) or by group (`--ignore verbosity`). Expect some
noise from them: this very README trips SLOP017, since its prose is heavy on
the enumerations that rule counts. That's Tier B working as intended, a lead
to investigate rather than a verdict. As with the rest of stopslop, this is a writing-quality
gate, not a claim about who or what wrote the text: every rule flags a
concrete pattern (a stale phrase, an unfilled slot, a density threshold,
a document-level statistic), never "this is AI-generated."

## Suppression

Two escape valves, both plain comments so they work in any language's
comment syntax:

- `// ai-slop-ignore`: suppresses findings on that line and the line below
  it (so it works whether the comment is trailing or sits above the flagged
  line).
- `// ai-slop-ignore-file`: anywhere in the file, drops every finding in
  that file.

In Markdown/MDX/Text/reST files, use the HTML comment form instead:
`<!-- ai-slop-ignore -->` / `<!-- ai-slop-ignore-file -->`, with the same
two-line and whole-file suppression semantics.

Bare, either directive suppresses every rule. Add `: CODE,CODE` (commas or
spaces both work) to scope it to specific rules, or to a group name:

```ts
// ai-slop-ignore: SLOP002,SLOP004
// Certainly! Generated by ChatGPT — kept verbatim as a fixture for this rule's own test
```

```markdown
<!-- ai-slop-ignore-file: SLOP018 -->
```

Group names expand the same way they do in `--select`/`--ignore`, so `//
ai-slop-ignore: verbosity` silences every rule in that group on the line.

The directive has to **open** the comment body, right after the delimiter. A
comment that merely mentions the token while documenting the feature, like
`// dogfooding would self-flag these; "ai-slop-ignore" is the escape hatch`,
is not a directive and suppresses nothing: the token has to be the first
thing after `//`/`#`/`<!--`/etc. This is the thing most likely to look like a
bug the first time you hit it: writing docs *about* the suppression syntax
in a comment that isn't itself a suppression is fine, and does nothing.

A suppression that absorbs no finding (wrong code, or nothing to suppress on
that line) is a dead suppression. stopslop prints a warning for it to
stderr, naming the file and line, so a stale or mistyped ignore doesn't
silently rot:

```
stopslop: warning: src/util.ts:14: ai-slop-ignore (SLOP018) suppressed nothing
```

![suppressing a finding with ai-slop-ignore](https://raw.githubusercontent.com/mgiovani/ai-stop-slop/main/assets/suppress.gif)

## Path exemptions

Rules that are path-gated (SLOP005, 006, 008, 009, 010, 037, 038, 039, 040)
don't run inside directories or files that look like tests or
generated/vendored code, since empty catches, broad excepts, unresolved
imports, and the stdlib/wrapper/single-implementation heuristics are all
legitimately common there:

- Any path segment named `tests`, `test`, `__tests__`, `testdata`,
  `fixtures`, `fixture`, `mocks`, `mock`, `examples`, `example`, `vendor`,
  `node_modules`, or `generated`.
- Filenames matching `test_*`, `conftest.py`, `*_test.go`, `*_test.py`,
  `*.test.ts(x)`, `*.spec.ts(x)`, `*.pyi`, `*.pb.go`, `*_pb2.py`, `*.min.js`.

SLOP001–004 (the junk-text rules) are **not** path-gated: a leaked chat
preamble or elision comment is junk in a test file too.

## Config file

`stopslop.toml` in the current directory (or pass `--config path`):

```toml
select = []                 # rule codes/prefixes/groups to run (empty = every rule except SLOP010)
ignore = ["SLOP009", "verbosity"]  # rule codes/prefixes/groups to subtract
extend-select = ["SLOP033"] # adds on top of `select`, instead of replacing it
extend-ignore = ["SLOP016"] # adds on top of `ignore`, same relationship
exclude = ["**/generated/**"]      # extra walker excludes, on top of .gitignore
check-imports = false
baseline = ".stopslop-baseline.json"  # subtract findings recorded here (omit to disable)

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

Use `--no-config` to ignore any `stopslop.toml` present. CLI flags for
`select`/`ignore`/`check-imports`/`baseline` override the config file; see
[User-defined rules](#user-defined-rules) below for `[[custom-rule]]`.

## User-defined rules

House-specific banned phrases don't need a Rust module. A `[[custom-rule]]`
array of tables in `stopslop.toml` compiles straight to a regex rule:

```toml
[[custom-rule]]
pattern = '(?i)\bsynergy\b'          # required: a regex, matched against comments/strings
                                       # (code files) or masked prose (Markdown/MDX/Text/reST)
message = "banned house phrase: synergy"  # required: the finding text
tier = "B"                            # optional, "A" or "B", defaults to "B"
fix = "say what the teams actually do"    # optional: printed as a second "fix:" line
files = ["docs/**"]                   # optional glob list; omit for every supported file
```

Codes are auto-assigned `SLOP900`, `SLOP901`, ... in declaration order, one
per `[[custom-rule]]` entry, and grouped as `custom` in `--list-rules`. Every
custom rule is on by default, but still subject to `select`/`ignore` and
suppressible with `ai-slop-ignore: SLOP900` like any built-in rule. An
invalid regex, an invalid `tier`, or an invalid `files` glob is a config
error (exit 2) naming the offending entry's index.

`files` globs match the file's display path, with a leading `./` stripped
from both the glob and the path exactly as `[per-file-ignores]` does, so
`"docs/**"` behaves the same whether you run `stopslop .` or `stopslop docs`.

## `--check-imports`

SLOP010 cross-references each import against the project's manifest
(`pyproject.toml`/`requirements.txt`, `package.json`/`tsconfig.json`,
`go.mod`, `Cargo.toml`) plus an embedded stdlib/builtin list per language.

Honest caveats:

- If no manifest is found for a language, that language's checks are
  skipped silently rather than risk false positives.
- It's a static namecheck, not a resolver: workspace/path dependencies,
  dynamic imports, and unusual build setups can still false-positive or
  false-negative.
- It never fails the build on its own (Tier B): treat it as a lead to
  investigate, not a hard gate.

## Exit codes

- `0`: no Tier A findings. Tier B findings (including any custom rule
  declared `tier = "B"`) never affect the exit code: they print,
  they don't block.
- `1`: at least one Tier A finding.
- `2`: usage error, a path that couldn't be scanned, or a config error
  (bad glob, bad regex, unknown key, invalid `[[custom-rule]]` field).

## Non-goals

- **Not an AI-origin detector.** stopslop never claims code *was* written by
  an AI, only that a pattern in it is junk regardless of origin.
- **Not a correctness checker.** It doesn't run your code or understand what
  it's supposed to do: a stub function that's genuinely fine (e.g. an
  intentionally unimplemented trait default) can still need a suppression
  comment.
- **No member-level hallucination detection.** It can't tell you that
  `requests.get_json()` isn't a real method: that's a type checker's job.
- **No general code-comment style/verbosity grading.** In code files it
  flags specific junk patterns (preamble, attribution, elision), not
  comment style or verbosity in general. Prose files additionally get the
  density/style checks in [Prose linting](#prose-linting) above; those are
  warn-only judgment calls, and none of them are a house-style grader.
- **`[[custom-rule]]` is a phrase matcher, not a plugin system.** It compiles
  a regex against comments/strings or masked prose. There's no hook for
  custom AST shapes, no arbitrary code execution, and no way to write a
  custom rule as sophisticated as, say, SLOP041's document statistics. For
  anything past "flag this phrase," it's still a Rust module in `src/rules`.

## License

MIT (see [LICENSE](https://github.com/mgiovani/ai-stop-slop/blob/main/LICENSE)).

stopslop's own CI fails if its source contains slop: every push runs
`cargo run --quiet -- src` against the project's own code as a gate, alongside
`cargo fmt`, `cargo clippy`, and `cargo test`.
