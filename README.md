<p align="center">
  <img src="https://raw.githubusercontent.com/mgiovani/stopslop/main/assets/hero.png" alt="stopslop: Like Ruff, but for AI slop. Catch the junk AI leaves in your code.">
</p>

# stopslop

A deterministic linter, written in Rust, for the tells AI leaves in code and prose: 51 rules, one static binary, no model at scan time.

[![CI](https://github.com/mgiovani/stopslop/actions/workflows/ci.yml/badge.svg)](https://github.com/mgiovani/stopslop/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/mgiovani/stopslop/blob/main/LICENSE)
[![Crates.io](https://img.shields.io/crates/v/stopslop.svg)](https://crates.io/crates/stopslop)

[Docs](https://github.com/mgiovani/stopslop/blob/main/docs/README.md) · [Rules](https://github.com/mgiovani/stopslop/blob/main/docs/rules.md) · [Changelog](https://github.com/mgiovani/stopslop/blob/main/CHANGELOG.md)

![stopslop demo](https://raw.githubusercontent.com/mgiovani/stopslop/main/assets/demo.gif)

## Install

```bash
cargo install stopslop
```

From the latest main, or from a local checkout:

```bash
cargo install --git https://github.com/mgiovani/stopslop
cargo install --path .
```

## Usage

```bash
stopslop .                 # lint the current directory
stopslop --since origin/main   # lint only the files changed on this branch
```

```
tests/fixtures/typescript/slop_attribution.ts:1:1 SLOP004 AI attribution / chat-export artifact in comment
tests/fixtures/typescript/slop_elision.ts:3:3 SLOP001 elision comment may have replaced real code
tests/fixtures/markdown/slop_residue.md:3:1 SLOP011 unedited assistant-response residue in prose
    fix: delete the sentence
```

![stopslop findings across Python and TypeScript files](https://raw.githubusercontent.com/mgiovani/stopslop/main/assets/findings.png)

Tier A findings exit 1 and fail CI. Tier B findings warn. See [usage](https://github.com/mgiovani/stopslop/blob/main/docs/usage.md) for formats (text, JSON, SARIF, Markdown), baselines and exit codes.

![stopslop --format json output](https://raw.githubusercontent.com/mgiovani/stopslop/main/assets/formats.png)

## Why stopslop

- **Reads what other linters skip.** ESLint, Ruff, and Clippy treat comment and string content as trivia, which is where `// ... rest of code unchanged` and a leaked "Certainly! Here's..." preamble end up. See [why](https://github.com/mgiovani/stopslop/blob/main/docs/why.md).
- **Deterministic.** Every rule is a tree-sitter AST match or a regex over extracted comment, string or prose text. Same input, same output, with no LLM call, API call, or network access at scan time.
- **CI-ready tiers.** Tier A is mechanical and fails the run; Tier B warns. Tier C stays opt-in until a labelled corpus has scored it.
- **Code, prose, and images.** Source code and documents, plus the metadata of PNG, JPEG and WebP images.
- **English and Brazilian Portuguese.** Phrase panels for both run by default.
- **Measured.** `just corpus` scores every rule against labelled human and AI corpora; see [benchmarks](https://github.com/mgiovani/stopslop/blob/main/docs/benchmarks.md).
- **Fast and quiet to adopt.** One binary and no config to start. `--staged` and `--since` lint only a diff, and a baseline grandfathers old findings.

## What it catches

51 rules in 8 groups. `--list-rules` prints every rule with its group, tier, and state.

| Group | What it covers |
|-------|----------------|
| `artifact` | Mechanical leftovers from a generation session: chat turns, tool tokens, unfilled slots |
| `structure` | Structural code smells: swallowed errors, escaped types, stubs, speculative abstraction |
| `stdlib` | Code that rebuilds something the standard library or the platform already provides |
| `rhetoric` | Formulaic rhetorical shapes: clichés, staged reveals, manufactured significance |
| `verbosity` | Words that cost the reader something and return nothing: hedging, filler, padding |
| `sourcing` | Claims with no checkable source behind them |
| `format` | Typographic and Markdown affectations |
| `provenance` | Embedded image metadata that names how or by what an image was made |

The per-rule table is in [docs/rules.md](https://github.com/mgiovani/stopslop/blob/main/docs/rules.md).

| Tier | Default | Effect |
|------|---------|--------|
| A | on (14 rules) | A finding exits 1 and blocks CI. Mechanical artifacts with no legitimate reading. |
| B | on (32 rules) or off (3 rules) | Judgment calls that warn and never exit 1. `fail-on-tier = "B"` makes them fail. |
| C | off (2 rules) | Thresholds not yet scored against a labelled corpus. Opt in with `--extend-select`. |

## Configuration

```toml
# stopslop.toml
ignore = ["SLOP009", "verbosity"]   # rule codes or groups to subtract
extend-select = ["SLOP033"]         # add a rule on top of the defaults
fail-on-tier = "A"                  # lowest tier that exits 1

[per-file-ignores]
"docs/**" = ["SLOP036"]
```

`stopslop --help-config` prints an annotated file with every key. Details, user-defined rules, and `--check-imports` are in [docs/configuration.md](https://github.com/mgiovani/stopslop/blob/main/docs/configuration.md). To silence one finding, put `// ai-slop-ignore` above it; see [suppression](https://github.com/mgiovani/stopslop/blob/main/docs/suppression.md).

![suppressing a finding with ai-slop-ignore](https://raw.githubusercontent.com/mgiovani/stopslop/main/assets/suppress.gif)

## CI

```bash
stopslop --staged            # pre-commit hook
stopslop --since origin/main # pull request check
stopslop --format sarif .    # GitHub code scanning
```

## Non-goals

- Not an AI-origin verdict. A rule may name a span as a known AI tell so you can rewrite it. stopslop never scores a whole file as written by AI and never runs a model.
- Not a correctness checker. It does not run your code or detect hallucinated methods.
- `[[custom-rule]]` is a phrase matcher, not a plugin system.

The full list is in [docs/why.md](https://github.com/mgiovani/stopslop/blob/main/docs/why.md).

## Contributing

Read [AGENTS.md](https://github.com/mgiovani/stopslop/blob/main/AGENTS.md) for the architecture, the checklist for adding a rule, and the test gates.

## License

MIT (see [LICENSE](https://github.com/mgiovani/stopslop/blob/main/LICENSE)).

stopslop's own CI fails if its source contains slop: every push runs `cargo run --quiet -- .` against the whole repo, prose included, alongside `cargo fmt`, `cargo clippy`, and `cargo test`. Tier A findings fail the build; Tier B findings print as warnings, the same contract users get.
