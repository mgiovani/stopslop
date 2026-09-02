# stopslop

Deterministic linter for AI slop in code and prose.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run -- .              # dogfood this repo; Tier A findings exit 1
cargo run -- . --list-rules # code, group, tier, default state
cargo install --path .      # rebuild the binary before dogfooding a new rule
```

## Architecture

- `main.rs` calls `cli::run` and returns its exit code. Keep every decision out of it.
- Rule modules import `context`, `diagnostic`, `registry`, `lang`, `prose`, `prose_words`, `suppress`, and sibling rules for shared helpers. Never `cli`, `engine`, `output`, `walk`, or `git`.
- A rule that wants a new fact gets a `LintContext` field computed once per file, never a new argument threaded through `engine`.
- `engine` owns selection (`resolve_enabled`) and per-file dispatch (`lint_file`), `walk` owns the parallel file walk, `output` owns rendering, `cli` wires them together.
- Rules receive source text and an AST index, never a path to open. I/O stays in `walk`, `git`, `config`, `baseline`, and the one manifest scan in `imports_data::DepIndex::discover`, which `cli` runs before the walk.
- `lib.rs` is the public surface. Tests link the library and never shell out to the binary.

## Adding a rule

1. Take the next free `SLOPNNN` and add `src/rules/<tell>.rs` with `pub static RULE: RuleDef` and `fn check`.
2. Declare the module in `src/rules/mod.rs` and the static in `registry::RULES`.
3. Put the code in exactly one `groups.rs` entry. `groups_partition_every_rule` fails otherwise.
4. Open at Tier B. Tier A is for findings with no judgment call in them, because Tier A fails CI.
5. Set `langs` to `lang::CODE_LANGS` or `lang::PROSE_LANGS` unless the rule is narrower, and add fixtures under `tests/fixtures/<lang>/` for every language listed: one file carrying `expect:` markers and one clean file carrying none. `every_declared_lang_has_a_fixture_witness` fails when the marked file is missing.
6. Grep `src/` for each phrase before adding it to a panel. If another rule owns the span, drop the phrase.
7. Keep the panel in the rule file. `prose_words.rs` holds the panels the prose density rules share and takes no new entries.
8. Write the message lowercase and specific, and use `Diagnostic::at_fix` whenever a concrete replacement exists.
9. Install the binary, lint this repo, and fix every finding your rule makes here before opening the PR.

## Engineering principles

These bullets are SOLID, DDD, clean architecture, clean code, and YAGNI applied to this crate.

- One rule detects one tell. A second idea earns a second code, not a second branch inside one `check`.
- Extend the crate by adding a `RuleDef`. The `CheckFn` signature and `engine.rs` stay untouched.
- Every rule honors the same contract: read `ctx`, push into `out`, print nothing, exit nothing, open no file.
- No trait, factory, or builder for a single implementation. Function pointers in a static table cover every rule.
- Keep helpers pure: take slices, return values, leave `check` as the only place that appends diagnostics.
- Treat `RuleDef`, `Diagnostic`, and `ProseDoc` as immutable once built.
- Split a multi-language `check` into named per-language helpers, not one nested match.
- Add no dependency for what `regex`, `std`, or an already-vendored crate does. SLOP037 and SLOP038 exist to say this out loud.
- Add no knob nobody sets. A documented `const` beats a config field with one caller.
- Delete before you add, and put the proof that a branch is dead in the commit message.
- Ship the smallest correct diff.
- Speak the domain vocabulary in code, messages, docs, and commits: Rule, RuleDef, Diagnostic, Tier, Group, Panel, LintContext, NodeIndex, ProseDoc, Baseline, suppression directive, custom rule, Lang.
- Coining a synonym for one of those names is a defect. SLOP034 flags the same habit in prose.

## Comments and docs

- Never write a comment that restates the line under it. SLOP042 flags that pattern in linted code and the crate holds itself to it.
- Doc comments carry the why: why this threshold, why this exclusion, why this ordering.
- Every tuned constant states the evidence that tuned it. See `sentence_length::OVERLONG_WORDS`.
- Record a rejected alternative wherever the next reader would otherwise re-propose it. See `context::NodeIndex`.
- Rename the thing before annotating it. A comment is not a fix for an unclear name.
- Mark a deliberate shortcut with a `ponytail:` comment naming its ceiling and upgrade path.
- No rule message and no doc line may claim AI origin. This is a quality gate, and the README non-goals stay true.

## Testing

- Unit tests live in the rule file under `#[cfg(test)]` with a `diagnostics_for(src)` helper copied from a neighboring rule.
- `tests/integration.rs` is frozen harness code. Add fixture files, never harness branches.
- Mark expectations inline with `expect: SLOP0NN` on the offending line, or `expect-line: NN SLOP0NN` when the anchor sits elsewhere.
- Pair every positive fixture with a clean one. A rule that cannot stay quiet is unfinished.
- Test the exclusions you wrote, not the match you already saw work.
- Treat the dogfood run as the third gate after unit and fixture tests.

## Invariants

- Panel disjointness: no span may be flagged by two rules. Grep first, then drop the phrase whose owner already exists.
- `groups.rs` partitions `RULES` exactly. `ALL` is special-cased inside `groups::expand` and never joins the table.
- Tier and `default_on` are separate axes: A plus on blocks CI, B plus on warns, B plus off is opt-in.
- Custom codes (`SLOP900` and up) come from user config, so they never appear in `RULES` or `GROUPS`.
- Call `paths::strip_dot_slash` on every new glob surface, because `.` and `docs` yield different display paths.
- Parse every `ai-slop-ignore` directive through `suppress::comment_body`, never with an ad-hoc prefix strip.

## Performance traps

- `next_named_sibling()` is O(index). Iterate `named_children` once instead of asking for siblings inside a loop.
- Query the shared index through `ctx.nodes(&["kind"])`. A rule never walks the tree again.
- `ProseDoc` memoizes line lookups in a `Cell` and is therefore `!Sync`. Keep it inside the per-file scope, since the walk is parallel.
- A Unicode `\b` under `(?i)` drops the regex crate to PikeVM on non-ASCII input. Write `(?-u:\b)` on ASCII-only panels. See issue #21.
- Compile every regex in a module-scope `LazyLock`, never inside `check`.

## Workflow

- Branch from `main`, one topic per branch, prefixed `feat/`, `fix/`, `perf/`, `docs/`, or `ci/`.
- Write conventional commit subjects (`feat:`, `fix:`, `perf:`, `refactor:`, `docs:`, `test:`, `ci:`, `chore:`).
- Self-review the whole patch and run the three CI commands plus the dogfood run before pushing.
- Open a PR and let CI decide. Merge commits, no squash.
- Update the README rule table and `CHANGELOG.md` in the same PR that adds or retires a rule.
- Touch `Cargo.toml` dependencies only with a PR-body line saying what std or an existing crate could not do.
