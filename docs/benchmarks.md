# Benchmarks

## Measured on labelled corpora

`just corpus` runs every rule against labelled human-vs-AI corpora and
rebuilds the report. For every rule, per code language and per natural language, it
prints the hit rate on the human split (a false-positive proxy), the hit rate
on the AI split (a recall proxy), precision at a 1:1 human/AI prior, and
findings per KLoC or per thousand words.

```bash
just corpus          # fetch, score, analyse, render
just corpus-fetch    # download and materialize the cells (the only step needing the network)
just corpus-score    # lint them, rebuild bench/corpus_report.md, write results.json
just corpus-analyze  # measure bench/candidates.toml, split rates by message, rank phrases
just corpus-html     # render target/corpus/report.html
```

Each recipe is one command; run `just --list` to see them, or read the
[justfile](../justfile) and run the underlying `python3 bench/...` line by hand.
The corpus recipes split on the network boundary: `corpus-fetch` downloads and
materializes every cell and writes `target/corpus/fetched.json`; `corpus-score`
reads that manifest and never touches the network, so scoring a rule change
is offline and reproducible. Set `HF_TOKEN` (or `HF_API_TOKEN`) for the
Hugging Face datasets-server.
Datasets are never committed, and neither is the rendered HTML, because several
registered datasets forbid redistributing a derivative. The committed
[bench/corpus_report.md](../bench/corpus_report.md) is the last run's output.

### What counts as a human split

A split is read as human only when its text is dated 2019 or earlier, or was
written by identified people under controlled conditions. Everything else was
collected once coding assistants and chat models were in general use, and
nobody can guarantee its human side was written without one.

Splits that clear the bar are `pinned-2019` (a 2019 release tag, or a corpus
built from pre-2020 sources) or `verified-authors` (exam essays written under
supervision, professional annotators writing to a brief). Splits that do not
are `unverified`: they are still fetched, scored and reported in full, but
below a divider that says why, and they never enter any pooled number or
takeaway.

Every AI split carries the years of the models that wrote it, so a 2022 result
is never read as a statement about a 2025 model.

### Candidate tells

`bench/candidates.toml` holds proposed tells that are not rules: a regex, a
scope, and what the corpus said when `just corpus-analyze` measured it. A
candidate that separates the splits earns an issue and then a rule with
fixtures; one that does not stays in the file as the record of a measurement,
next to the tells already measured and dropped. Nothing in that file changes
what `stopslop` reports.

Datasets in the registry, none committed to this repo:

