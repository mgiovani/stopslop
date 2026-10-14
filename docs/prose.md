# Prose linting

stopslop also lints `.md`, `.mdx`, `.txt`, `.rst`, and `.html`/`.htm` files:
same binary, same deterministic regex/structural matching, no LLM involved
in the scan.

HTML is parsed with tree-sitter-html, and the rules see what a reader sees:
visible text, HTML comments, and `href`/`src` attribute values. Everything
else is blanked, including `<script>`, `<style>`, `<pre>`, `<code>`,
`<textarea>`, `<template>`, `<svg>`, `<math>`, and `<noscript>` bodies.
Django/Jinja `{{ … }}`, `{% … %}`, and `{# … #}` are blanked before parsing
too. Each leaf block element (`<p>`, `<div>`, `<blockquote>`, `<td>`) is one
paragraph for the rhythm, recap, synonym, and uniformity rules; list items,
headings, table cells, and form controls never are, and a `<footer>` is
never the ending. `<strong>`/`<b>` count as
bold, inline `<code>` counts as words, and `&mdash;`, `&#8212;`, and the
curly-quote entities are read as the characters they encode. `&ndash;` and
every other entity stay blank.

SLOP011–013 catch mechanical, high-confidence artifacts (leftover chat-turn
phrasing including reasoning-chain scaffolding and acknowledgment loops,
citation-tool tokens, unfilled placeholders) and are Tier A: a finding there
fails the run. SLOP001 (elision comments) also runs on HTML, reading
`<!-- ... -->` the way it reads a code comment on every other lang, and
defers to SLOP013 on a comment the two would otherwise both flag. SLOP014,
SLOP018, and the formulaic-shape rules SLOP022–025 and SLOP029 are on by
default too but Tier B, warn-only: judgment calls the tool still surfaces
every run, just without the power to fail CI. SLOP018
flags every mid-prose dash outright (the one exemption is a block-opening
attribution dash like `— Oscar Wilde`).

The rest are document-wide density and style checks, also on by default: hedging,
overused vocabulary, rhetorical parallelism, boldface overuse,
heading formatting, promotional register, hyphen stacking, sentence length,
synonym rotation, outline filler, change-narrating docs, colon reveals,
filler and adverb density, weak-verb phrasing, dramatic fragmentation, and
mechanical uniformity. SLOP041 is the one rule in this list that measures statistics
(burstiness, vocabulary diversity, trigram repetition) instead of matching a
phrase. It catches templated prose that rotates its vocabulary just enough to
slide past every phrase-based rule. These are judgment calls rather than
mechanical certainties, so they're warn-only. Silence the ones you don't want
by code (`--ignore SLOP015`) or by group (`--ignore verbosity`). Expect some
noise from them: a finding here is a lead to investigate rather than a
verdict. As with the rest of stopslop, this is a writing-quality
gate, not a claim about who or what wrote the text: every rule flags a
concrete pattern (a stale phrase, an unfilled slot, a density threshold,
a document-level statistic), never "this is AI-generated."

Every rule runs every natural-language panel it declares, on every document,
by default: nothing to configure, nothing to detect. `Lang` is the file
syntax (Markdown, MDX, ...); `NatLang` is a separate axis for which
lexicons a rule's phrase panels actually cover, listed per rule in the `NL`
column of the [rule table](rules.md). Structural and statistical rules (AST shape, punctuation,
density) need no lexicon and already run on pt-BR text. Phrase-panel rules
gain a pt-BR panel one rule at a time, each proven by a fixture under
`tests/fixtures/markdown/pt-br/` before the rule may list `pt-BR`; SLOP014
(cliché phrases), SLOP016 (vocabulary), and SLOP032 (hyphenated-compound
overuse) still stay English-only, and
[issue #30](https://github.com/mgiovani/stopslop/issues/30) tracks the rest.
`pt` and `pt-PT` both resolve to the same Brazilian lexicon as `pt-BR`, a
best-effort alias rather than a separate European-Portuguese panel. An HTML
page that declares `<html lang="...">` narrows its own run to that language
when the config allows it (`.rst` docinfo and Markdown front matter carry no
such hint yet). SLOP033's overlong-sentence cap rises to 57 words once a run
is Portuguese-only (`language = "pt-BR"`, measured against English's 50 on
matched corpora). SLOP041's thresholds hold as measured (Portuguese trips
them less often than English, not more); SLOP043 and SLOP029's caps are
still unmeasured for Portuguese, and `language = "pt-BR"` in
`stopslop.toml` restricts a run to the Portuguese panels only.

