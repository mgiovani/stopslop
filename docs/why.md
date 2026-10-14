# Why stopslop

Your linter can't see `// ... rest of code unchanged`. ESLint, Ruff, and
Clippy throw away comment and string content before analysis: exactly
where AI coding artifacts live. stopslop reads what they discard: leaked
chat preambles, elision comments that silently deleted code, stray markdown
fences, placeholder credentials, and package imports that don't resolve to
anything you declared. It reads your prose too: Markdown, MDX, plain text,
reST, and HTML get the same deterministic treatment. One static binary, no
LLM at scan time: same input, same output, every run.

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
**quality gate that also names AI tells**: it flags patterns that are junk
regardless of who (or what) produced them, and patterns that are known
giveaways of machine-generated text or code, so you can rewrite either. It
never scores a file as "written by AI" and never needs a model to say so.

## Non-goals

- **Not an AI-origin verdict.** A rule may name a span as a known AI tell so
  you can rewrite it, but stopslop never scores a whole file as "written by
  AI", never runs a model, and never phones home. The verdict stays with the
  reader.
- **Not a correctness checker.** It doesn't run your code or understand what
  it's supposed to do: a stub function that's genuinely fine (e.g. an
  intentionally unimplemented trait default) can still need a suppression
  comment.
- **No member-level hallucination detection.** It can't tell you that
  `requests.get_json()` isn't a real method: that's a type checker's job.
- **No general code-comment style/verbosity grading.** In code files it
  flags specific junk patterns (preamble, attribution, elision, a comment
  that only repeats the statement under it), not comment style, length, or
  density in general. Prose files additionally get the
  density/style checks in [Prose linting](prose.md); those are
  warn-only judgment calls, and none of them are a house-style grader.
- **`[[custom-rule]]` is a phrase matcher, not a plugin system.** It compiles
  a regex against comments/strings or masked prose. There's no hook for
  custom AST shapes or arbitrary code execution. A custom rule also cannot be
  as sophisticated as, say, SLOP041's document statistics. For
  anything past "flag this phrase," it's still a Rust module in `src/rules`.