| Name | What it covers | Human split | Generators | License |
|---|---|---|---|---|
| Go 1.13 src | Go | pinned-2019 | n/a | BSD-3-Clause |
| CPython 3.8.0 Lib and Doc | Python and reStructuredText | pinned-2019 | n/a | PSF-2.0 |
| Rust 1.40.0 std | Rust | pinned-2019 | n/a | MIT or Apache-2.0 |
| TypeScript 3.7.2 src | TypeScript | pinned-2019 | n/a | Apache-2.0 |
| Ant Design 3.26.0 | TSX | pinned-2019 | n/a | MIT |
| naples-code | Python, human and AI functions paired in one corpus, docstring in a separate column | pinned-2019 | gpt-3.5-turbo, DeepSeek-Coder-Instruct-33B, Qwen2.5-Coder-Instruct-32B, 2023-24 | CC-BY-4.0 |
| The Rust Programming Language book | English Markdown | pinned-2019 | n/a | MIT or Apache-2.0 |
| React docs | English Markdown | pinned-2019 | n/a | CC-BY-4.0 |
| Kubernetes docs | English Markdown | pinned-2019 | n/a | CC-BY-4.0 |
| HC3 | English answers | pinned-2019 (ELI5, FiQA, WikiQA) | ChatGPT, 2022 | CC-BY-SA-4.0 |
| MAGE | English documents from ten corpora | pinned-2019 (SciGen dropped) | 27 models, 2019-2022 | Apache-2.0 on the card, CC-BY-4.0 in the repo |
| Ghostbuster data | English news and stories | pinned-2019 (Reuters, WritingPrompts) | gpt-3.5, claude, 2023 | CC-BY-3.0 |
| SemEval-2024 Task 8 | English text from PeerRead, arXiv, Reddit and WikiHow | pinned-2019 (Wikipedia dropped) | ChatGPT, GPT-3 davinci, Cohere, Dolly, 2023 | Apache-2.0 |
| GPT-2 output dataset | English web text (WebText) | pinned-2019 | GPT-2 1542M, 2019 | MIT |
| Beemo | English instruction responses | verified-authors (No Robots) | GPT-4o, Llama 3.1 70B, Mixtral, Gemma, Mistral 7B, Zephyr, 2023-24 | MIT (edits); prompts and human outputs CC-BY-NC-4.0 |
| Diplomatrix-BR | Brazilian Portuguese exam essays | verified-authors | GPT-4o, Claude 3, Gemini, Llama 3, Sabiá and others, 2024 | MIT |
| LeNER-Br | Brazilian Portuguese statutes and court rulings | pinned-2019 | n/a | MIT (packaging); the raw texts are public-domain Brazilian federal statutes and court rulings (Lei 9.610/98 Art. 8) |
| DroidCollection | code: Python, Go, JavaScript as .ts, Rust adversarial | unverified | open and API code models, 2024-25 | not stated on the card |
| SemEval-2026 Task 13 | code: Python, Go, JavaScript as .ts, hybrid and adversarial | unverified | Qwen2.5-Coder, DeepSeek-Coder, Llama 3.x, GPT-4o and others, 2024-25 | Apache-2.0 |
| CodeMirage | code: Python, Go, JavaScript as .ts, plus paraphrased | unverified | ten LLMs, 2025 | CC-BY-NC-ND-4.0 |
| AIGCodeSet | Python | unverified | CodeLlama, Codestral, Gemini 1.5, 2024 | CDLA-Permissive-2.0 |
| CoDET-M4 | Python, comments stripped by the publisher | unverified | GPT-4o, Llama 3, Qwen, 2024 | MIT |
| Rosetta Code | Python, Go, Rust, TypeScript | unverified | n/a | GFDL |
| H-AIRosettaMP | Python, Go, Rust, TypeScript, AI side translates a human Rosetta Code solution from another language | unverified | StarCoder2, 2024 (ai-translated) | MIT |
| FAIDSet | English text, plus a human-LLM collaborative split | unverified | GPT-4o, Gemini 2, Llama 3, DeepSeek, 2024-25 | MIT |
| PAN25 Voight-Kampff | English fiction, essay and news text, plus machine-paraphrased | unverified | 23 models incl. gpt-4o, o3-mini, gemini-2.0-flash, deepseek-r1, llama-3.3-70b, 2023-25 | research use only, no redistribution (Zenodo record terms) |
| AIDev | English pull-request descriptions, AI only | none | Claude Code, Codex, Cursor, Devin, Copilot, Jules, 2025 | CC-BY-4.0 |
| APT-Eval | English text polished to a stated degree | unverified | GPT-4o, Llama 3.1 70B, Llama 3 8B, DeepSeek-V3, 2024-25 | CC-BY-4.0 |
| WETBench | Portuguese Wikipedia paragraphs | unverified | GPT-4o mini, Gemini 2.0, Qwen2.5, Mistral, 2024 | CC-BY-NC-SA-4.0 |
| Essay-BR | Brazilian Portuguese student essays | unverified | n/a | MIT |

No dataset anywhere pairs human and AI-written TSX or real TypeScript: the
`typescript` cells drawn from DroidCollection, SemEval-2026 Task 13, and
CodeMirage are JavaScript the source dataset stored with a `.ts` extension,
and no registered dataset carries TSX at all, so TSX AI stays synthesized.
Across Google, DeepMind, Meta, OpenAI, Microsoft, GitHub, AI2, BigCode,
Salesforce, Amazon, IBM, NVIDIA, DeepSeek, Alibaba, and Mistral, the only
labelled human-vs-AI corpus any of them publishes is OpenAI's 2019 GPT-2
output dataset.

`just corpus-generate` (`uv run`, needs an Anthropic API key, costs money)
calls Claude to synthesize the AI split for a cell that has no public one. Its
output is raw model text, every synthesized file is marked as such on disk
under `synth-*`, and those five cells are the only source of plain AI Rust and
the only source of a TSX AI split.

A hit rate here is evidence that a tell appears in a corpus, on a human split
or an AI split, and it is never a claim that any single file was AI-written.

