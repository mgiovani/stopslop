## Per-rule hit rate across the corpus registry

- Binary: stopslop 0.5.1, run as `stopslop <dir> --format json --stats --no-config --select ALL`
- --limit: 500  **-- a deterministic spread sample, never a reported number, when non-zero and below a dataset's full size**
- aidev revision: c63c8a57a2de
- aigcodeset revision: b4d4e69fdbcb
- antd-tsx revision: 3.26.0
- apt-eval revision: 1a183126ec24
- beemo revision: 9c014107fe9b
- codemirage revision: 174c25ffa9e9
- codet_m4 revision: 4d4e665037cb
- cpython-doc revision: v3.8.0
- cpython-lib revision: v3.8.0
- diplomatrix revision: n/a
- droid revision: 9a42843be994
- essay-br revision: n/a
- faidset revision: e2927dd1218b
- ghostbuster revision: 86ebd7259055
- go-std revision: go1.13
- gpt2-output revision: n/a
- hairosetta revision: 2bd3dc134a28
- hc3 revision: 4d0ff18143b5
- k8s-docs revision: snapshot-initial-v1.17
- lener-br revision: 4999cb7f6319
- mage revision: 342663f0a2b7
- naples-code revision: n/a
- pan25 revision: n/a
- react-docs revision: f90d199a61c1
- rosetta revision: 11d8b38cbd90
- rust-book revision: be91ce0d3e63
- rust-std revision: 1.40.0
- semeval13 revision: df2aec18238a
- semeval24-m4 revision: 509a1a6a33f9
- typescript-src revision: v3.7.2
- wetbench-pt revision: n/a

### How to read this

- `human hits` is the share of human files where the rule fired. Under "the rule firing predicts AI-authored" that share **is** the false-positive rate.
- `precision @1:1` assumes one ai file per human file, a property of this table's construction, not of any repository; it re-expresses the two hit rates and carries no information they do not. `precision @prior` instead uses the raw counts actually fetched, so it reflects this run's `--limit` and each cell's real population. `--` means neither side fired.
- `lift` is the ai hit rate divided by the human hit rate: `inf` means the rule is clean on human files here and not on ai ones; `--` means neither side fired.
- Findings per KLoC/1k words only mean something read against the hit rate: a high hit rate next to a modest density is a rule weakly present everywhere, and a low hit rate next to a high density is a rule that fires hard in a few files.
- A `*` marks a precision built from fewer than 20 hits in total. Read those off the counts, not the ratio.
- The `any rule` row is the OR of every applicable rule for that lang, so its precision tracks the noisiest member rather than the best one; `any Tier A rule` is the same OR restricted to rules CI actually fails on.
- A hit rate is evidence that the tells appear in that corpus. It is not a claim that any file is AI-written, and this crate ships no such claim.
- SLOP010 names every code `Lang` too, but its check needs a dependency manifest a per-file corpus cell has no reason to carry, so it can never fire here and the tables leave it out rather than show a rule that structurally cannot score.
- Hugging Face cells are sampled through `/filter` when the datasets-server index answers and through a spread `/rows` scan when it does not. Both are deterministic, and a `scan_*` marker in the page cache pins a scanned cell to the scan on later runs, so reruns reproduce this file byte for byte; a cold cache under the other path draws a different sample of the same population.

### Which human splits count as human

A split is read as human only when its text is dated 2019 or earlier (`pinned-2019`) or was written by identified people under controlled conditions (`verified-authors`). Every other split is `unverified` and reported below a divider.

| dataset | human split | source and years | generators |
|---|---|---|---|
| aidev | ai only | -- | Claude Code, OpenAI Codex, Cursor, Devin, Copilot and Jules agents (2025) |
| aigcodeset | unverified | IBM CodeNet submissions, collected through 2020 and published 2021; AtCoder and AIZU users, unverified for assistant use | CodeLlama-34B, Codestral-22B, Gemini 1.5 Flash (2024) |
| antd-tsx | pinned-2019 | Ant Design 3.26.0 components, tagged December 2019 | -- |
| apt-eval | unverified | human blog, email, news, review and speech texts drawn from MixSet and similar 2024 collections | GPT-4o, Llama 3.1 70B, Llama 3 8B and DeepSeek-V3 as polishers (2024-25) |
| beemo | verified-authors | No Robots (2023): responses written by expert annotators to the same prompts the models answered | GPT-4o, Llama 3.1 70B, Mixtral, Gemma, Mistral 7B, Zephyr (2023-24) |
| codemirage | unverified | CodeParrot github-code-clean, a GitHub snapshot from May 2022 | ten LLMs, 2025 (source column) |
| codet_m4 | unverified | LeetCode and Codeforces solutions, undated, plus CodeSearchNet (2019) | GPT-4o, Llama 3, Qwen and other 2024 models (model column) |
| cpython-doc | pinned-2019 | CPython 3.8.0 Doc, tagged October 2019 | -- |
| cpython-lib | pinned-2019 | CPython 3.8.0 Lib, tagged October 2019 | -- |
| diplomatrix | verified-authors | CACD diplomatic-career exam essays, handwritten by identified candidates under supervision | GPT-4o, Claude 3, Gemini, Llama 3, Sabiá and other 2024 models |
| droid | unverified | GitHub, LeetCode and Codeforces code collected 2024-25; the paper says the human class may contain assistant-written code | open and API code models, 2024-25 (Generator column) |
| essay-br | unverified | ENEM-style student essays published on a correction site 2015-2020; the 2020 tail postdates the bar and authors are anonymous | -- |
| faidset | unverified | undated academic theses and essays collected 2024-25; English rows only | GPT-4o, Gemini 2, Llama 3 and DeepSeek V3/R1 (2024-25; model column) |
| ghostbuster | pinned-2019 | Reuters 50-50 news (1996-97) and r/WritingPrompts stories (2017-18); the undated IvyPanda essays are dropped from the human split | gpt-3.5-turbo and claude (2023) |
| go-std | pinned-2019 | Go 1.13 standard library, tagged September 2019 | -- |
| gpt2-output | pinned-2019 | WebText: Reddit-outbound links with karma >= 3, scraped through December 2017 | GPT-2 1542M (2019) |
| hairosetta | unverified | Rosetta Code wiki solutions, retrieved 2022-07-01, after Copilot shipped | StarCoder2 (2024) |
| hc3 | pinned-2019 | ELI5 (2019), FiQA (2018) and WikiQA (2015) answers; the wiki_csai (2022) and medicine (2020) human rows are dropped, their ChatGPT rows kept | ChatGPT (December 2022) |
| k8s-docs | pinned-2019 | kubernetes/website English docs, tagged December 2019 | -- |
| lener-br | pinned-2019 | Brazilian federal statutes and court rulings dated 2008-2018 by their own text; the repo's own commit history only starts 2020-05-15, so the cutoff rests on the documents' stated dates, not the clone | -- |
| mage | pinned-2019 | nine pre-2020 corpora (CMV, Yelp, XSum, TLDR, ELI5, WritingPrompts, ROC, HellaSwag, SQuAD); the SciGen human rows (arXiv through 2021) are dropped | 27 LLMs from GPT-J to GPT-3.5 and LLaMA (2019-2022) |
| naples-code | pinned-2019 | HMCorp: 16,928 non-forked GitHub repos sorted by stars, filtered from CodeSearchNet (2019) | gpt-3.5-turbo (2023), DeepSeek-Coder-Instruct-33B and Qwen2.5-Coder-Instruct-32B (2024) |
| pan25 | unverified | fiction, essay and news human text; the fiction/essay sources could not be confirmed, the news text is dated 2021 | 23 models spanning 2023-2025, including gpt-4o, o3-mini, gemini-2.0-flash, deepseek-r1-distill-qwen-32b, llama-3.3-70b-instruct and gpt-4.5-preview (model column) |
| react-docs | pinned-2019 | react.dev docs at their last default-branch commit before 2020-01-01 | -- |
| rosetta | unverified | Rosetta Code wiki solutions in a 2022-23 snapshot; individual edits are undated | -- |
| rust-book | pinned-2019 | rust-lang/book at its last default-branch commit before 2020-01-01 | -- |
| rust-std | pinned-2019 | Rust 1.40.0 libstd, libcore and liballoc, tagged December 2019 | -- |
| semeval13 | unverified | Droid-derived GitHub, LeetCode and Codeforces code collected 2024-25 | Qwen2.5-Coder, DeepSeek-Coder, Llama 3.x, GPT-4o and other 2024-25 models (generator column) |
| semeval24-m4 | pinned-2019 | PeerRead (2007-2017) plus pre-2020 arXiv, Reddit and WikiHow text; the wikipedia human rows are dropped, their machine rows kept, the same treatment HC3's wiki_csai config gets | ChatGPT, GPT-3 davinci, Cohere, Dolly (2023) |
| typescript-src | pinned-2019 | TypeScript 3.7.2 compiler sources, tagged November 2019 | -- |
| wetbench-pt | unverified | Portuguese Wikipedia paragraphs from 2024 revisions | GPT-4o mini, Gemini 2.0 Flash, Qwen2.5-7B, Mistral-7B (2024) |

## Summary, by lang: verified human splits

#### python

| rule | cpython-lib H% | cpython-lib A% | naples-code H% | naples-code A% |
|---|---|---|---|---|
| SLOP001 | 0.00% |  | n/a | n/a |
| SLOP002 | 0.40% |  | n/a | n/a |
| SLOP003 | 0.00% |  | n/a | n/a |
| SLOP004 | 0.00% |  | n/a | n/a |
| SLOP006 | 29.46% |  | 2.20% | 1.60% |
| SLOP008 | 9.02% |  | 0.00% | 4.60% |
| SLOP009 | 0.60% |  | 0.00% | 1.00% |
| SLOP037 | 4.41% |  | 1.20% | 1.40% |
| SLOP039 | 1.00% |  | 0.00% | 0.20% |
| SLOP040 | 0.80% |  | 0.00% | 0.00% |
| SLOP042 | 8.02% |  | n/a | n/a |
| SLOP043 | 30.26% |  | n/a | n/a |
| **any Tier A rule** | 34.67% |  | 2.20% | 7.00% |
| **any rule** | 48.70% |  | 7.40% | 14.40% |

#### go

| rule | go-std H% | go-std A% |
|---|---|---|
| SLOP001 | 0.20% |  |
| SLOP002 | 0.20% |  |
| SLOP003 | 0.00% |  |
| SLOP004 | 0.00% |  |
| SLOP005 | 0.00% |  |
| SLOP008 | 4.40% |  |
| SLOP009 | 0.20% |  |
| SLOP037 | 4.00% |  |
| SLOP039 | 2.40% |  |
| SLOP042 | 4.60% |  |
| SLOP043 | 21.00% |  |
| **any Tier A rule** | 5.00% |  |
| **any rule** | 29.60% |  |

#### rust

| rule | rust-std H% | rust-std A% |
|---|---|---|
| SLOP001 | 0.00% |  |
| SLOP002 | 0.00% |  |
| SLOP003 | 0.00% |  |
| SLOP004 | 0.00% |  |
| SLOP005 | 3.20% |  |
| SLOP008 | 0.00% |  |
| SLOP009 | 0.20% |  |
| SLOP037 | 0.00% |  |
| SLOP039 | 3.20% |  |
| SLOP042 | 1.20% |  |
| SLOP043 | 24.20% |  |
| **any Tier A rule** | 3.40% |  |
| **any rule** | 27.40% |  |

Plain ai Rust in this table, when present, comes only from the synthesized `synth-rust` cell; no real-world dataset registered here ships plain machine-generated Rust (see the droid caveats below).

#### typescript

| rule | typescript-src H% | typescript-src A% |
|---|---|---|
| SLOP001 | 0.00% |  |
| SLOP002 | 0.00% |  |
| SLOP003 | 0.00% |  |
| SLOP004 | 0.00% |  |
| SLOP005 | 1.91% |  |
| SLOP007 | 10.05% |  |
| SLOP008 | 0.48% |  |
| SLOP009 | 0.00% |  |
| SLOP037 | 1.44% |  |
| SLOP038 | 0.00% |  |
| SLOP039 | 4.31% |  |
| SLOP040 | 2.87% |  |
| SLOP042 | 16.75% |  |
| SLOP043 | 34.93% |  |
| **any Tier A rule** | 11.00% |  |
| **any rule** | 39.71% |  |

#### tsx

| rule | antd-tsx H% | antd-tsx A% |
|---|---|---|
| SLOP001 | 0.00% |  |
| SLOP002 | 0.00% |  |
| SLOP003 | 0.00% |  |
| SLOP004 | 0.00% |  |
| SLOP005 | 0.20% |  |
| SLOP007 | 5.40% |  |
| SLOP008 | 0.00% |  |
| SLOP009 | 0.00% |  |
| SLOP037 | 0.00% |  |
| SLOP038 | 2.20% |  |
| SLOP039 | 0.00% |  |
| SLOP040 | 0.00% |  |
| SLOP042 | 0.40% |  |
| SLOP043 | 0.00% |  |
| **any Tier A rule** | 5.40% |  |
| **any rule** | 7.40% |  |

#### prose

| rule | beemo H% | beemo A% | cpython-doc H% | cpython-doc A% | diplomatrix H% | diplomatrix A% | ghostbuster H% | ghostbuster A% | gpt2-output H% | gpt2-output A% | hc3 H% | hc3 A% | k8s-docs H% | k8s-docs A% | lener-br H% | lener-br A% | mage H% | mage A% | react-docs H% | react-docs A% | rust-book H% | rust-book A% | semeval24-m4 H% | semeval24-m4 A% |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| SLOP011 | 1.20% | 24.20% | 2.49% |  | 0.00% | 0.00% | 1.20% | 0.00% | 2.40% | 1.20% | 0.40% | 1.20% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 5.56% |  | 1.90% |  | 0.80% | 3.40% |
| SLOP012 | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP013 | 0.60% | 2.00% | 0.42% |  | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.20% | 0.20% |  | 0.00% |  | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.20% |
| SLOP014 | 0.60% | 2.80% | 0.00% |  | 0.00% | 1.28% | 0.20% | 8.80% | 0.80% | 0.60% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.20% | 0.20% |
| SLOP015 | 0.00% | 0.80% | 1.04% |  | 0.00% | 2.31% | 0.60% | 2.20% | 0.40% | 0.60% | 0.40% | 0.40% | 0.80% |  | 14.29% |  | 0.80% | 0.40% | 1.11% |  | 0.95% |  | 1.40% | 2.20% |
| SLOP016 | 0.20% | 0.60% | 0.00% |  | 0.00% | 0.51% | 0.00% | 12.00% | 0.20% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.60% |
| SLOP017 | 1.80% | 6.20% | 11.23% |  | 7.95% | 22.05% | 1.80% | 22.20% | 3.20% | 2.00% | 0.00% | 3.80% | 4.00% |  | 12.86% |  | 0.40% | 1.40% | 8.89% |  | 7.62% |  | 9.80% | 9.80% |
| SLOP018 | 5.40% | 1.80% | 19.96% |  | 14.77% | 2.05% | 23.20% | 14.00% | 35.00% | 41.20% | 5.00% | 0.20% | 3.20% |  | 11.43% |  | 0.00% | 0.00% | 26.67% |  | 20.95% |  | 8.80% | 6.60% |
| SLOP019 | 0.00% | 3.80% | 0.00% |  | 0.00% | 0.77% | 0.40% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 1.20% |  | 0.00% |  | 0.00% | 0.00% | 2.22% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP020 | 8.40% | 1.60% | 0.42% |  | 65.91% | 2.05% | 1.20% | 14.00% | 0.00% | 0.00% | 0.40% | 0.00% | 3.80% |  | 27.14% |  | 0.00% | 0.00% | 7.78% |  | 98.10% |  | 11.60% | 21.60% |
| SLOP021 | 0.00% | 0.00% | 0.00% |  | 0.00% | 1.54% | 0.00% | 0.00% | 0.00% | 0.20% | 0.00% | 0.00% | 24.40% |  | 0.00% |  | 0.00% | 0.00% | 7.78% |  | 55.24% |  | 0.00% | 0.00% |
| SLOP022 | 1.60% | 0.80% | 0.83% |  | 2.27% | 7.44% | 5.80% | 2.60% | 2.60% | 4.20% | 0.00% | 0.20% | 0.20% |  | 0.00% |  | 1.40% | 4.80% | 1.11% |  | 0.95% |  | 2.80% | 0.40% |
| SLOP023 | 0.00% | 0.20% | 0.00% |  | 0.00% | 0.00% | 1.20% | 0.00% | 0.60% | 0.20% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.20% | 0.20% | 0.00% |  | 0.00% |  | 0.00% | 0.20% |
| SLOP024 | 0.00% | 1.20% | 4.78% |  | 0.00% | 11.54% | 1.20% | 8.60% | 1.60% | 1.40% | 1.20% | 0.80% | 2.40% |  | 0.00% |  | 0.60% | 1.80% | 10.00% |  | 1.90% |  | 3.20% | 3.00% |
| SLOP025 | 0.20% | 0.60% | 0.62% |  | 0.00% | 1.03% | 2.20% | 5.00% | 1.40% | 1.00% | 0.20% | 0.60% | 0.20% |  | 1.43% |  | 0.40% | 1.80% | 1.11% |  | 0.00% |  | 0.40% | 0.60% |
| SLOP026 | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP027 | 0.20% | 0.20% | 6.65% |  | 0.00% | 0.51% | 0.00% | 1.40% | 1.00% | 1.20% | 0.00% | 1.00% | 4.00% |  | 4.29% |  | 0.00% | 0.80% | 5.56% |  | 2.86% |  | 3.00% | 5.20% |
| SLOP028 | 0.00% | 1.00% | 2.08% |  | 0.00% | 0.00% | 0.00% | 2.20% | 0.00% | 0.00% | 0.00% | 0.40% | 1.00% |  | 0.00% |  | 0.00% | 0.20% | 0.00% |  | 1.90% |  | 1.00% | 0.80% |
| SLOP029 | 0.40% | 0.40% | 0.00% |  | 0.00% | 0.26% | 0.00% | 0.00% | 0.20% | 0.00% | 0.20% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP030 | 12.80% | 6.80% | 46.15% |  | 29.55% | 1.28% | 65.00% | 34.00% | 16.00% | 14.00% | 9.00% | 9.00% | 3.20% |  | 55.71% |  | 22.60% | 30.60% | 7.78% |  | 15.24% |  | 27.00% | 23.40% |
| SLOP031 | 0.20% | 0.60% | 0.00% |  | 0.00% | 0.26% | 0.00% | 0.40% | 0.00% | 0.40% | 0.00% | 0.00% | 0.20% |  | 0.00% |  | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.80% |
| SLOP032 | 0.00% | 0.20% | 1.04% |  | 0.00% | 0.00% | 0.00% | 2.20% | 0.60% | 0.00% | 0.20% | 0.40% | 0.20% |  | 0.00% |  | 0.00% | 0.00% | 1.11% |  | 0.00% |  | 0.20% | 0.20% |
| SLOP033 | 2.60% | 2.20% | 26.20% |  | 38.64% | 6.41% | 22.40% | 5.60% | 29.40% | 36.40% | 9.80% | 21.80% | 5.20% |  | 91.43% |  | 10.00% | 31.40% | 5.56% |  | 17.14% |  | 27.00% | 15.00% |
| SLOP034 | 0.00% | 0.40% | 31.19% |  | 0.00% | 0.00% | 2.20% | 0.60% | 1.60% | 1.40% | 0.60% | 0.00% | 4.20% |  | 1.43% |  | 0.40% | 1.20% | 2.22% |  | 19.05% |  | 5.80% | 5.00% |
| SLOP035 | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.77% | 0.00% | 1.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP036 | 1.40% | 0.40% | 11.85% |  | 0.00% | 0.00% | 2.00% | 1.00% | 0.80% | 2.40% | 0.40% | 0.40% | 2.00% |  | 0.00% |  | 0.60% | 1.60% | 2.22% |  | 0.95% |  | 1.00% | 0.40% |
| SLOP041 | 2.20% | 10.20% | 46.15% |  | 25.00% | 71.79% | 3.40% | 31.40% | 5.80% | 2.60% | 2.80% | 19.20% | 32.20% |  | 12.86% |  | 0.80% | 14.00% | 34.44% |  | 61.90% |  | 14.80% | 55.60% |
| **any Tier A rule** | 1.60% | 26.00% | 2.70% |  | 0.00% | 0.00% | 1.20% | 0.00% | 2.40% | 1.20% | 0.40% | 1.20% | 0.20% |  | 0.00% |  | 0.00% | 0.00% | 5.56% |  | 1.90% |  | 0.80% | 3.60% |
| **any rule** | 29.40% | 47.00% | 78.38% |  | 92.05% | 87.95% | 83.40% | 70.40% | 60.20% | 64.60% | 23.00% | 42.80% | 57.60% |  | 98.57% |  | 30.40% | 59.20% | 65.56% |  | 98.10% |  | 60.80% | 83.00% |

---

The splits below were collected after coding assistants and chat models came into general use, so nobody can guarantee their human side was written without one unless the dataset's authors verified it. They are fetched, scored and read exactly like the splits above, but they stay out of the pooled human rates, the lift and precision columns, and the takeaways.

## Summary, by lang: unverified human splits

#### python

| rule | aigcodeset H% | aigcodeset A% | codemirage H% | codemirage A% | codet_m4 H% | codet_m4 A% | droid H% | droid A% | hairosetta H% | hairosetta A% | rosetta H% | rosetta A% | semeval13 H% | semeval13 A% |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| SLOP001 | 0.00% | 0.00% | 0.00% | 0.00% | n/a | n/a | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.20% |
| SLOP002 | 0.00% | 0.00% | 0.00% | 0.00% | n/a | n/a | 0.00% | 0.60% | 0.20% |  | 0.20% |  | 0.00% | 0.00% |
| SLOP003 | 0.00% | 0.20% | 0.00% | 5.00% | n/a | n/a | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP004 | 0.00% | 0.00% | 0.00% | 0.00% | n/a | n/a | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP006 | 0.20% | 0.40% | 5.20% | 13.40% | 2.00% | 0.00% | 2.40% | 7.00% | 1.80% |  | 2.60% |  | 2.60% | 5.40% |
| SLOP008 | 0.00% | 0.00% | 4.60% | 4.00% | 0.40% | 1.60% | 1.00% | 1.60% | 0.80% |  | 1.00% |  | 1.00% | 3.00% |
| SLOP009 | 0.00% | 0.00% | 0.60% | 5.00% | 0.20% | 1.20% | 0.20% | 5.60% | 1.40% |  | 0.60% |  | 0.20% | 7.20% |
| SLOP037 | 5.40% | 4.20% | 1.60% | 2.60% | 3.20% | 7.00% | 4.20% | 4.00% | 1.40% |  | 1.20% |  | 3.40% | 3.40% |
| SLOP039 | 1.00% | 0.40% | 0.20% | 1.60% | 0.80% | 0.20% | 0.20% | 0.00% | 0.00% |  | 0.40% |  | 0.40% | 0.20% |
| SLOP040 | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.40% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP042 | 0.20% | 0.40% | 6.40% | 13.60% | n/a | n/a | 1.60% | 13.20% | 1.60% |  | 1.20% |  | 0.80% | 13.80% |
| SLOP043 | 0.00% | 0.00% | 5.80% | 0.20% | n/a | n/a | 0.40% | 1.20% | 0.40% |  | 0.20% |  | 0.00% | 0.40% |
| **any Tier A rule** | 0.20% | 0.60% | 10.00% | 24.60% | 2.60% | 2.80% | 3.60% | 12.80% | 4.00% |  | 4.20% |  | 3.40% | 14.60% |
| **any rule** | 6.80% | 5.60% | 22.80% | 37.40% | 8.80% | 15.20% | 10.00% | 26.60% | 7.40% |  | 7.20% |  | 7.60% | 27.80% |

#### go

| rule | codemirage H% | codemirage A% | droid H% | droid A% | hairosetta H% | hairosetta A% | rosetta H% | rosetta A% | semeval13 H% | semeval13 A% |
|---|---|---|---|---|---|---|---|---|---|---|
| SLOP001 | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP002 | 0.00% | 0.20% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.40% |
| SLOP003 | 0.00% | 7.40% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP004 | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP005 | 0.20% | 0.20% | 0.20% | 0.40% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP008 | 1.20% | 2.20% | 0.20% | 0.40% | 0.40% |  | 0.80% |  | 0.20% | 0.00% |
| SLOP009 | 1.20% | 7.80% | 0.40% | 6.40% | 1.00% |  | 0.80% |  | 0.60% | 6.00% |
| SLOP037 | 9.80% | 7.60% | 2.00% | 7.00% | 2.80% |  | 3.60% |  | 3.00% | 5.80% |
| SLOP039 | 0.40% | 1.80% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.20% | 0.00% |
| SLOP042 | 6.00% | 11.60% | 0.60% | 13.20% | 2.80% |  | 2.40% |  | 1.60% | 11.60% |
| SLOP043 | 2.40% | 0.20% | 0.00% | 0.00% | 1.60% |  | 1.80% |  | 0.40% | 0.60% |
| **any Tier A rule** | 2.40% | 17.20% | 0.80% | 7.00% | 1.40% |  | 1.60% |  | 0.80% | 6.40% |
| **any rule** | 18.60% | 33.40% | 3.40% | 23.80% | 7.80% |  | 9.20% |  | 6.00% | 22.20% |

#### rust

| rule | droid H% | droid A% | hairosetta H% | hairosetta A% | rosetta H% | rosetta A% |
|---|---|---|---|---|---|---|
| SLOP001 | 0.00% |  | 0.00% |  | 0.00% |  |
| SLOP002 | 0.00% |  | 0.00% |  | 0.00% |  |
| SLOP003 | 0.00% |  | 0.00% |  | 0.00% |  |
| SLOP004 | 0.00% |  | 0.00% |  | 0.00% |  |
| SLOP005 | 0.20% |  | 0.00% |  | 0.00% |  |
| SLOP008 | 0.00% |  | 0.00% |  | 0.00% |  |
| SLOP009 | 0.00% |  | 1.00% |  | 0.80% |  |
| SLOP037 | 0.00% |  | 0.00% |  | 0.00% |  |
| SLOP039 | 0.00% |  | 0.60% |  | 0.40% |  |
| SLOP042 | 3.80% |  | 3.20% |  | 3.00% |  |
| SLOP043 | 5.40% |  | 2.00% |  | 2.20% |  |
| **any Tier A rule** | 0.20% |  | 1.00% |  | 0.80% |  |
| **any rule** | 9.20% |  | 6.80% |  | 6.40% |  |

Plain ai Rust in this table, when present, comes only from the synthesized `synth-rust` cell; no real-world dataset registered here ships plain machine-generated Rust (see the droid caveats below).

#### typescript

| rule | codemirage (js proxy) H% | codemirage (js proxy) A% | droid (js proxy) H% | droid (js proxy) A% | hairosetta (js proxy) H% | hairosetta (js proxy) A% | rosetta H% | rosetta A% | semeval13 (js proxy) H% | semeval13 (js proxy) A% |
|---|---|---|---|---|---|---|---|---|---|---|
| SLOP001 | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP002 | 0.00% | 0.40% | 0.00% | 0.20% | 0.00% |  | 0.00% |  | 0.00% | 0.40% |
| SLOP003 | 0.00% | 5.20% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP004 | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP005 | 0.63% | 4.20% | 1.20% | 5.00% | 0.00% |  | 0.00% |  | 0.60% | 3.80% |
| SLOP007 | 0.00% | 0.20% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.40% | 0.00% |
| SLOP008 | 0.00% | 0.00% | 0.00% | 0.00% | 0.20% |  | 0.00% |  | 0.20% | 0.00% |
| SLOP009 | 0.63% | 8.60% | 0.80% | 9.80% | 0.20% |  | 0.00% |  | 0.60% | 5.80% |
| SLOP037 | 1.04% | 2.40% | 0.20% | 1.40% | 1.00% |  | 0.00% |  | 0.40% | 2.00% |
| SLOP038 | 2.51% | 4.00% | 1.40% | 1.80% | 0.00% |  | 0.00% |  | 1.20% | 1.00% |
| SLOP039 | 0.21% | 0.60% | 0.20% | 0.00% | 0.20% |  | 0.00% |  | 0.00% | 0.20% |
| SLOP040 | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% |
| SLOP042 | 1.88% | 13.00% | 3.80% | 20.80% | 10.62% |  | 5.13% |  | 2.80% | 15.80% |
| SLOP043 | 3.34% | 0.60% | 0.80% | 0.80% | 1.60% |  | 0.00% |  | 1.00% | 1.00% |
| **any Tier A rule** | 1.25% | 17.20% | 2.00% | 13.60% | 0.40% |  | 0.00% |  | 1.60% | 9.40% |
| **any rule** | 9.81% | 33.00% | 8.40% | 32.60% | 12.83% |  | 5.13% |  | 6.00% | 25.20% |

#### prose

| rule | aidev H% | aidev A% | apt-eval H% | apt-eval A% | essay-br H% | essay-br A% | faidset H% | faidset A% | pan25 H% | pan25 A% | wetbench-pt H% | wetbench-pt A% |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| SLOP011 |  | 0.00% | 0.00% |  | 0.00% |  | 0.40% | 0.00% | 0.00% | 0.60% | 0.00% | 0.00% |
| SLOP012 |  | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |
| SLOP013 |  | 0.20% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |
| SLOP014 |  | 0.00% | 0.33% |  | 0.00% |  | 0.20% | 1.00% | 0.40% | 25.00% | 0.00% | 0.00% |
| SLOP015 |  | 0.20% | 1.00% |  | 0.20% |  | 0.00% | 0.00% | 2.00% | 1.20% | 0.00% | 0.00% |
| SLOP016 |  | 0.20% | 0.00% |  | 0.00% |  | 0.40% | 24.60% | 0.00% | 23.00% | 0.00% | 0.00% |
| SLOP017 |  | 1.20% | 0.00% |  | 1.20% |  | 0.20% | 33.80% | 10.80% | 23.60% | 0.00% | 0.60% |
| SLOP018 |  | 10.24% | 6.67% |  | 4.60% |  | 2.60% | 0.20% | 27.20% | 46.40% | 2.00% | 0.20% |
| SLOP019 |  | 16.67% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |
| SLOP020 |  | 0.40% | 2.33% |  | 23.20% |  | 11.40% | 0.40% | 45.20% | 31.60% | 1.00% | 0.00% |
| SLOP021 |  | 49.80% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |
| SLOP022 |  | 0.60% | 3.00% |  | 2.20% |  | 0.00% | 0.00% | 5.20% | 2.00% | 0.00% | 0.00% |
| SLOP023 |  | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 0.20% | 0.20% | 0.00% | 0.00% |
| SLOP024 |  | 0.40% | 0.67% |  | 0.20% |  | 1.20% | 10.20% | 1.00% | 14.80% | 0.00% | 0.00% |
| SLOP025 |  | 0.00% | 0.67% |  | 1.60% |  | 0.40% | 0.00% | 0.60% | 3.60% | 0.20% | 0.20% |
| SLOP026 |  | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |
| SLOP027 |  | 0.20% | 0.33% |  | 0.00% |  | 0.40% | 0.40% | 1.00% | 0.40% | 0.00% | 0.00% |
| SLOP028 |  | 0.00% | 0.00% |  | 0.00% |  | 0.20% | 0.00% | 0.20% | 0.20% | 0.00% | 0.00% |
| SLOP029 |  | 0.00% | 0.00% |  | 0.40% |  | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |
| SLOP030 |  | 0.40% | 16.33% |  | 1.20% |  | 9.00% | 6.60% | 42.40% | 14.20% | 1.00% | 0.80% |
| SLOP031 |  | 0.00% | 0.00% |  | 0.00% |  | 1.60% | 0.00% | 0.20% | 0.00% | 0.00% | 0.00% |
| SLOP032 |  | 0.40% | 0.00% |  | 0.00% |  | 0.40% | 4.00% | 0.00% | 0.40% | 0.00% | 0.00% |
| SLOP033 |  | 9.04% | 9.00% |  | 46.60% |  | 8.60% | 0.60% | 61.20% | 19.80% | 9.20% | 2.40% |
| SLOP034 |  | 1.20% | 0.33% |  | 0.00% |  | 0.60% | 0.00% | 1.00% | 0.40% | 0.00% | 0.00% |
| SLOP035 |  | 0.00% | 0.00% |  | 0.00% |  | 0.00% | 0.00% | 0.00% | 0.40% | 0.00% | 0.20% |
| SLOP036 |  | 0.80% | 0.33% |  | 0.00% |  | 0.00% | 0.00% | 2.40% | 0.40% | 0.00% | 0.20% |
| SLOP041 |  | 3.01% | 0.00% |  | 1.20% |  | 0.40% | 0.80% | 3.00% | 8.00% | 0.20% | 0.20% |
| **any Tier A rule** |  | 0.20% | 0.00% |  | 0.00% |  | 0.40% | 0.00% | 0.00% | 0.60% | 0.00% | 0.00% |
| **any rule** |  | 66.06% | 34.33% |  | 63.40% |  | 29.20% | 59.40% | 93.20% | 88.80% | 12.20% | 4.60% |

## Per-dataset detail

### antd-tsx

- Source: https://github.com/ant-design/ant-design
- License: MIT
- Natural language: n/a (code)
- Human split: pinned-2019 -- Ant Design 3.26.0 components, tagged December 2019
- Revision: 3.26.0
- Files fetched: 500
  - tsx: human 500 (0 blank dropped)

#### antd-tsx / tsx

| rule | human hits |
|---|---|
| SLOP007 | 27 (5.40%) |
| SLOP038 | 11 (2.20%) |
| SLOP042 | 2 (0.40%) |
| SLOP005 | 1 (0.20%) |
| SLOP001 | 0 (0.00%) |
| SLOP002 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP008 | 0 (0.00%) |
| SLOP009 | 0 (0.00%) |
| SLOP037 | 0 (0.00%) |
| SLOP039 | 0 (0.00%) |
| SLOP040 | 0 (0.00%) |
| SLOP043 | 0 (0.00%) |
| **any Tier A rule** | 27 (5.40%) |
| **any rule** | 37 (7.40%) |


### beemo

- Source: https://huggingface.co/datasets/toloka/beemo
- Paper: arXiv 2411.04032
- License: MIT (edits); prompts and human outputs CC-BY-NC-4.0
- Natural language: en
- Generators: GPT-4o, Llama 3.1 70B, Mixtral, Gemma, Mistral 7B, Zephyr (2023-24)
- Human split: verified-authors -- No Robots (2023): responses written by expert annotators to the same prompts the models answered
- Revision: 9c014107fe9b
- Files fetched: 2000
  - prose: ai 500 (0 blank dropped), ai-human-edited 500 (0 blank dropped), ai-llm-edited 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: zephyr-7b-beta 77, Llama-2-13b-chat-hf 65, Llama-2-7b-chat-hf 63, gemma-7b-it 58, gemma-2b-it 47, tulu-2-7b 47, tulu-2-13b 43, Llama-2-70b-chat-hf 40
    - ai-human-edited generators: zephyr-7b-beta+human 77, Llama-2-13b-chat-hf+human 65, Llama-2-7b-chat-hf+human 63, gemma-7b-it+human 58, gemma-2b-it+human 47, tulu-2-7b+human 47, tulu-2-13b+human 43, Llama-2-70b-chat-hf+human 40
    - ai-llm-edited generators: zephyr-7b-beta+gpt-4o 77, Llama-2-13b-chat-hf+gpt-4o 65, Llama-2-7b-chat-hf+gpt-4o 63, gemma-7b-it+gpt-4o 58, gemma-2b-it+gpt-4o 47, tulu-2-7b+gpt-4o 47, tulu-2-13b+gpt-4o 43, Llama-2-70b-chat-hf+gpt-4o 40
- `ai-human-edited` is the model output after an expert edit; `ai-llm-edited` is the model output rewritten by GPT-4o or Llama 3.1 70B. Both are robustness splits and never pooled with plain ai.
- Every split answers the same 2,187 prompts, so topic is matched by construction.

#### beemo / prose

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /1k words | ai /1k words |
|---|---|---|---|---|---|---|---|
| SLOP011 | 6 (1.20%) | 121 (24.20%) | 0.95 | 0.95 | 20.17 | 0.11 | 1.31 |
| SLOP041 | 11 (2.20%) | 51 (10.20%) | 0.82 | 0.82 | 4.64 | 0.17 | 0.54 |
| SLOP030 | 64 (12.80%) | 34 (6.80%) | 0.35 | 0.35 | 0.53 | 1.28 | 0.48 |
| SLOP017 | 9 (1.80%) | 31 (6.20%) | 0.78 | 0.78 | 3.44 | 0.14 | 0.33 |
| SLOP019 | 0 (0.00%) | 19 (3.80%) | 1.00* | 1.00* | inf | 0.00 | 0.22 |
| SLOP014 | 3 (0.60%) | 14 (2.80%) | 0.82* | 0.82* | 4.67 | 0.05 | 0.17 |
| SLOP033 | 13 (2.60%) | 11 (2.20%) | 0.46 | 0.46 | 0.85 | 0.26 | 0.12 |
| SLOP013 | 3 (0.60%) | 10 (2.00%) | 0.77* | 0.77* | 3.33 | 0.08 | 0.12 |
| SLOP018 | 27 (5.40%) | 9 (1.80%) | 0.25 | 0.25 | 0.33 | 0.73 | 0.16 |
| SLOP020 | 42 (8.40%) | 8 (1.60%) | 0.16 | 0.16 | 0.19 | 0.64 | 0.08 |
| SLOP024 | 0 (0.00%) | 6 (1.20%) | 1.00* | 1.00* | inf | 0.00 | 0.06 |
| SLOP028 | 0 (0.00%) | 5 (1.00%) | 1.00* | 1.00* | inf | 0.00 | 0.05 |
| SLOP015 | 0 (0.00%) | 4 (0.80%) | 1.00* | 1.00* | inf | 0.00 | 0.04 |
| SLOP022 | 8 (1.60%) | 4 (0.80%) | 0.33* | 0.33* | 0.50 | 0.14 | 0.04 |
| SLOP016 | 1 (0.20%) | 3 (0.60%) | 0.75* | 0.75* | 3.00 | 0.02 | 0.03 |
| SLOP025 | 1 (0.20%) | 3 (0.60%) | 0.75* | 0.75* | 3.00 | 0.02 | 0.03 |
| SLOP031 | 1 (0.20%) | 3 (0.60%) | 0.75* | 0.75* | 3.00 | 0.02 | 0.03 |
| SLOP029 | 2 (0.40%) | 2 (0.40%) | 0.50* | 0.50* | 1.00 | 0.03 | 0.02 |
| SLOP034 | 0 (0.00%) | 2 (0.40%) | 1.00* | 1.00* | inf | 0.00 | 0.02 |
| SLOP036 | 7 (1.40%) | 2 (0.40%) | 0.22* | 0.22* | 0.29 | 0.11 | 0.03 |
| SLOP023 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP027 | 1 (0.20%) | 1 (0.20%) | 0.50* | 0.50* | 1.00 | 0.02 | 0.01 |
| SLOP032 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP012 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP021 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP026 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP035 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 8 (1.60%) | 130 (26.00%) | 0.94 | 0.94 | 16.25 |  |  |
| **any rule** | 147 (29.40%) | 235 (47.00%) | 0.62 | 0.62 | 1.60 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | Llama-2-13b-chat-hf | Llama-2-70b-chat-hf | Llama-2-7b-chat-hf | Mistral-7B-Instruct-v0.1 | Mixtral-8x7B-Instruct-v0.1 | gemma-2b-it | gemma-7b-it | tulu-2-13b | tulu-2-7b | zephyr-7b-beta |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| SLOP011 | 49.2% | 30.0% | 31.7% | 14.3% | 5.1% | 40.4% | 50.0% | 0.0% | 4.3% | 2.6% |
| SLOP017 | 7.7% | 12.5% | 9.5% | 9.5% | 5.1% | 4.3% | 1.7% | 4.7% | 0.0% | 7.8% |
| SLOP030 | 3.1% | 17.5% | 11.1% | 0.0% | 2.6% | 8.5% | 1.7% | 11.6% | 0.0% | 9.1% |
| SLOP041 | 18.5% | 37.5% | 11.1% | 4.8% | 17.9% | 0.0% | 3.4% | 4.7% | 4.3% | 3.9% |


### cpython-doc

- Source: https://github.com/python/cpython
- License: PSF-2.0
- Natural language: en
- Human split: pinned-2019 -- CPython 3.8.0 Doc, tagged October 2019
- Revision: v3.8.0
- Files fetched: 481
  - prose: human 481 (0 blank dropped)
- Shares its clone with cpython-lib (same tag). Written as `.rst`: the boldface/heading-style rules (SLOP019/SLOP021) never fire on `.rst`, only on Md/Mdx/Html, so those two rows read 0 here by construction.

#### cpython-doc / prose

| rule | human hits |
|---|---|
| SLOP030 | 222 (46.15%) |
| SLOP041 | 222 (46.15%) |
| SLOP034 | 150 (31.19%) |
| SLOP033 | 126 (26.20%) |
| SLOP018 | 96 (19.96%) |
| SLOP036 | 57 (11.85%) |
| SLOP017 | 54 (11.23%) |
| SLOP027 | 32 (6.65%) |
| SLOP024 | 23 (4.78%) |
| SLOP011 | 12 (2.49%) |
| SLOP028 | 10 (2.08%) |
| SLOP015 | 5 (1.04%) |
| SLOP032 | 5 (1.04%) |
| SLOP022 | 4 (0.83%) |
| SLOP025 | 3 (0.62%) |
| SLOP013 | 2 (0.42%) |
| SLOP020 | 2 (0.42%) |
| SLOP012 | 0 (0.00%) |
| SLOP014 | 0 (0.00%) |
| SLOP016 | 0 (0.00%) |
| SLOP019 | 0 (0.00%) |
| SLOP021 | 0 (0.00%) |
| SLOP023 | 0 (0.00%) |
| SLOP026 | 0 (0.00%) |
| SLOP029 | 0 (0.00%) |
| SLOP031 | 0 (0.00%) |
| SLOP035 | 0 (0.00%) |
| **any Tier A rule** | 13 (2.70%) |
| **any rule** | 377 (78.38%) |


### cpython-lib

- Source: https://github.com/python/cpython
- License: PSF-2.0
- Natural language: n/a (code)
- Human split: pinned-2019 -- CPython 3.8.0 Lib, tagged October 2019
- Revision: v3.8.0
- Files fetched: 499
  - python: human 499 (1 blank dropped)
- Shares its clone with cpython-doc (same tag).

#### cpython-lib / python

| rule | human hits |
|---|---|
| SLOP043 | 151 (30.26%) |
| SLOP006 | 147 (29.46%) |
| SLOP008 | 45 (9.02%) |
| SLOP042 | 40 (8.02%) |
| SLOP037 | 22 (4.41%) |
| SLOP039 | 5 (1.00%) |
| SLOP040 | 4 (0.80%) |
| SLOP009 | 3 (0.60%) |
| SLOP002 | 2 (0.40%) |
| SLOP001 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| **any Tier A rule** | 173 (34.67%) |
| **any rule** | 243 (48.70%) |


### diplomatrix

- Source: https://huggingface.co/datasets/melll-uff/diplomatrixbr-gen/resolve/main/Diplomatrixbr.json
- License: MIT
- Natural language: pt
- Generators: GPT-4o, Claude 3, Gemini, Llama 3, Sabiá and other 2024 models
- Human split: verified-authors -- CACD diplomatic-career exam essays, handwritten by identified candidates under supervision
- Revision: n/a
- Files fetched: 478
  - prose: ai 390 (0 blank dropped), human 88 (0 blank dropped)
    - ai generators: gpt4o_temp03 10, gpt4o_temp05 10, gpt4o_temp07 10, command_r_plus_08_2024_temp03 10, command_r_plus_08_2024_temp05 10, command_r_plus_08_2024_temp07 10, gemma_27b_temp03 10, gemma_27b_temp05 10
- The human split holds under a hundred essays, so its rates move a full point per file.

#### diplomatrix / prose

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /1k words | ai /1k words |
|---|---|---|---|---|---|---|---|
| SLOP041 | 22 (25.00%) | 280 (71.79%) | 0.74 | 0.93 | 2.87 | 0.38 | 1.50 |
| SLOP017 | 7 (7.95%) | 86 (22.05%) | 0.73 | 0.92 | 2.77 | 0.12 | 0.46 |
| SLOP024 | 0 (0.00%) | 45 (11.54%) | 1.00 | 1.00 | inf | 0.00 | 0.32 |
| SLOP022 | 2 (2.27%) | 29 (7.44%) | 0.77 | 0.94 | 3.27 | 0.03 | 0.18 |
| SLOP033 | 34 (38.64%) | 25 (6.41%) | 0.14 | 0.42 | 0.17 | 0.93 | 0.14 |
| SLOP015 | 0 (0.00%) | 9 (2.31%) | 1.00* | 1.00* | inf | 0.00 | 0.05 |
| SLOP018 | 13 (14.77%) | 8 (2.05%) | 0.12 | 0.38 | 0.14 | 0.86 | 0.07 |
| SLOP020 | 58 (65.91%) | 8 (2.05%) | 0.03 | 0.12 | 0.03 | 1.00 | 0.04 |
| SLOP021 | 0 (0.00%) | 6 (1.54%) | 1.00* | 1.00* | inf | 0.00 | 0.03 |
| SLOP014 | 0 (0.00%) | 5 (1.28%) | 1.00* | 1.00* | inf | 0.00 | 0.04 |
| SLOP030 | 26 (29.55%) | 5 (1.28%) | 0.04 | 0.16 | 0.04 | 0.45 | 0.03 |
| SLOP025 | 0 (0.00%) | 4 (1.03%) | 1.00* | 1.00* | inf | 0.00 | 0.02 |
| SLOP019 | 0 (0.00%) | 3 (0.77%) | 1.00* | 1.00* | inf | 0.00 | 0.02 |
| SLOP035 | 0 (0.00%) | 3 (0.77%) | 1.00* | 1.00* | inf | 0.00 | 0.02 |
| SLOP016 | 0 (0.00%) | 2 (0.51%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP027 | 0 (0.00%) | 2 (0.51%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP029 | 0 (0.00%) | 1 (0.26%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP031 | 0 (0.00%) | 1 (0.26%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP011 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP012 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP013 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP023 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP026 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP028 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP032 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP034 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP036 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 0 (0.00%) | 0 (0.00%) | -- | -- | -- |  |  |
| **any rule** | 81 (92.05%) | 343 (87.95%) | 0.49 | 0.81 | 0.96 |  |  |


### ghostbuster

- Source: https://github.com/vivek3141/ghostbuster-data
- Paper: arXiv 2305.15047
- License: CC-BY-3.0
- Natural language: en
- Generators: gpt-3.5-turbo and claude (2023)
- Human split: pinned-2019 -- Reuters 50-50 news (1996-97) and r/WritingPrompts stories (2017-18); the undated IvyPanda essays are dropped from the human split
- Revision: 86ebd7259055
- Files fetched: 1000
  - prose: ai 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: gpt 286, claude 214
- gpt_prompt*/gpt_semantic/gpt_writing variants are skipped; only the plain gpt/ and claude/ generations are counted as `ai`.
- Every split carries a sibling logprobs/ tree of GPT-2 token/score dumps, two thirds of the .txt files under human/; they are excluded, because they are model output about a document rather than the document.
- AI documents were generated from prompts derived from the paired human document with a target length, so length is matched by construction.

#### ghostbuster / prose

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /1k words | ai /1k words |
|---|---|---|---|---|---|---|---|
| SLOP030 | 325 (65.00%) | 170 (34.00%) | 0.34 | 0.34 | 0.52 | 1.29 | 0.89 |
| SLOP041 | 17 (3.40%) | 157 (31.40%) | 0.90 | 0.90 | 9.24 | 0.07 | 0.78 |
| SLOP017 | 9 (1.80%) | 111 (22.20%) | 0.92 | 0.93 | 12.33 | 0.04 | 0.55 |
| SLOP018 | 116 (23.20%) | 70 (14.00%) | 0.38 | 0.38 | 0.60 | 1.17 | 0.54 |
| SLOP020 | 6 (1.20%) | 70 (14.00%) | 0.92 | 0.92 | 11.67 | 0.02 | 0.35 |
| SLOP016 | 0 (0.00%) | 60 (12.00%) | 1.00 | 1.00 | inf | 0.00 | 0.30 |
| SLOP014 | 1 (0.20%) | 44 (8.80%) | 0.98 | 0.98 | 44.00 | 0.00 | 0.27 |
| SLOP024 | 6 (1.20%) | 43 (8.60%) | 0.88 | 0.88 | 7.17 | 0.02 | 0.28 |
| SLOP033 | 112 (22.40%) | 28 (5.60%) | 0.20 | 0.20 | 0.25 | 0.57 | 0.17 |
| SLOP025 | 11 (2.20%) | 25 (5.00%) | 0.69 | 0.69 | 2.27 | 0.04 | 0.18 |
| SLOP022 | 29 (5.80%) | 13 (2.60%) | 0.31 | 0.31 | 0.45 | 0.12 | 0.07 |
| SLOP015 | 3 (0.60%) | 11 (2.20%) | 0.79* | 0.79* | 3.67 | 0.01 | 0.05 |
| SLOP028 | 0 (0.00%) | 11 (2.20%) | 1.00* | 1.00* | inf | 0.00 | 0.05 |
| SLOP032 | 0 (0.00%) | 11 (2.20%) | 1.00* | 1.00* | inf | 0.00 | 0.05 |
| SLOP027 | 0 (0.00%) | 7 (1.40%) | 1.00* | 1.00* | inf | 0.00 | 0.03 |
| SLOP035 | 0 (0.00%) | 5 (1.00%) | 1.00* | 1.00* | inf | 0.00 | 0.02 |
| SLOP036 | 10 (2.00%) | 5 (1.00%) | 0.33* | 0.33* | 0.50 | 0.04 | 0.02 |
| SLOP034 | 11 (2.20%) | 3 (0.60%) | 0.21* | 0.21* | 0.27 | 0.04 | 0.01 |
| SLOP031 | 0 (0.00%) | 2 (0.40%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP011 | 6 (1.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.03 | 0.00 |
| SLOP012 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP013 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP019 | 2 (0.40%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.01 | 0.00 |
| SLOP021 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP023 | 6 (1.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.02 | 0.00 |
| SLOP026 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP029 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 6 (1.20%) | 0 (0.00%) | 0.00 | 0.00 | 0.00 |  |  |
| **any rule** | 417 (83.40%) | 352 (70.40%) | 0.46 | 0.46 | 0.84 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | claude | gpt |
| --- | --- | --- |
| SLOP014 | 1.9% | 14.0% |
| SLOP016 | 0.5% | 20.6% |
| SLOP017 | 15.0% | 27.6% |
| SLOP018 | 15.4% | 12.9% |
| SLOP020 | 25.7% | 5.2% |
| SLOP024 | 3.3% | 12.6% |
| SLOP025 | 6.1% | 4.2% |
| SLOP030 | 15.9% | 47.6% |
| SLOP033 | 3.3% | 7.3% |
| SLOP041 | 22.9% | 37.8% |


### go-std

- Source: https://github.com/golang/go
- License: BSD-3-Clause
- Natural language: n/a (code)
- Human split: pinned-2019 -- Go 1.13 standard library, tagged September 2019
- Revision: go1.13
- Files fetched: 500
  - go: human 500 (0 blank dropped)

#### go-std / go

| rule | human hits |
|---|---|
| SLOP043 | 105 (21.00%) |
| SLOP042 | 23 (4.60%) |
| SLOP008 | 22 (4.40%) |
| SLOP037 | 20 (4.00%) |
| SLOP039 | 12 (2.40%) |
| SLOP001 | 1 (0.20%) |
| SLOP002 | 1 (0.20%) |
| SLOP009 | 1 (0.20%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP005 | 0 (0.00%) |
| **any Tier A rule** | 25 (5.00%) |
| **any rule** | 148 (29.60%) |


### gpt2-output

- Source: https://github.com/openai/gpt-2-output-dataset
- License: MIT
- Natural language: en
- Generators: GPT-2 1542M (2019)
- Human split: pinned-2019 -- WebText: Reddit-outbound links with karma >= 3, scraped through December 2017
- Revision: n/a
- Files fetched: 1000
  - prose: ai 500 (0 blank dropped), human 500 (0 blank dropped)
- Top-K 40 sampling shifts the part-of-speech distribution (underuses proper nouns, overuses pronouns) per OpenAI's own detection.md, so a pronoun or opener rule can look good here for a sampling reason rather than a style one; the plain xl-1542M file is registered, not a -k40 variant.
- Documents near 500 characters detect about 15% worse per the same note, so length is a confound on this dataset.

#### gpt2-output / prose

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /1k words | ai /1k words |
|---|---|---|---|---|---|---|---|
| SLOP018 | 175 (35.00%) | 206 (41.20%) | 0.54 | 0.54 | 1.18 | 2.64 | 3.49 |
| SLOP033 | 147 (29.40%) | 182 (36.40%) | 0.55 | 0.55 | 1.24 | 1.01 | 1.43 |
| SLOP030 | 80 (16.00%) | 70 (14.00%) | 0.47 | 0.47 | 0.88 | 0.41 | 0.41 |
| SLOP022 | 13 (2.60%) | 21 (4.20%) | 0.62 | 0.62 | 1.62 | 0.06 | 0.10 |
| SLOP041 | 29 (5.80%) | 13 (2.60%) | 0.31 | 0.31 | 0.45 | 0.14 | 0.06 |
| SLOP036 | 4 (0.80%) | 12 (2.40%) | 0.75* | 0.75* | 3.00 | 0.02 | 0.05 |
| SLOP017 | 16 (3.20%) | 10 (2.00%) | 0.38 | 0.38 | 0.62 | 0.07 | 0.04 |
| SLOP024 | 8 (1.60%) | 7 (1.40%) | 0.47* | 0.47* | 0.87 | 0.04 | 0.03 |
| SLOP034 | 8 (1.60%) | 7 (1.40%) | 0.47* | 0.47* | 0.87 | 0.04 | 0.03 |
| SLOP011 | 12 (2.40%) | 6 (1.20%) | 0.33* | 0.33* | 0.50 | 0.06 | 0.03 |
| SLOP027 | 5 (1.00%) | 6 (1.20%) | 0.55* | 0.55* | 1.20 | 0.02 | 0.03 |
| SLOP025 | 7 (1.40%) | 5 (1.00%) | 0.42* | 0.42* | 0.71 | 0.04 | 0.02 |
| SLOP014 | 4 (0.80%) | 3 (0.60%) | 0.43* | 0.43* | 0.75 | 0.02 | 0.01 |
| SLOP015 | 2 (0.40%) | 3 (0.60%) | 0.60* | 0.60* | 1.50 | 0.01 | 0.01 |
| SLOP031 | 0 (0.00%) | 2 (0.40%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP021 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.00 |
| SLOP023 | 3 (0.60%) | 1 (0.20%) | 0.25* | 0.25* | 0.33 | 0.01 | 0.00 |
| SLOP012 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP013 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP016 | 1 (0.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.00 | 0.00 |
| SLOP019 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP020 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP026 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP028 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP029 | 1 (0.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.00 | 0.00 |
| SLOP032 | 3 (0.60%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.01 | 0.00 |
| SLOP035 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 12 (2.40%) | 6 (1.20%) | 0.33 | 0.33 | 0.50 |  |  |
| **any rule** | 301 (60.20%) | 323 (64.60%) | 0.52 | 0.52 | 1.07 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | gpt-2-xl-1542m |
| --- | --- |
| SLOP018 | 41.2% |
| SLOP022 | 4.2% |
| SLOP030 | 14.0% |
| SLOP033 | 36.4% |


### hc3

- Source: https://huggingface.co/datasets/Hello-SimpleAI/HC3
- Paper: arXiv 2301.07597
- License: CC-BY-SA-4.0
- Natural language: en
- Generators: ChatGPT (December 2022)
- Human split: pinned-2019 -- ELI5 (2019), FiQA (2018) and WikiQA (2015) answers; the wiki_csai (2022) and medicine (2020) human rows are dropped, their ChatGPT rows kept
- Revision: 4d0ff18143b5
- Files fetched: 1000
  - prose: ai 500 (0 blank dropped), human 500 (0 blank dropped)
- `domain` is the HC3 config (wiki_csai, open_qa, reddit_eli5, finance, medicine); every ai file is ChatGPT, so the per-generator table has exactly one column here.
- Answers were pasted out of the API payload: about one in seven ChatGPT files carries a literal backslash-n paragraph break, a pipeline artifact rather than model style.

#### hc3 / prose

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /1k words | ai /1k words |
|---|---|---|---|---|---|---|---|
| SLOP033 | 49 (9.80%) | 109 (21.80%) | 0.69 | 0.69 | 2.22 | 1.15 | 1.77 |
| SLOP041 | 14 (2.80%) | 96 (19.20%) | 0.87 | 0.87 | 6.86 | 0.23 | 1.19 |
| SLOP030 | 45 (9.00%) | 45 (9.00%) | 0.50 | 0.50 | 1.00 | 0.73 | 0.56 |
| SLOP017 | 0 (0.00%) | 19 (3.80%) | 1.00* | 1.00* | inf | 0.00 | 0.24 |
| SLOP011 | 2 (0.40%) | 6 (1.20%) | 0.75* | 0.75* | 3.00 | 0.03 | 0.07 |
| SLOP027 | 0 (0.00%) | 5 (1.00%) | 1.00* | 1.00* | inf | 0.00 | 0.06 |
| SLOP024 | 6 (1.20%) | 4 (0.80%) | 0.40* | 0.40* | 0.67 | 0.10 | 0.05 |
| SLOP025 | 1 (0.20%) | 3 (0.60%) | 0.75* | 0.75* | 3.00 | 0.02 | 0.04 |
| SLOP015 | 2 (0.40%) | 2 (0.40%) | 0.50* | 0.50* | 1.00 | 0.03 | 0.02 |
| SLOP028 | 0 (0.00%) | 2 (0.40%) | 1.00* | 1.00* | inf | 0.00 | 0.02 |
| SLOP032 | 1 (0.20%) | 2 (0.40%) | 0.67* | 0.67* | 2.00 | 0.02 | 0.02 |
| SLOP036 | 2 (0.40%) | 2 (0.40%) | 0.50* | 0.50* | 1.00 | 0.03 | 0.05 |
| SLOP013 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP018 | 25 (5.00%) | 1 (0.20%) | 0.04 | 0.04 | 0.04 | 0.74 | 0.01 |
| SLOP022 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP012 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP014 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP016 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP019 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP020 | 2 (0.40%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.03 | 0.00 |
| SLOP021 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP023 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP026 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP029 | 1 (0.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.02 | 0.00 |
| SLOP031 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP034 | 3 (0.60%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.05 | 0.00 |
| SLOP035 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 2 (0.40%) | 6 (1.20%) | 0.75 | 0.75 | 3.00 |  |  |
| **any rule** | 115 (23.00%) | 214 (42.80%) | 0.65 | 0.65 | 1.86 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | chatgpt |
| --- | --- |
| SLOP030 | 9.0% |
| SLOP033 | 21.8% |
| SLOP041 | 19.2% |


### k8s-docs

- Source: https://github.com/kubernetes/website
- License: CC-BY-4.0
- Natural language: en
- Human split: pinned-2019 -- kubernetes/website English docs, tagged December 2019
- Revision: snapshot-initial-v1.17
- Files fetched: 500
  - prose: human 500 (0 blank dropped)
- Pages carry Hugo shortcodes (`{{% capture body %}}`); some tutorials ship as `.html` instead of `.md`, so the glob stays `.md`-only.

#### k8s-docs / prose

| rule | human hits |
|---|---|
| SLOP041 | 161 (32.20%) |
| SLOP021 | 122 (24.40%) |
| SLOP033 | 26 (5.20%) |
| SLOP034 | 21 (4.20%) |
| SLOP017 | 20 (4.00%) |
| SLOP027 | 20 (4.00%) |
| SLOP020 | 19 (3.80%) |
| SLOP018 | 16 (3.20%) |
| SLOP030 | 16 (3.20%) |
| SLOP024 | 12 (2.40%) |
| SLOP036 | 10 (2.00%) |
| SLOP019 | 6 (1.20%) |
| SLOP028 | 5 (1.00%) |
| SLOP015 | 4 (0.80%) |
| SLOP013 | 1 (0.20%) |
| SLOP022 | 1 (0.20%) |
| SLOP025 | 1 (0.20%) |
| SLOP031 | 1 (0.20%) |
| SLOP032 | 1 (0.20%) |
| SLOP011 | 0 (0.00%) |
| SLOP012 | 0 (0.00%) |
| SLOP014 | 0 (0.00%) |
| SLOP016 | 0 (0.00%) |
| SLOP023 | 0 (0.00%) |
| SLOP026 | 0 (0.00%) |
| SLOP029 | 0 (0.00%) |
| SLOP035 | 0 (0.00%) |
| **any Tier A rule** | 1 (0.20%) |
| **any rule** | 288 (57.60%) |


### lener-br

- Source: https://github.com/peluz/lener-br
- Paper: PROPOR 2018 (Universidade de Brasília)
- License: MIT (packaging); the raw texts are public-domain Brazilian federal statutes and court rulings (Lei 9.610/98 Art. 8)
- Natural language: pt
- Human split: pinned-2019 -- Brazilian federal statutes and court rulings dated 2008-2018 by their own text; the repo's own commit history only starts 2020-05-15, so the cutoff rests on the documents' stated dates, not the clone
- Revision: 4999cb7f6319
- Files fetched: 70
  - prose: human 70 (0 blank dropped)

#### lener-br / prose

| rule | human hits |
|---|---|
| SLOP033 | 64 (91.43%) |
| SLOP030 | 39 (55.71%) |
| SLOP020 | 19 (27.14%) |
| SLOP015 | 10 (14.29%) |
| SLOP017 | 9 (12.86%) |
| SLOP041 | 9 (12.86%) |
| SLOP018 | 8 (11.43%) |
| SLOP027 | 3 (4.29%) |
| SLOP025 | 1 (1.43%) |
| SLOP034 | 1 (1.43%) |
| SLOP011 | 0 (0.00%) |
| SLOP012 | 0 (0.00%) |
| SLOP013 | 0 (0.00%) |
| SLOP014 | 0 (0.00%) |
| SLOP016 | 0 (0.00%) |
| SLOP019 | 0 (0.00%) |
| SLOP021 | 0 (0.00%) |
| SLOP022 | 0 (0.00%) |
| SLOP023 | 0 (0.00%) |
| SLOP024 | 0 (0.00%) |
| SLOP026 | 0 (0.00%) |
| SLOP028 | 0 (0.00%) |
| SLOP029 | 0 (0.00%) |
| SLOP031 | 0 (0.00%) |
| SLOP032 | 0 (0.00%) |
| SLOP035 | 0 (0.00%) |
| SLOP036 | 0 (0.00%) |
| **any Tier A rule** | 0 (0.00%) |
| **any rule** | 69 (98.57%) |


### mage

- Source: https://huggingface.co/datasets/yaful/MAGE
- Paper: arXiv 2305.13242
- License: Apache-2.0 on the card, CC-BY-4.0 in the repo
- Natural language: en
- Generators: 27 LLMs from GPT-J to GPT-3.5 and LLaMA (2019-2022)
- Human split: pinned-2019 -- nine pre-2020 corpora (CMV, Yelp, XSum, TLDR, ELI5, WritingPrompts, ROC, HellaSwag, SQuAD); the SciGen human rows (arXiv through 2021) are dropped
- Revision: 342663f0a2b7
- Files fetched: 1000
  - prose: ai 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: 13B 100, gpt_j 100, opt_350m 100, opt_2.7b 100, text-davinci-002 53, flan_t5_base 47
- MAGE's own license terms conflict across its source domains (it re-publishes several licensed corpora); treat this dataset as measurement-only, nothing republished beyond aggregate counts.
- Punctuation was normalized and line breaks removed before release, so paragraph, whitespace and markdown-structure signals are gone from both splits.

#### mage / prose

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /1k words | ai /1k words |
|---|---|---|---|---|---|---|---|
| SLOP033 | 50 (10.00%) | 157 (31.40%) | 0.76 | 0.76 | 3.14 | 0.87 | 1.75 |
| SLOP030 | 113 (22.60%) | 153 (30.60%) | 0.58 | 0.58 | 1.35 | 1.53 | 0.99 |
| SLOP041 | 4 (0.80%) | 70 (14.00%) | 0.95 | 0.95 | 17.50 | 0.05 | 0.45 |
| SLOP022 | 7 (1.40%) | 24 (4.80%) | 0.77 | 0.77 | 3.43 | 0.10 | 0.16 |
| SLOP024 | 3 (0.60%) | 9 (1.80%) | 0.75* | 0.75* | 3.00 | 0.04 | 0.06 |
| SLOP025 | 2 (0.40%) | 9 (1.80%) | 0.82* | 0.82* | 4.50 | 0.03 | 0.06 |
| SLOP036 | 3 (0.60%) | 8 (1.60%) | 0.73* | 0.73* | 2.67 | 0.04 | 0.05 |
| SLOP017 | 2 (0.40%) | 7 (1.40%) | 0.78* | 0.78* | 3.50 | 0.03 | 0.05 |
| SLOP034 | 2 (0.40%) | 6 (1.20%) | 0.75* | 0.75* | 3.00 | 0.03 | 0.04 |
| SLOP027 | 0 (0.00%) | 4 (0.80%) | 1.00* | 1.00* | inf | 0.00 | 0.03 |
| SLOP015 | 4 (0.80%) | 2 (0.40%) | 0.33* | 0.33* | 0.50 | 0.05 | 0.01 |
| SLOP023 | 1 (0.20%) | 1 (0.20%) | 0.50* | 0.50* | 1.00 | 0.01 | 0.01 |
| SLOP028 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP011 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP012 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP013 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP014 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP016 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP018 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP019 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP020 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP021 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP026 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP029 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP031 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP032 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP035 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 0 (0.00%) | 0 (0.00%) | -- | -- | -- |  |  |
| **any rule** | 152 (30.40%) | 296 (59.20%) | 0.66 | 0.66 | 1.95 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | 13B | flan_t5_base | gpt_j | opt_2.7b | opt_350m | text-davinci-002 |
| --- | --- | --- | --- | --- | --- | --- |
| SLOP022 | 12.0% | 6.4% | 4.0% | 0.0% | 3.0% | 3.8% |
| SLOP030 | 78.0% | 8.5% | 13.0% | 5.0% | 13.0% | 75.5% |
| SLOP033 | 13.0% | 0.0% | 35.0% | 63.0% | 43.0% | 5.7% |
| SLOP041 | 67.0% | 0.0% | 0.0% | 0.0% | 0.0% | 5.7% |


### naples-code

- Source: https://zenodo.org/records/15423067
- Paper: arXiv 2508.21634
- License: CC-BY-4.0
- Natural language: en
- Generators: gpt-3.5-turbo (2023), DeepSeek-Coder-Instruct-33B and Qwen2.5-Coder-Instruct-32B (2024)
- Human split: pinned-2019 -- HMCorp: 16,928 non-forked GitHub repos sorted by stars, filtered from CodeSearchNet (2019)
- Revision: n/a
- Files fetched: 1000
  - python: ai 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: dsc 181, qwen 161, chatgpt 158
- `docstring` ships as its own column, separate from `human_code`, so SLOP001-SLOP004, SLOP042 and SLOP043 print `n/a` here the same way they do on codet_m4 -- the extraction pass strips comments and docstrings out of the code columns, not the model.
- The file is 651 MB; `fetch_naples_code` streams it and stops once both samples are full, so this is a head sample of file order rather than the spread sample every HF-backed dataset above gets through `/filter`.
- The Java half of this Zenodo record is not registered: stopslop has no Java lang.
- `n/a` rules on this dataset: SLOP001, SLOP002, SLOP003, SLOP004, SLOP042, SLOP043

#### naples-code / python

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /KLoC | ai /KLoC |
|---|---|---|---|---|---|---|---|
| SLOP008 | 0 (0.00%) | 23 (4.60%) | 1.00 | 1.00 | inf | 0.00 | 3.85 |
| SLOP006 | 11 (2.20%) | 8 (1.60%) | 0.42* | 0.42* | 0.73 | 1.73 | 1.60 |
| SLOP037 | 6 (1.20%) | 7 (1.40%) | 0.54* | 0.54* | 1.17 | 0.74 | 1.44 |
| SLOP009 | 0 (0.00%) | 5 (1.00%) | 1.00* | 1.00* | inf | 0.00 | 0.80 |
| SLOP039 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.16 |
| SLOP001 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| SLOP002 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| SLOP003 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| SLOP004 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| SLOP040 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP042 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| SLOP043 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| **any Tier A rule** | 11 (2.20%) | 35 (7.00%) | 0.76 | 0.76 | 3.18 |  |  |
| **any rule** | 37 (7.40%) | 72 (14.40%) | 0.66 | 0.66 | 1.95 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | chatgpt | dsc | qwen |
| --- | --- | --- | --- |
| SLOP008 | 2.5% | 2.8% | 8.7% |


### react-docs

- Source: https://github.com/reactjs/react.dev
- License: CC-BY-4.0
- Natural language: en
- Human split: pinned-2019 -- react.dev docs at their last default-branch commit before 2020-01-01
- Revision: f90d199a61c1
- Files fetched: 90
  - prose: human 90 (0 blank dropped)
- Files carry YAML frontmatter (id/title/permalink); the repo has no release tag near the cutoff, cloned shallow-since 2019 like rust-book.

#### react-docs / prose

| rule | human hits |
|---|---|
| SLOP041 | 31 (34.44%) |
| SLOP018 | 24 (26.67%) |
| SLOP024 | 9 (10.00%) |
| SLOP017 | 8 (8.89%) |
| SLOP020 | 7 (7.78%) |
| SLOP021 | 7 (7.78%) |
| SLOP030 | 7 (7.78%) |
| SLOP011 | 5 (5.56%) |
| SLOP027 | 5 (5.56%) |
| SLOP033 | 5 (5.56%) |
| SLOP019 | 2 (2.22%) |
| SLOP034 | 2 (2.22%) |
| SLOP036 | 2 (2.22%) |
| SLOP015 | 1 (1.11%) |
| SLOP022 | 1 (1.11%) |
| SLOP025 | 1 (1.11%) |
| SLOP032 | 1 (1.11%) |
| SLOP012 | 0 (0.00%) |
| SLOP013 | 0 (0.00%) |
| SLOP014 | 0 (0.00%) |
| SLOP016 | 0 (0.00%) |
| SLOP023 | 0 (0.00%) |
| SLOP026 | 0 (0.00%) |
| SLOP028 | 0 (0.00%) |
| SLOP029 | 0 (0.00%) |
| SLOP031 | 0 (0.00%) |
| SLOP035 | 0 (0.00%) |
| **any Tier A rule** | 5 (5.56%) |
| **any rule** | 59 (65.56%) |


### rust-book

- Source: https://github.com/rust-lang/book
- License: MIT OR Apache-2.0
- Natural language: en
- Human split: pinned-2019 -- rust-lang/book at its last default-branch commit before 2020-01-01
- Revision: be91ce0d3e63
- Files fetched: 105
  - prose: human 105 (0 blank dropped)
- The repo has no release tags near the cutoff; cloned shallow-since 2019 and checked out the last commit before 2020-01-01.

#### rust-book / prose

| rule | human hits |
|---|---|
| SLOP020 | 103 (98.10%) |
| SLOP041 | 65 (61.90%) |
| SLOP021 | 58 (55.24%) |
| SLOP018 | 22 (20.95%) |
| SLOP034 | 20 (19.05%) |
| SLOP033 | 18 (17.14%) |
| SLOP030 | 16 (15.24%) |
| SLOP017 | 8 (7.62%) |
| SLOP027 | 3 (2.86%) |
| SLOP011 | 2 (1.90%) |
| SLOP024 | 2 (1.90%) |
| SLOP028 | 2 (1.90%) |
| SLOP015 | 1 (0.95%) |
| SLOP022 | 1 (0.95%) |
| SLOP036 | 1 (0.95%) |
| SLOP012 | 0 (0.00%) |
| SLOP013 | 0 (0.00%) |
| SLOP014 | 0 (0.00%) |
| SLOP016 | 0 (0.00%) |
| SLOP019 | 0 (0.00%) |
| SLOP023 | 0 (0.00%) |
| SLOP025 | 0 (0.00%) |
| SLOP026 | 0 (0.00%) |
| SLOP029 | 0 (0.00%) |
| SLOP031 | 0 (0.00%) |
| SLOP032 | 0 (0.00%) |
| SLOP035 | 0 (0.00%) |
| **any Tier A rule** | 2 (1.90%) |
| **any rule** | 103 (98.10%) |


### rust-std

- Source: https://github.com/rust-lang/rust
- License: MIT OR Apache-2.0
- Natural language: n/a (code)
- Human split: pinned-2019 -- Rust 1.40.0 libstd, libcore and liballoc, tagged December 2019
- Revision: 1.40.0
- Files fetched: 500
  - rust: human 500 (0 blank dropped)

#### rust-std / rust

| rule | human hits |
|---|---|
| SLOP043 | 121 (24.20%) |
| SLOP005 | 16 (3.20%) |
| SLOP039 | 16 (3.20%) |
| SLOP042 | 6 (1.20%) |
| SLOP009 | 1 (0.20%) |
| SLOP001 | 0 (0.00%) |
| SLOP002 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP008 | 0 (0.00%) |
| SLOP037 | 0 (0.00%) |
| **any Tier A rule** | 17 (3.40%) |
| **any rule** | 137 (27.40%) |


### semeval24-m4

- Source: https://huggingface.co/datasets/d0rj/SemEval2024-task8
- Paper: arXiv 2404.14183 (task overview), arXiv 2305.14902 (corpus)
- License: Apache-2.0
- Natural language: en
- Generators: ChatGPT, GPT-3 davinci, Cohere, Dolly (2023)
- Human split: pinned-2019 -- PeerRead (2007-2017) plus pre-2020 arXiv, Reddit and WikiHow text; the wikipedia human rows are dropped, their machine rows kept, the same treatment HC3's wiki_csai config gets
- Revision: 509a1a6a33f9
- Files fetched: 1000
  - prose: ai 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: davinci 201, dolly 199, chatGPT 100
- WikiHow is CC-BY-NC-SA and Wikipedia CC-BY-SA upstream, which the Apache-2.0 mirror label does not override; never quote a line from this dataset in an issue.

#### semeval24-m4 / prose

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /1k words | ai /1k words |
|---|---|---|---|---|---|---|---|
| SLOP041 | 74 (14.80%) | 278 (55.60%) | 0.79 | 0.79 | 3.76 | 0.28 | 1.34 |
| SLOP030 | 135 (27.00%) | 117 (23.40%) | 0.46 | 0.46 | 0.87 | 0.68 | 0.59 |
| SLOP020 | 58 (11.60%) | 108 (21.60%) | 0.65 | 0.65 | 1.86 | 0.22 | 0.52 |
| SLOP033 | 135 (27.00%) | 75 (15.00%) | 0.36 | 0.36 | 0.56 | 1.16 | 0.50 |
| SLOP017 | 49 (9.80%) | 49 (9.80%) | 0.50 | 0.50 | 1.00 | 0.19 | 0.24 |
| SLOP018 | 44 (8.80%) | 33 (6.60%) | 0.43 | 0.43 | 0.75 | 0.73 | 0.28 |
| SLOP027 | 15 (3.00%) | 26 (5.20%) | 0.63 | 0.63 | 1.73 | 0.06 | 0.13 |
| SLOP034 | 29 (5.80%) | 25 (5.00%) | 0.46 | 0.46 | 0.86 | 0.12 | 0.13 |
| SLOP011 | 4 (0.80%) | 17 (3.40%) | 0.81 | 0.81 | 4.25 | 0.02 | 0.58 |
| SLOP024 | 16 (3.20%) | 15 (3.00%) | 0.48 | 0.48 | 0.94 | 0.06 | 0.08 |
| SLOP015 | 7 (1.40%) | 11 (2.20%) | 0.61* | 0.61* | 1.57 | 0.03 | 0.07 |
| SLOP028 | 5 (1.00%) | 4 (0.80%) | 0.44* | 0.44* | 0.80 | 0.02 | 0.02 |
| SLOP031 | 0 (0.00%) | 4 (0.80%) | 1.00* | 1.00* | inf | 0.00 | 0.02 |
| SLOP016 | 0 (0.00%) | 3 (0.60%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP025 | 2 (0.40%) | 3 (0.60%) | 0.60* | 0.60* | 1.50 | 0.01 | 0.01 |
| SLOP022 | 14 (2.80%) | 2 (0.40%) | 0.13* | 0.12* | 0.14 | 0.06 | 0.01 |
| SLOP036 | 5 (1.00%) | 2 (0.40%) | 0.29* | 0.29* | 0.40 | 0.02 | 0.01 |
| SLOP013 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.00 |
| SLOP014 | 1 (0.20%) | 1 (0.20%) | 0.50* | 0.50* | 1.00 | 0.00 | 0.01 |
| SLOP023 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.00 |
| SLOP032 | 1 (0.20%) | 1 (0.20%) | 0.50* | 0.50* | 1.00 | 0.00 | 0.00 |
| SLOP012 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP019 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP021 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP026 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP029 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP035 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 4 (0.80%) | 18 (3.60%) | 0.82 | 0.82 | 4.50 |  |  |
| **any rule** | 304 (60.80%) | 415 (83.00%) | 0.58 | 0.58 | 1.37 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | chatGPT | davinci | dolly |
| --- | --- | --- | --- |
| SLOP017 | 8.0% | 16.9% | 3.5% |
| SLOP018 | 2.0% | 10.9% | 4.5% |
| SLOP020 | 3.0% | 45.3% | 7.0% |
| SLOP027 | 1.0% | 9.5% | 3.0% |
| SLOP030 | 7.0% | 4.5% | 50.8% |
| SLOP033 | 3.0% | 10.4% | 25.6% |
| SLOP034 | 1.0% | 9.0% | 3.0% |
| SLOP041 | 64.0% | 62.2% | 44.7% |


### typescript-src

- Source: https://github.com/microsoft/TypeScript
- License: Apache-2.0
- Natural language: n/a (code)
- Human split: pinned-2019 -- TypeScript 3.7.2 compiler sources, tagged November 2019
- Revision: v3.7.2
- Files fetched: 209
  - typescript: human 209 (2 blank dropped)

#### typescript-src / typescript

| rule | human hits |
|---|---|
| SLOP043 | 73 (34.93%) |
| SLOP042 | 35 (16.75%) |
| SLOP007 | 21 (10.05%) |
| SLOP039 | 9 (4.31%) |
| SLOP040 | 6 (2.87%) |
| SLOP005 | 4 (1.91%) |
| SLOP037 | 3 (1.44%) |
| SLOP008 | 1 (0.48%) |
| SLOP001 | 0 (0.00%) |
| SLOP002 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP009 | 0 (0.00%) |
| SLOP038 | 0 (0.00%) |
| **any Tier A rule** | 23 (11.00%) |
| **any rule** | 83 (39.71%) |


---

The splits below were collected after coding assistants and chat models came into general use, so nobody can guarantee their human side was written without one unless the dataset's authors verified it. They are fetched, scored and read exactly like the splits above, but they stay out of the pooled human rates, the lift and precision columns, and the takeaways.

### aidev

- Source: https://huggingface.co/datasets/hao-li/AIDev
- Paper: arXiv 2507.15003
- License: CC-BY-4.0
- Natural language: en
- Generators: Claude Code, OpenAI Codex, Cursor, Devin, Copilot and Jules agents (2025)
- Revision: c63c8a57a2de
- Files fetched: 498
  - prose: ai 498 (0 blank dropped)
    - ai generators: OpenAI_Codex 298, Copilot 200
- AI-only: pull-request descriptions written by autonomous coding agents on public GitHub repositories; the technical-prose baseline is the pinned cpython-doc and rust-book cells.
- A description is Markdown a maintainer reads; SLOP029/SLOP035 style rules see their natural habitat here.

#### aidev / prose

| rule | ai hits |
|---|---|
| SLOP021 | 248 (49.80%) |
| SLOP019 | 83 (16.67%) |
| SLOP018 | 51 (10.24%) |
| SLOP033 | 45 (9.04%) |
| SLOP041 | 15 (3.01%) |
| SLOP017 | 6 (1.20%) |
| SLOP034 | 6 (1.20%) |
| SLOP036 | 4 (0.80%) |
| SLOP022 | 3 (0.60%) |
| SLOP020 | 2 (0.40%) |
| SLOP024 | 2 (0.40%) |
| SLOP030 | 2 (0.40%) |
| SLOP032 | 2 (0.40%) |
| SLOP013 | 1 (0.20%) |
| SLOP015 | 1 (0.20%) |
| SLOP016 | 1 (0.20%) |
| SLOP027 | 1 (0.20%) |
| SLOP011 | 0 (0.00%) |
| SLOP012 | 0 (0.00%) |
| SLOP014 | 0 (0.00%) |
| SLOP023 | 0 (0.00%) |
| SLOP025 | 0 (0.00%) |
| SLOP026 | 0 (0.00%) |
| SLOP028 | 0 (0.00%) |
| SLOP029 | 0 (0.00%) |
| SLOP031 | 0 (0.00%) |
| SLOP035 | 0 (0.00%) |
| **any Tier A rule** | 1 (0.20%) |
| **any rule** | 329 (66.06%) |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | Copilot | OpenAI_Codex |
| --- | --- | --- |
| SLOP018 | 24.5% | 0.7% |
| SLOP019 | 41.5% | 0.0% |
| SLOP021 | 38.5% | 57.4% |
| SLOP033 | 22.5% | 0.0% |


### aigcodeset

- Source: https://huggingface.co/datasets/basakdemirok/AIGCodeSet
- Paper: arXiv 2412.16594
- License: CDLA-Permissive-2.0
- Natural language: n/a (code)
- Generators: CodeLlama-34B, Codestral-22B, Gemini 1.5 Flash (2024)
- Human split: unverified -- IBM CodeNet submissions, collected through 2020 and published 2021; AtCoder and AIZU users, unverified for assistant use
- Revision: b4d4e69fdbcb
- Files fetched: 1000
  - python: ai 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: GEMINI 174, CODESTRAL 171, LLAMA 155
- The machine side is post-extraction code: the dataset authors kept the code block and dropped the surrounding chat answer, so SLOP001-SLOP004 are measured at their hardest case here, a floor rather than a typical number.
- Both sides are competitive-programming solutions: short, single-file, few abstractions. SLOP037, SLOP039 and SLOP040 have little to bite, and this false-positive rate does not transfer to application code.
- AtCoder is Japanese and many human files carry Japanese comments, which the English-lexicon rules cannot match.
- The machine side is entirely code that failed; the human side is one third accepted -- the dataset authors matched outcome buckets deliberately.

#### aigcodeset / python

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /KLoC | ai /KLoC |
|---|---|---|---|---|---|---|---|
| SLOP037 | 27 (5.40%) | 21 (4.20%) | 0.44 | 0.44 | 0.78 | 2.85 | 2.83 |
| SLOP006 | 1 (0.20%) | 2 (0.40%) | 0.67* | 0.67* | 2.00 | 0.09 | 0.20 |
| SLOP039 | 5 (1.00%) | 2 (0.40%) | 0.29* | 0.29* | 0.40 | 0.46 | 0.20 |
| SLOP042 | 1 (0.20%) | 2 (0.40%) | 0.67* | 0.67* | 2.00 | 0.09 | 0.20 |
| SLOP003 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.20 |
| SLOP001 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP002 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP004 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP008 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP009 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP040 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP043 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 1 (0.20%) | 3 (0.60%) | 0.75 | 0.75 | 3.00 |  |  |
| **any rule** | 34 (6.80%) | 28 (5.60%) | 0.45 | 0.45 | 0.82 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | CODESTRAL | GEMINI | LLAMA |
| --- | --- | --- | --- |
| SLOP037 | 3.5% | 2.9% | 6.5% |


### apt-eval

- Source: https://huggingface.co/datasets/smksaha/apt-eval
- Paper: arXiv 2502.15666
- License: CC-BY-4.0
- Natural language: en
- Generators: GPT-4o, Llama 3.1 70B, Llama 3 8B and DeepSeek-V3 as polishers (2024-25)
- Human split: unverified -- human blog, email, news, review and speech texts drawn from MixSet and similar 2024 collections
- Revision: 1a183126ec24
- Files fetched: 800
  - prose: ai-polished 500 (0 blank dropped), human 300 (0 blank dropped)
    - ai-polished generators: GPT-4o 109, Llama3-8B 109, DeepSeek-V3 108, Llama3.1-70B 108, Llama2-7B 66
- `ai-polished` is the human text after an LLM polish of a stated degree (`domain` records domain/polish type/degree); there is no plain-ai split, so this dataset measures how far a light polish moves each rule.

#### apt-eval / prose

| rule | human hits |
|---|---|
| SLOP030 | 49 (16.33%) |
| SLOP033 | 27 (9.00%) |
| SLOP018 | 20 (6.67%) |
| SLOP022 | 9 (3.00%) |
| SLOP020 | 7 (2.33%) |
| SLOP015 | 3 (1.00%) |
| SLOP024 | 2 (0.67%) |
| SLOP025 | 2 (0.67%) |
| SLOP014 | 1 (0.33%) |
| SLOP027 | 1 (0.33%) |
| SLOP034 | 1 (0.33%) |
| SLOP036 | 1 (0.33%) |
| SLOP011 | 0 (0.00%) |
| SLOP012 | 0 (0.00%) |
| SLOP013 | 0 (0.00%) |
| SLOP016 | 0 (0.00%) |
| SLOP017 | 0 (0.00%) |
| SLOP019 | 0 (0.00%) |
| SLOP021 | 0 (0.00%) |
| SLOP023 | 0 (0.00%) |
| SLOP026 | 0 (0.00%) |
| SLOP028 | 0 (0.00%) |
| SLOP029 | 0 (0.00%) |
| SLOP031 | 0 (0.00%) |
| SLOP032 | 0 (0.00%) |
| SLOP035 | 0 (0.00%) |
| SLOP041 | 0 (0.00%) |
| **any Tier A rule** | 0 (0.00%) |
| **any rule** | 103 (34.33%) |


### codemirage

- Source: https://huggingface.co/datasets/HanxiGuo/CodeMirage
- Paper: arXiv 2506.11059
- License: CC-BY-NC-ND-4.0
- Natural language: n/a (code)
- Generators: ten LLMs, 2025 (source column)
- Human split: unverified -- CodeParrot github-code-clean, a GitHub snapshot from May 2022
- Revision: 174c25ffa9e9
- Files fetched: 4478
  - go: ai 500 (0 blank dropped), ai-paraphrased 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: claude-3.5-haiku 50, deepseek-r1 50, deepseek-v3 50, gemini-2.0-flash 50, gemini-2.0-flash-thinking-exp 50, gemini-2.0-pro-exp 50, gpt-4o-mini 50, llama3.3-70b 50
    - ai-paraphrased generators: claude-3.5-haiku 50, deepseek-r1 50, deepseek-v3 50, gemini-2.0-flash 50, gemini-2.0-flash-thinking-exp 50, gemini-2.0-pro-exp 50, gpt-4o-mini 50, llama3.3-70b 50
  - python: ai 500 (0 blank dropped), ai-paraphrased 499 (1 blank dropped), human 500 (0 blank dropped)
    - ai generators: claude-3.5-haiku 50, deepseek-r1 50, deepseek-v3 50, gemini-2.0-flash 50, gemini-2.0-flash-thinking-exp 50, gemini-2.0-pro-exp 50, gpt-4o-mini 50, llama3.3-70b 50
    - ai-paraphrased generators: claude-3.5-haiku 50, deepseek-v3 50, gemini-2.0-flash 50, gemini-2.0-flash-thinking-exp 50, gemini-2.0-pro-exp 50, gpt-4o-mini 50, llama3.3-70b 50, o3-mini 50
  - typescript: ai 500 (0 blank dropped), ai-paraphrased 500 (0 blank dropped), human 479 (0 blank dropped)
    - ai generators: claude-3.5-haiku 50, deepseek-r1 50, deepseek-v3 50, gemini-2.0-flash 50, gemini-2.0-flash-thinking-exp 50, gemini-2.0-pro-exp 50, gpt-4o-mini 50, llama3.3-70b 50
    - ai-paraphrased generators: claude-3.5-haiku 50, deepseek-r1 50, deepseek-v3 50, gemini-2.0-flash 50, gemini-2.0-flash-thinking-exp 50, gemini-2.0-pro-exp 50, gpt-4o-mini 50, llama3.3-70b 50
- javascript written as .ts; SLOP007 cannot fire
- CC-BY-NC-ND-4.0 forbids redistributing a derivative; measurement only, nothing from this dataset is republished here beyond aggregate counts.
- Every AI file is regenerated to match its paired human file's line count and size and filtered to BLEU < 0.5 against it, so length-based tells are suppressed by construction.

#### codemirage / go

| rule | human hits | ai hits | ai-paraphrased hits | precision @1:1 | precision @prior | lift | human /KLoC | ai /KLoC |
|---|---|---|---|---|---|---|---|---|
| SLOP042 | 30 (6.00%) | 58 (11.60%) | 78 (15.60%) | 0.66 | 0.66 | 1.93 | 1.14 | 1.48 |
| SLOP009 | 6 (1.20%) | 39 (7.80%) | 36 (7.20%) | 0.87 | 0.87 | 6.50 | 0.24 | 1.22 |
| SLOP037 | 49 (9.80%) | 38 (7.60%) | 29 (5.80%) | 0.44 | 0.44 | 0.78 | 1.22 | 1.04 |
| SLOP003 | 0 (0.00%) | 37 (7.40%) | 2 (0.40%) | 1.00 | 1.00 | inf | 0.00 | 0.56 |
| SLOP008 | 6 (1.20%) | 11 (2.20%) | 10 (2.00%) | 0.65* | 0.65* | 1.83 | 0.13 | 0.21 |
| SLOP039 | 2 (0.40%) | 9 (1.80%) | 12 (2.40%) | 0.82* | 0.82* | 4.50 | 0.03 | 0.15 |
| SLOP002 | 0 (0.00%) | 1 (0.20%) | 0 (0.00%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP005 | 1 (0.20%) | 1 (0.20%) | 0 (0.00%) | 0.50* | 0.50* | 1.00 | 0.02 | 0.04 |
| SLOP043 | 12 (2.40%) | 1 (0.20%) | 1 (0.20%) | 0.08* | 0.08* | 0.08 | 0.22 | 0.01 |
| SLOP001 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP004 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 12 (2.40%) | 86 (17.20%) | 47 (9.40%) | 0.88 | 0.88 | 7.17 |  |  |
| **any rule** | 93 (18.60%) | 167 (33.40%) | 143 (28.60%) | 0.64 | 0.64 | 1.80 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | claude-3.5-haiku | deepseek-r1 | deepseek-v3 | gemini-2.0-flash | gemini-2.0-flash-thinking-exp | gemini-2.0-pro-exp | gpt-4o-mini | llama3.3-70b | o3-mini | qwen2.5-coder |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| SLOP003 | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 70.0% | 0.0% | 0.0% | 0.0% | 4.0% |
| SLOP009 | 4.0% | 2.0% | 2.0% | 4.0% | 10.0% | 0.0% | 12.0% | 16.0% | 16.0% | 12.0% |
| SLOP037 | 8.0% | 2.0% | 10.0% | 6.0% | 8.0% | 6.0% | 14.0% | 4.0% | 12.0% | 6.0% |
| SLOP042 | 4.0% | 0.0% | 4.0% | 8.0% | 10.0% | 2.0% | 14.0% | 14.0% | 54.0% | 6.0% |

#### codemirage / python

| rule | human hits | ai hits | ai-paraphrased hits | precision @1:1 | precision @prior | lift | human /KLoC | ai /KLoC |
|---|---|---|---|---|---|---|---|---|
| SLOP042 | 32 (6.40%) | 68 (13.60%) | 83 (16.63%) | 0.68 | 0.68 | 2.12 | 0.94 | 2.78 |
| SLOP006 | 26 (5.20%) | 67 (13.40%) | 64 (12.83%) | 0.72 | 0.72 | 2.58 | 0.90 | 2.42 |
| SLOP003 | 0 (0.00%) | 25 (5.00%) | 3 (0.60%) | 1.00 | 1.00 | inf | 0.00 | 0.58 |
| SLOP009 | 3 (0.60%) | 25 (5.00%) | 26 (5.21%) | 0.89 | 0.89 | 8.33 | 0.25 | 1.11 |
| SLOP008 | 23 (4.60%) | 20 (4.00%) | 22 (4.41%) | 0.47 | 0.47 | 0.87 | 1.08 | 0.77 |
| SLOP037 | 8 (1.60%) | 13 (2.60%) | 14 (2.81%) | 0.62 | 0.62 | 1.62 | 0.25 | 0.41 |
| SLOP039 | 1 (0.20%) | 8 (1.60%) | 9 (1.80%) | 0.89* | 0.89* | 8.00 | 0.02 | 0.17 |
| SLOP043 | 29 (5.80%) | 1 (0.20%) | 1 (0.20%) | 0.03 | 0.03 | 0.03 | 0.74 | 0.02 |
| SLOP001 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP002 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP004 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP040 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 50 (10.00%) | 123 (24.60%) | 103 (20.64%) | 0.71 | 0.71 | 2.46 |  |  |
| **any rule** | 114 (22.80%) | 187 (37.40%) | 192 (38.48%) | 0.62 | 0.62 | 1.64 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | claude-3.5-haiku | deepseek-r1 | deepseek-v3 | gemini-2.0-flash | gemini-2.0-flash-thinking-exp | gemini-2.0-pro-exp | gpt-4o-mini | llama3.3-70b | o3-mini | qwen2.5-coder |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| SLOP003 | 2.0% | 0.0% | 0.0% | 0.0% | 0.0% | 24.0% | 0.0% | 0.0% | 0.0% | 24.0% |
| SLOP006 | 12.0% | 14.0% | 8.0% | 14.0% | 12.0% | 12.0% | 20.0% | 4.0% | 30.0% | 8.0% |
| SLOP008 | 2.0% | 0.0% | 2.0% | 8.0% | 0.0% | 2.0% | 4.0% | 14.0% | 2.0% | 6.0% |
| SLOP009 | 4.0% | 8.0% | 2.0% | 4.0% | 4.0% | 0.0% | 8.0% | 10.0% | 2.0% | 8.0% |
| SLOP042 | 12.0% | 2.0% | 12.0% | 2.0% | 2.0% | 2.0% | 20.0% | 32.0% | 42.0% | 10.0% |

#### codemirage / typescript

| rule | human hits | ai hits | ai-paraphrased hits | precision @1:1 | precision @prior | lift | human /KLoC | ai /KLoC |
|---|---|---|---|---|---|---|---|---|
| SLOP042 | 9 (1.88%) | 65 (13.00%) | 66 (13.20%) | 0.87 | 0.88 | 6.92 | 0.50 | 2.85 |
| SLOP009 | 3 (0.63%) | 43 (8.60%) | 43 (8.60%) | 0.93 | 0.93 | 13.73 | 0.23 | 2.53 |
| SLOP003 | 0 (0.00%) | 26 (5.20%) | 4 (0.80%) | 1.00 | 1.00 | inf | 0.00 | 0.81 |
| SLOP005 | 3 (0.63%) | 21 (4.20%) | 20 (4.00%) | 0.87 | 0.88 | 6.71 | 0.13 | 0.92 |
| SLOP038 | 12 (2.51%) | 20 (4.00%) | 20 (4.00%) | 0.61 | 0.62 | 1.60 | 0.47 | 0.83 |
| SLOP037 | 5 (1.04%) | 12 (2.40%) | 14 (2.80%) | 0.70* | 0.71* | 2.30 | 0.17 | 0.35 |
| SLOP039 | 1 (0.21%) | 3 (0.60%) | 9 (1.80%) | 0.74* | 0.75* | 2.87 | 0.03 | 0.09 |
| SLOP043 | 16 (3.34%) | 3 (0.60%) | 3 (0.60%) | 0.15* | 0.16* | 0.18 | 0.54 | 0.09 |
| SLOP002 | 0 (0.00%) | 2 (0.40%) | 0 (0.00%) | 1.00* | 1.00* | inf | 0.00 | 0.06 |
| SLOP007 | 0 (0.00%) | 1 (0.20%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.03 |
| SLOP001 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP004 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP008 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP040 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 6 (1.25%) | 86 (17.20%) | 61 (12.20%) | 0.93 | 0.93 | 13.73 |  |  |
| **any rule** | 47 (9.81%) | 165 (33.00%) | 148 (29.60%) | 0.77 | 0.78 | 3.36 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | claude-3.5-haiku | deepseek-r1 | deepseek-v3 | gemini-2.0-flash | gemini-2.0-flash-thinking-exp | gemini-2.0-pro-exp | gpt-4o-mini | llama3.3-70b | o3-mini | qwen2.5-coder |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| SLOP003 | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% | 48.0% | 0.0% | 0.0% | 0.0% | 4.0% |
| SLOP005 | 12.0% | 2.0% | 4.0% | 0.0% | 2.0% | 4.0% | 6.0% | 0.0% | 6.0% | 6.0% |
| SLOP009 | 12.0% | 6.0% | 12.0% | 4.0% | 6.0% | 2.0% | 12.0% | 8.0% | 10.0% | 14.0% |
| SLOP038 | 2.0% | 6.0% | 2.0% | 6.0% | 2.0% | 6.0% | 2.0% | 6.0% | 2.0% | 6.0% |
| SLOP042 | 12.0% | 0.0% | 12.0% | 4.0% | 10.0% | 2.0% | 24.0% | 32.0% | 22.0% | 12.0% |


### codet_m4

- Source: https://huggingface.co/datasets/DaniilOr/CoDET-M4
- Paper: arXiv 2503.13733
- License: MIT
- Natural language: n/a (code)
- Generators: GPT-4o, Llama 3, Qwen and other 2024 models (model column)
- Human split: unverified -- LeetCode and Codeforces solutions, undated, plus CodeSearchNet (2019)
- Revision: 4d4e665037cb
- Files fetched: 1000
  - python: ai 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: llama3.1 155, codellama 145, qwen1.5 131, gpt 39, nxcode 30
- Comments and docstrings are stripped by the publisher's extraction pass, so SLOP001-SLOP004, SLOP042 and SLOP043 print `n/a` here instead of a number that would just measure the extraction, not the code.
- `n/a` rules on this dataset: SLOP001, SLOP002, SLOP003, SLOP004, SLOP042, SLOP043

#### codet_m4 / python

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /KLoC | ai /KLoC |
|---|---|---|---|---|---|---|---|
| SLOP037 | 16 (3.20%) | 35 (7.00%) | 0.69 | 0.69 | 2.19 | 0.68 | 6.55 |
| SLOP008 | 2 (0.40%) | 8 (1.60%) | 0.80* | 0.80* | 4.00 | 0.07 | 1.16 |
| SLOP009 | 1 (0.20%) | 6 (1.20%) | 0.86* | 0.86* | 6.00 | 0.03 | 1.60 |
| SLOP039 | 4 (0.80%) | 1 (0.20%) | 0.20* | 0.20* | 0.25 | 0.14 | 0.15 |
| SLOP001 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| SLOP002 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| SLOP003 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| SLOP004 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| SLOP006 | 10 (2.00%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.38 | 0.00 |
| SLOP040 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP042 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| SLOP043 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| **any Tier A rule** | 13 (2.60%) | 14 (2.80%) | 0.52 | 0.52 | 1.08 |  |  |
| **any rule** | 44 (8.80%) | 76 (15.20%) | 0.63 | 0.63 | 1.73 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | codellama | gpt | llama3.1 | nxcode | qwen1.5 |
| --- | --- | --- | --- | --- | --- |
| SLOP037 | 10.3% | 5.1% | 6.5% | 0.0% | 6.1% |


### droid

- Source: https://huggingface.co/datasets/project-droid/DroidCollection
- Paper: arXiv 2507.10583
- License: not stated on the dataset card
- Natural language: n/a (code)
- Generators: open and API code models, 2024-25 (Generator column)
- Human split: unverified -- GitHub, LeetCode and Codeforces code collected 2024-25; the paper says the human class may contain assistant-written code
- Revision: 9a42843be994
- Files fetched: 5000
  - go: ai 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: GPT-4o-mini 339, GPT-4o 30, Qwen/Qwen2.5-Coder-7B-Instruct 20, deepseek-ai/deepseek-coder-6.7b-instruct 13, meta-llama/Llama-3.3-70B-Instruct 11, meta-llama/Llama-3.1-8B-Instruct 9, Qwen/Qwen2.5-Codder-14B-Instruct 8, 01-ai/Yi-Coder-9B-Chat 8
  - python: ai 500 (0 blank dropped), ai-adversarial 500 (0 blank dropped), ai-refined 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: GPT-4o-mini 144, Qwen/Qwen2.5-Coder-7B-Instruct 21, GPT-4o 20, Qwen/Qwen2.5-Coder-7B 20, meta-llama/Llama-3.3-70B-Instruct 20, microsoft/Phi-3-medium-4k-instruct 20, codellama/CodeLlama-34b-Instruct-hf 18, 01-ai/Yi-Coder-1.5B-Chat 17
    - ai-adversarial generators: Qwen/Qwen2.5-Coder-7B-Instruct 200, deepseek-ai/deepseek-coder-6.7b-instruct 100, microsoft/Phi-3.5-mini-instruct 100, 01-ai/Yi-Coder-9B-Chat 100
    - ai-refined generators: Qwen/Qwen2.5-72B-Instruct 125, ibm-granite/granite-34b-code-instruct-8k 118, Qwen/Qwen2.5-Coder-32B-Instruct 31, 01-ai/Yi-Coder-9B-Chat 29, google/codegemma-7b-it 24, codellama/CodeLlama-70b-Instruct-hf 21, Qwen/Qwen2.5-Coder-7B-Instruct 17, meta-llama/Llama-3.1-8B-Instruct 17
  - rust: ai-adversarial 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai-adversarial generators: deepseek-ai/deepseek-coder-6.7b-instruct 148, Qwen/Qwen2.5-Coder-7B-Instruct 124, Qwen/Qwen2.5-Coder-1.5B-Instruct 62, microsoft/Phi-3-mini-4k-instruct 59, deepseek-ai/deepseek-coder-1.3b-instruct 52, microsoft/Phi-3.5-mini-instruct 41, 01-ai/Yi-Coder-1.5B-Chat 14
  - typescript: ai 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: GPT-4o-mini 140, GPT-4o 85, Qwen/Qwen2.5-Coder-7B-Instruct 33, meta-llama/Llama-3.1-8B-Instruct 20, 01-ai/Yi-Coder-9B-Chat 19, Qwen/Qwen2.5-Coder-1.5B-Instruct 17, meta-llama/Llama-3.3-70B-Instruct 17, Qwen/Qwen2.5-72B-Instruct 14
- javascript written as .ts; SLOP007 cannot fire
- MACHINE_REFINED exists for Python only on this dataset; the adversarial split exists for Python and Rust only.
- DroidCollection ships no plain machine-generated Rust; the adversarial label is the only AI Rust it has.

#### droid / go

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /KLoC | ai /KLoC |
|---|---|---|---|---|---|---|---|
| SLOP042 | 3 (0.60%) | 66 (13.20%) | 0.96 | 0.96 | 22.00 | 0.21 | 2.94 |
| SLOP037 | 10 (2.00%) | 35 (7.00%) | 0.78 | 0.78 | 3.50 | 0.82 | 1.32 |
| SLOP009 | 2 (0.40%) | 32 (6.40%) | 0.94 | 0.94 | 16.00 | 0.14 | 1.26 |
| SLOP005 | 1 (0.20%) | 2 (0.40%) | 0.67* | 0.67* | 2.00 | 0.07 | 0.14 |
| SLOP008 | 1 (0.20%) | 2 (0.40%) | 0.67* | 0.67* | 2.00 | 0.07 | 0.11 |
| SLOP001 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP002 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP003 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP004 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP039 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP043 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 4 (0.80%) | 35 (7.00%) | 0.90 | 0.90 | 8.75 |  |  |
| **any rule** | 17 (3.40%) | 119 (23.80%) | 0.88 | 0.88 | 7.00 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | GPT-4o | GPT-4o-mini | Qwen/Qwen2.5-Coder-7B-Instruct |
| --- | --- | --- | --- |
| SLOP009 | 10.0% | 6.5% | 10.0% |
| SLOP037 | 3.3% | 9.4% | 0.0% |
| SLOP042 | 13.3% | 13.3% | 5.0% |

#### droid / python

| rule | human hits | ai hits | ai-refined hits | ai-adversarial hits | precision @1:1 | precision @prior | lift | human /KLoC | ai /KLoC |
|---|---|---|---|---|---|---|---|---|---|
| SLOP042 | 8 (1.60%) | 66 (13.20%) | 48 (9.60%) | 53 (10.60%) | 0.89 | 0.89 | 8.25 | 0.72 | 6.73 |
| SLOP006 | 12 (2.40%) | 35 (7.00%) | 13 (2.60%) | 15 (3.00%) | 0.74 | 0.74 | 2.92 | 1.22 | 2.60 |
| SLOP009 | 1 (0.20%) | 28 (5.60%) | 5 (1.00%) | 11 (2.20%) | 0.97 | 0.97 | 28.00 | 0.21 | 2.29 |
| SLOP037 | 21 (4.20%) | 20 (4.00%) | 12 (2.40%) | 3 (0.60%) | 0.49 | 0.49 | 0.95 | 2.44 | 1.12 |
| SLOP008 | 5 (1.00%) | 8 (1.60%) | 18 (3.60%) | 13 (2.60%) | 0.62* | 0.62* | 1.60 | 0.43 | 0.82 |
| SLOP043 | 2 (0.40%) | 6 (1.20%) | 0 (0.00%) | 2 (0.40%) | 0.75* | 0.75* | 3.00 | 0.21 | 0.31 |
| SLOP002 | 0 (0.00%) | 3 (0.60%) | 2 (0.40%) | 2 (0.40%) | 1.00* | 1.00* | inf | 0.00 | 0.15 |
| SLOP040 | 0 (0.00%) | 2 (0.40%) | 0 (0.00%) | 0 (0.00%) | 1.00* | 1.00* | inf | 0.00 | 0.10 |
| SLOP001 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | 1 (0.20%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP003 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | 64 (12.80%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP004 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP039 | 1 (0.20%) | 0 (0.00%) | 1 (0.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.07 | 0.00 |
| **any Tier A rule** | 18 (3.60%) | 64 (12.80%) | 37 (7.40%) | 96 (19.20%) | 0.78 | 0.78 | 3.56 |  |  |
| **any rule** | 50 (10.00%) | 133 (26.60%) | 94 (18.80%) | 138 (27.60%) | 0.73 | 0.73 | 2.66 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | GPT-4o | GPT-4o-mini | Qwen/Qwen2.5-Coder-7B | Qwen/Qwen2.5-Coder-7B-Instruct | meta-llama/Llama-3.3-70B-Instruct | microsoft/Phi-3-medium-4k-instruct |
| --- | --- | --- | --- | --- | --- | --- |
| SLOP006 | 0.0% | 20.1% | 0.0% | 0.0% | 0.0% | 0.0% |
| SLOP009 | 0.0% | 18.1% | 0.0% | 0.0% | 0.0% | 0.0% |
| SLOP037 | 0.0% | 2.1% | 10.0% | 4.8% | 0.0% | 0.0% |
| SLOP042 | 20.0% | 27.8% | 5.0% | 4.8% | 20.0% | 10.0% |

#### droid / rust

| rule | human hits | ai-adversarial hits |
|---|---|---|
| SLOP043 | 27 (5.40%) | 11 (2.20%) |
| SLOP042 | 19 (3.80%) | 28 (5.60%) |
| SLOP005 | 1 (0.20%) | 0 (0.00%) |
| SLOP001 | 0 (0.00%) | 3 (0.60%) |
| SLOP002 | 0 (0.00%) | 1 (0.20%) |
| SLOP003 | 0 (0.00%) | 55 (11.00%) |
| SLOP004 | 0 (0.00%) | 0 (0.00%) |
| SLOP008 | 0 (0.00%) | 1 (0.20%) |
| SLOP009 | 0 (0.00%) | 6 (1.20%) |
| SLOP037 | 0 (0.00%) | 0 (0.00%) |
| SLOP039 | 0 (0.00%) | 3 (0.60%) |
| **any Tier A rule** | 1 (0.20%) | 63 (12.60%) |
| **any rule** | 46 (9.20%) | 95 (19.00%) |

#### droid / typescript

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /KLoC | ai /KLoC |
|---|---|---|---|---|---|---|---|
| SLOP042 | 19 (3.80%) | 104 (20.80%) | 0.85 | 0.85 | 5.47 | 1.27 | 8.66 |
| SLOP009 | 4 (0.80%) | 49 (9.80%) | 0.92 | 0.92 | 12.25 | 0.19 | 3.91 |
| SLOP005 | 6 (1.20%) | 25 (5.00%) | 0.81 | 0.81 | 4.17 | 0.35 | 1.32 |
| SLOP038 | 7 (1.40%) | 9 (1.80%) | 0.56* | 0.56* | 1.29 | 0.31 | 0.42 |
| SLOP037 | 1 (0.20%) | 7 (1.40%) | 0.88* | 0.88* | 7.00 | 0.04 | 0.30 |
| SLOP043 | 4 (0.80%) | 4 (0.80%) | 0.50* | 0.50* | 1.00 | 0.15 | 0.17 |
| SLOP002 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.04 |
| SLOP001 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP003 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP004 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP007 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP008 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP039 | 1 (0.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.04 | 0.00 |
| SLOP040 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 10 (2.00%) | 68 (13.60%) | 0.87 | 0.87 | 6.80 |  |  |
| **any rule** | 42 (8.40%) | 163 (32.60%) | 0.80 | 0.80 | 3.88 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | GPT-4o | GPT-4o-mini | Qwen/Qwen2.5-Coder-7B-Instruct | meta-llama/Llama-3.1-8B-Instruct |
| --- | --- | --- | --- | --- |
| SLOP005 | 2.4% | 10.0% | 6.1% | 0.0% |
| SLOP009 | 7.1% | 16.4% | 9.1% | 5.0% |
| SLOP042 | 2.4% | 53.6% | 9.1% | 10.0% |


### essay-br

- Source: https://raw.githubusercontent.com/rafaelanchieta/essay/master/essay-br/essay-br.csv
- License: MIT
- Natural language: pt
- Human split: unverified -- ENEM-style student essays published on a correction site 2015-2020; the 2020 tail postdates the bar and authors are anonymous
- Revision: n/a
- Files fetched: 500
  - prose: human 500 (0 blank dropped)
- Human-only: student essays for a national exam prompt.
- ENEM pedagogy teaches the recap-connective close, so SLOP029-family signals read as genre here, not as a tell.

#### essay-br / prose

| rule | human hits |
|---|---|
| SLOP033 | 233 (46.60%) |
| SLOP020 | 116 (23.20%) |
| SLOP018 | 23 (4.60%) |
| SLOP022 | 11 (2.20%) |
| SLOP025 | 8 (1.60%) |
| SLOP017 | 6 (1.20%) |
| SLOP030 | 6 (1.20%) |
| SLOP041 | 6 (1.20%) |
| SLOP029 | 2 (0.40%) |
| SLOP015 | 1 (0.20%) |
| SLOP024 | 1 (0.20%) |
| SLOP011 | 0 (0.00%) |
| SLOP012 | 0 (0.00%) |
| SLOP013 | 0 (0.00%) |
| SLOP014 | 0 (0.00%) |
| SLOP016 | 0 (0.00%) |
| SLOP019 | 0 (0.00%) |
| SLOP021 | 0 (0.00%) |
| SLOP023 | 0 (0.00%) |
| SLOP026 | 0 (0.00%) |
| SLOP027 | 0 (0.00%) |
| SLOP028 | 0 (0.00%) |
| SLOP031 | 0 (0.00%) |
| SLOP032 | 0 (0.00%) |
| SLOP034 | 0 (0.00%) |
| SLOP035 | 0 (0.00%) |
| SLOP036 | 0 (0.00%) |
| **any Tier A rule** | 0 (0.00%) |
| **any rule** | 317 (63.40%) |


### faidset

- Source: https://huggingface.co/datasets/ngocminhta/FAIDSet
- Paper: arXiv 2505.14271
- License: MIT
- Natural language: en
- Generators: GPT-4o, Gemini 2, Llama 3 and DeepSeek V3/R1 (2024-25; model column)
- Human split: unverified -- undated academic theses and essays collected 2024-25; English rows only
- Revision: e2927dd1218b
- Files fetched: 1500
  - prose: ai 500 (0 blank dropped), ai-collab 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: llama 145, gpt 121, deepseek 121, gemini 113
    - ai-collab generators: llama 138, gemini 128, gpt 119, deepseek 115
- The published `test.jsonl` mixes English and Vietnamese with no language column; rows holding a Vietnamese-only letter are dropped before sampling.
- `ai-collab` is the dataset's human-LLM collaborative class, a robustness split never pooled with plain ai.

#### faidset / prose

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /1k words | ai /1k words |
|---|---|---|---|---|---|---|---|
| SLOP017 | 1 (0.20%) | 169 (33.80%) | 0.99 | 0.99 | 169.00 | 0.02 | 1.77 |
| SLOP016 | 2 (0.40%) | 123 (24.60%) | 0.98 | 0.98 | 61.50 | 0.04 | 1.29 |
| SLOP024 | 6 (1.20%) | 51 (10.20%) | 0.89 | 0.89 | 8.50 | 0.13 | 0.53 |
| SLOP030 | 45 (9.00%) | 33 (6.60%) | 0.42 | 0.42 | 0.73 | 0.95 | 0.35 |
| SLOP032 | 2 (0.40%) | 20 (4.00%) | 0.91 | 0.91 | 10.00 | 0.04 | 0.21 |
| SLOP014 | 1 (0.20%) | 5 (1.00%) | 0.83* | 0.83* | 5.00 | 0.02 | 0.05 |
| SLOP041 | 2 (0.40%) | 4 (0.80%) | 0.67* | 0.67* | 2.00 | 0.04 | 0.04 |
| SLOP033 | 43 (8.60%) | 3 (0.60%) | 0.07 | 0.07 | 0.07 | 0.99 | 0.03 |
| SLOP020 | 57 (11.40%) | 2 (0.40%) | 0.03 | 0.03 | 0.04 | 1.21 | 0.02 |
| SLOP027 | 2 (0.40%) | 2 (0.40%) | 0.50* | 0.50* | 1.00 | 0.04 | 0.02 |
| SLOP018 | 13 (2.60%) | 1 (0.20%) | 0.07* | 0.07* | 0.08 | 0.38 | 0.02 |
| SLOP011 | 2 (0.40%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.04 | 0.00 |
| SLOP012 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP013 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP015 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP019 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP021 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP022 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP023 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP025 | 2 (0.40%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.04 | 0.00 |
| SLOP026 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP028 | 1 (0.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.02 | 0.00 |
| SLOP029 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP031 | 8 (1.60%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.17 | 0.00 |
| SLOP034 | 3 (0.60%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.06 | 0.00 |
| SLOP035 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP036 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 2 (0.40%) | 0 (0.00%) | 0.00 | 0.00 | 0.00 |  |  |
| **any rule** | 146 (29.20%) | 297 (59.40%) | 0.67 | 0.67 | 2.03 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | deepseek | gemini | gpt | llama |
| --- | --- | --- | --- | --- |
| SLOP016 | 14.9% | 9.7% | 69.4% | 6.9% |
| SLOP017 | 38.0% | 25.7% | 40.5% | 31.0% |
| SLOP024 | 9.1% | 5.3% | 5.0% | 19.3% |
| SLOP030 | 2.5% | 3.5% | 2.5% | 15.9% |
| SLOP032 | 7.4% | 0.9% | 5.8% | 2.1% |


### hairosetta

- Source: https://huggingface.co/datasets/isThisYouLLM/H-AIRosettaMP
- Paper: arXiv 2412.14611
- License: MIT
- Natural language: en
- Generators: StarCoder2 (2024)
- Human split: unverified -- Rosetta Code wiki solutions, retrieved 2022-07-01, after Copilot shipped
- Revision: 2bd3dc134a28
- Files fetched: 3998
  - go: ai-translated 500 (0 blank dropped), human 500 (0 blank dropped)
  - python: ai-translated 499 (1 blank dropped), human 500 (0 blank dropped)
  - rust: ai-translated 500 (0 blank dropped), human 500 (0 blank dropped)
  - typescript: ai-translated 500 (0 blank dropped), human 499 (1 blank dropped)
- javascript written as .ts; SLOP007 cannot fire
- The ai side is StarCoder2 translating a human solution from another language into this one (named in the `set` column, e.g. `Rust_from_Java`), not writing from a task prompt; translated code may carry different tells than prompted code, so every ai cell here is labelled `ai-translated`, never plain `ai`.
- Rosetta Code is already registered on its own (`rosetta`); this is the derivative that adds the ai half, not a re-proposal.

#### hairosetta / go

| rule | human hits |
|---|---|
| SLOP037 | 14 (2.80%) |
| SLOP042 | 14 (2.80%) |
| SLOP043 | 8 (1.60%) |
| SLOP009 | 5 (1.00%) |
| SLOP008 | 2 (0.40%) |
| SLOP001 | 0 (0.00%) |
| SLOP002 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP005 | 0 (0.00%) |
| SLOP039 | 0 (0.00%) |
| **any Tier A rule** | 7 (1.40%) |
| **any rule** | 39 (7.80%) |

#### hairosetta / python

| rule | human hits |
|---|---|
| SLOP006 | 9 (1.80%) |
| SLOP042 | 8 (1.60%) |
| SLOP009 | 7 (1.40%) |
| SLOP037 | 7 (1.40%) |
| SLOP008 | 4 (0.80%) |
| SLOP043 | 2 (0.40%) |
| SLOP002 | 1 (0.20%) |
| SLOP001 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP039 | 0 (0.00%) |
| SLOP040 | 0 (0.00%) |
| **any Tier A rule** | 20 (4.00%) |
| **any rule** | 37 (7.40%) |

#### hairosetta / rust

| rule | human hits |
|---|---|
| SLOP042 | 16 (3.20%) |
| SLOP043 | 10 (2.00%) |
| SLOP009 | 5 (1.00%) |
| SLOP039 | 3 (0.60%) |
| SLOP001 | 0 (0.00%) |
| SLOP002 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP005 | 0 (0.00%) |
| SLOP008 | 0 (0.00%) |
| SLOP037 | 0 (0.00%) |
| **any Tier A rule** | 5 (1.00%) |
| **any rule** | 34 (6.80%) |

#### hairosetta / typescript

| rule | human hits |
|---|---|
| SLOP042 | 53 (10.62%) |
| SLOP043 | 8 (1.60%) |
| SLOP037 | 5 (1.00%) |
| SLOP008 | 1 (0.20%) |
| SLOP009 | 1 (0.20%) |
| SLOP039 | 1 (0.20%) |
| SLOP001 | 0 (0.00%) |
| SLOP002 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP005 | 0 (0.00%) |
| SLOP007 | 0 (0.00%) |
| SLOP038 | 0 (0.00%) |
| SLOP040 | 0 (0.00%) |
| **any Tier A rule** | 2 (0.40%) |
| **any rule** | 64 (12.83%) |


### pan25

- Source: https://zenodo.org/records/14962653
- License: research use only, no redistribution (Zenodo record terms)
- Natural language: en
- Generators: 23 models spanning 2023-2025, including gpt-4o, o3-mini, gemini-2.0-flash, deepseek-r1-distill-qwen-32b, llama-3.3-70b-instruct and gpt-4.5-preview (model column)
- Human split: unverified -- fiction, essay and news human text; the fiction/essay sources could not be confirmed, the news text is dated 2021
- Revision: n/a
- Files fetched: 1500
  - prose: ai 500 (0 blank dropped), ai-paraphrased 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: gpt-4o-mini 60, gpt-3.5-turbo 57, o3-mini 43, falcon3-10b-instruct 39, llama-3.1-8b-instruct 39, gemini-2.0-flash 38, gpt-4o 36, gemini-1.5-pro 35
    - ai-paraphrased generators: gpt-4-turbo-paraphrase 253, gemini-pro-paraphrase 247
- The dataset's license permits research use only and forbids redistribution; fetched and scored locally like CodeMirage, but never quote a line from it anywhere, including in an issue.
- `gpt-4-turbo-paraphrase` and `gemini-pro-paraphrase` are machine-humanized rewrites of existing ai text, not organic generations; they are labelled `ai-paraphrased` and never pooled with plain `ai`.

#### pan25 / prose

| rule | human hits | ai hits | ai-paraphrased hits | precision @1:1 | precision @prior | lift | human /1k words | ai /1k words |
|---|---|---|---|---|---|---|---|---|
| SLOP018 | 136 (27.20%) | 232 (46.40%) | 85 (17.00%) | 0.63 | 0.63 | 1.71 | 1.65 | 2.79 |
| SLOP020 | 226 (45.20%) | 158 (31.60%) | 91 (18.20%) | 0.41 | 0.41 | 0.70 | 0.64 | 0.54 |
| SLOP014 | 2 (0.40%) | 125 (25.00%) | 25 (5.00%) | 0.98 | 0.98 | 62.50 | 0.01 | 0.56 |
| SLOP017 | 54 (10.80%) | 118 (23.60%) | 110 (22.00%) | 0.69 | 0.69 | 2.19 | 0.15 | 0.41 |
| SLOP016 | 0 (0.00%) | 115 (23.00%) | 38 (7.60%) | 1.00 | 1.00 | inf | 0.00 | 0.40 |
| SLOP033 | 306 (61.20%) | 99 (19.80%) | 46 (9.20%) | 0.24 | 0.24 | 0.32 | 2.02 | 0.61 |
| SLOP024 | 5 (1.00%) | 74 (14.80%) | 56 (11.20%) | 0.94 | 0.94 | 14.80 | 0.01 | 0.32 |
| SLOP030 | 212 (42.40%) | 71 (14.20%) | 10 (2.00%) | 0.25 | 0.25 | 0.33 | 0.80 | 0.32 |
| SLOP041 | 15 (3.00%) | 40 (8.00%) | 18 (3.60%) | 0.73 | 0.73 | 2.67 | 0.04 | 0.14 |
| SLOP025 | 3 (0.60%) | 18 (3.60%) | 18 (3.60%) | 0.86 | 0.86 | 6.00 | 0.01 | 0.08 |
| SLOP022 | 26 (5.20%) | 10 (2.00%) | 3 (0.60%) | 0.28 | 0.28 | 0.38 | 0.07 | 0.04 |
| SLOP015 | 10 (2.00%) | 6 (1.20%) | 11 (2.20%) | 0.37* | 0.38* | 0.60 | 0.03 | 0.02 |
| SLOP011 | 0 (0.00%) | 3 (0.60%) | 0 (0.00%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP027 | 5 (1.00%) | 2 (0.40%) | 0 (0.00%) | 0.29* | 0.29* | 0.40 | 0.01 | 0.01 |
| SLOP032 | 0 (0.00%) | 2 (0.40%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP034 | 5 (1.00%) | 2 (0.40%) | 1 (0.20%) | 0.29* | 0.29* | 0.40 | 0.02 | 0.01 |
| SLOP035 | 0 (0.00%) | 2 (0.40%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP036 | 12 (2.40%) | 2 (0.40%) | 2 (0.40%) | 0.14* | 0.14* | 0.17 | 0.03 | 0.01 |
| SLOP023 | 1 (0.20%) | 1 (0.20%) | 0 (0.00%) | 0.50* | 0.50* | 1.00 | 0.00 | 0.00 |
| SLOP028 | 1 (0.20%) | 1 (0.20%) | 1 (0.20%) | 0.50* | 0.50* | 1.00 | 0.00 | 0.00 |
| SLOP012 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP013 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP019 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP021 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP026 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP029 | 0 (0.00%) | 0 (0.00%) | 2 (0.40%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP031 | 1 (0.20%) | 0 (0.00%) | 1 (0.20%) | 0.00* | 0.00* | 0.00 | 0.00 | 0.00 |
| **any Tier A rule** | 0 (0.00%) | 3 (0.60%) | 0 (0.00%) | 1.00 | 1.00 | inf |  |  |
| **any rule** | 466 (93.20%) | 444 (88.80%) | 315 (63.00%) | 0.49 | 0.49 | 0.95 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | deepseek-r1-distill-qwen-32b | falcon3-10b-instruct | gemini-1.5-pro | gemini-2.0-flash | gpt-3.5-turbo | gpt-4o | gpt-4o-mini | llama-3.1-8b-instruct | ministral-8b-instruct-2410 | o3-mini |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| SLOP014 | 33.3% | 69.2% | 8.6% | 21.1% | 19.3% | 19.4% | 31.7% | 15.4% | 16.1% | 32.6% |
| SLOP016 | 16.7% | 41.0% | 17.1% | 28.9% | 15.8% | 44.4% | 28.3% | 15.4% | 16.1% | 25.6% |
| SLOP017 | 16.7% | 28.2% | 8.6% | 23.7% | 24.6% | 25.0% | 31.7% | 17.9% | 25.8% | 20.9% |
| SLOP018 | 73.3% | 30.8% | 48.6% | 34.2% | 15.8% | 83.3% | 76.7% | 28.2% | 19.4% | 86.0% |
| SLOP020 | 50.0% | 12.8% | 28.6% | 36.8% | 0.0% | 41.7% | 81.7% | 2.6% | 16.1% | 86.0% |
| SLOP024 | 13.3% | 17.9% | 17.1% | 10.5% | 14.0% | 16.7% | 16.7% | 17.9% | 6.5% | 7.0% |
| SLOP030 | 16.7% | 5.1% | 34.3% | 52.6% | 10.5% | 2.8% | 3.3% | 2.6% | 32.3% | 11.6% |
| SLOP033 | 6.7% | 10.3% | 11.4% | 10.5% | 7.0% | 11.1% | 30.0% | 46.2% | 6.5% | 58.1% |
| SLOP041 | 0.0% | 0.0% | 0.0% | 0.0% | 15.8% | 0.0% | 0.0% | 5.1% | 0.0% | 0.0% |


### rosetta

- Source: https://huggingface.co/datasets/christopher/rosetta-code
- License: GFDL
- Natural language: n/a (code)
- Human split: unverified -- Rosetta Code wiki solutions in a 2022-23 snapshot; individual edits are undated
- Revision: 11d8b38cbd90
- Files fetched: 1539
  - go: human 500 (0 blank dropped)
  - python: human 500 (0 blank dropped)
  - rust: human 500 (0 blank dropped)
  - typescript: human 39 (0 blank dropped)
- Human-only: every task/language pair is a solution someone wrote for the Rosetta Code wiki, so this dataset measures the false-positive rate alone.
- About nine in ten Python solutions are Python 2 and do not parse under Python 3, so any parse-validity signal has nothing to separate here.

#### rosetta / go

| rule | human hits |
|---|---|
| SLOP037 | 18 (3.60%) |
| SLOP042 | 12 (2.40%) |
| SLOP043 | 9 (1.80%) |
| SLOP008 | 4 (0.80%) |
| SLOP009 | 4 (0.80%) |
| SLOP001 | 0 (0.00%) |
| SLOP002 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP005 | 0 (0.00%) |
| SLOP039 | 0 (0.00%) |
| **any Tier A rule** | 8 (1.60%) |
| **any rule** | 46 (9.20%) |

#### rosetta / python

| rule | human hits |
|---|---|
| SLOP006 | 13 (2.60%) |
| SLOP037 | 6 (1.20%) |
| SLOP042 | 6 (1.20%) |
| SLOP008 | 5 (1.00%) |
| SLOP009 | 3 (0.60%) |
| SLOP039 | 2 (0.40%) |
| SLOP002 | 1 (0.20%) |
| SLOP043 | 1 (0.20%) |
| SLOP001 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP040 | 0 (0.00%) |
| **any Tier A rule** | 21 (4.20%) |
| **any rule** | 36 (7.20%) |

#### rosetta / rust

| rule | human hits |
|---|---|
| SLOP042 | 15 (3.00%) |
| SLOP043 | 11 (2.20%) |
| SLOP009 | 4 (0.80%) |
| SLOP039 | 2 (0.40%) |
| SLOP001 | 0 (0.00%) |
| SLOP002 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP005 | 0 (0.00%) |
| SLOP008 | 0 (0.00%) |
| SLOP037 | 0 (0.00%) |
| **any Tier A rule** | 4 (0.80%) |
| **any rule** | 32 (6.40%) |

#### rosetta / typescript

| rule | human hits |
|---|---|
| SLOP042 | 2 (5.13%) |
| SLOP001 | 0 (0.00%) |
| SLOP002 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP005 | 0 (0.00%) |
| SLOP007 | 0 (0.00%) |
| SLOP008 | 0 (0.00%) |
| SLOP009 | 0 (0.00%) |
| SLOP037 | 0 (0.00%) |
| SLOP038 | 0 (0.00%) |
| SLOP039 | 0 (0.00%) |
| SLOP040 | 0 (0.00%) |
| SLOP043 | 0 (0.00%) |
| **any Tier A rule** | 0 (0.00%) |
| **any rule** | 2 (5.13%) |


### semeval13

- Source: https://huggingface.co/datasets/DaniilOr/SemEval-2026-Task13
- License: Apache-2.0
- Natural language: n/a (code)
- Generators: Qwen2.5-Coder, DeepSeek-Coder, Llama 3.x, GPT-4o and other 2024-25 models (generator column)
- Human split: unverified -- Droid-derived GitHub, LeetCode and Codeforces code collected 2024-25
- Revision: df2aec18238a
- Files fetched: 3999
  - go: ai 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: GPT-4o-mini 268, GPT-4o 21, ibm-granite/granite-3.2-2b-instruct 13, mistralai/Mistral-7B-Instruct-v0.3 11, meta-llama/Meta-Llama-3.1-405B-Instruct 10, mistralai/Mistral-Nemo-Instruct-2407 8, google/gemma-3-4b-it 7, meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo 7
  - python: ai 500 (0 blank dropped), ai-adversarial 500 (0 blank dropped), ai-hybrid 499 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: GPT-4o-mini 118, GPT-4o 19, Qwen/Qwen2.5-Coder-7B 19, Qwen/Qwen2.5-Coder-7B-Instruct 19, meta-llama/Llama-3.1-8B-Instruct 15, 01-ai/Yi-Coder-1.5B-Chat 15, Qwen/Qwen2.5-Coder-1.5B-Instruct 14, meta-llama/Llama-3.2-3B 12
    - ai-adversarial generators: deepseek-ai/deepseek-coder-1.3b-instruct 69, 01-ai/Yi-Coder-1.5B-Chat 58, Qwen/Qwen2.5-Coder-7B-Instruct 56, deepseek-ai/deepseek-coder-6.7b-instruct 56, Qwen/Qwen2.5-Coder-1.5B-Instruct 45, 01-ai/Yi-Coder-9B-Chat 44, Qwen/Qwen2.5-72B-Instruct 28, microsoft/Phi-3-medium-4k-instruct 21
    - ai-hybrid generators: Qwen/Qwen2.5-72B-Instruct 121, ibm-granite/granite-34b-code-instruct-8k 88, Qwen/Qwen2.5-Coder-32B-Instruct 37, google/codegemma-7b-it 26, Qwen/Qwen2.5-Coder-1.5B-Instruct 23, codellama/CodeLlama-70b-Instruct-hf 23, 01-ai/Yi-Coder-9B-Chat 21, Qwen/Qwen2.5-Coder-7B-Instruct 19
  - typescript: ai 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: GPT-4o-mini 89, GPT-4o 56, Qwen/Qwen2.5-Coder-7B-Instruct 22, meta-llama/Llama-4-Scout-17B-16E-Instruct 17, meta-llama/Llama-3.1-8B-Instruct 15, meta-llama/Llama-3.3-70B-Instruct 14, microsoft/phi-4 14, 01-ai/Yi-Coder-1.5B-Chat 13
- javascript written as .ts; SLOP007 cannot fire
- Config C is the four-way subtask: label 0 human, 1 machine, 2 hybrid (human and model in one file), 3 adversarially humanized machine code. Hybrid and adversarial are robustness splits and are never pooled with plain ai.
- Subtask C ships no Rust and no TypeScript; its JavaScript is written as .ts like the other JavaScript cells.

#### semeval13 / go

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /KLoC | ai /KLoC |
|---|---|---|---|---|---|---|---|
| SLOP042 | 8 (1.60%) | 58 (11.60%) | 0.88 | 0.88 | 7.25 | 0.89 | 2.17 |
| SLOP009 | 3 (0.60%) | 30 (6.00%) | 0.91 | 0.91 | 10.00 | 0.25 | 1.17 |
| SLOP037 | 15 (3.00%) | 29 (5.80%) | 0.66 | 0.66 | 1.93 | 1.08 | 1.23 |
| SLOP043 | 2 (0.40%) | 3 (0.60%) | 0.60* | 0.60* | 1.50 | 0.13 | 0.11 |
| SLOP002 | 0 (0.00%) | 2 (0.40%) | 1.00* | 1.00* | inf | 0.00 | 0.06 |
| SLOP001 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP003 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP004 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP005 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP008 | 1 (0.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.06 | 0.00 |
| SLOP039 | 1 (0.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.06 | 0.00 |
| **any Tier A rule** | 4 (0.80%) | 32 (6.40%) | 0.89 | 0.89 | 8.00 |  |  |
| **any rule** | 30 (6.00%) | 111 (22.20%) | 0.79 | 0.79 | 3.70 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | GPT-4o | GPT-4o-mini |
| --- | --- | --- |
| SLOP009 | 0.0% | 6.7% |
| SLOP037 | 0.0% | 8.6% |
| SLOP042 | 4.8% | 13.1% |

#### semeval13 / python

| rule | human hits | ai hits | ai-adversarial hits | precision @1:1 | precision @prior | lift | human /KLoC | ai /KLoC |
|---|---|---|---|---|---|---|---|---|
| SLOP042 | 4 (0.80%) | 69 (13.80%) | 51 (10.20%) | 0.95 | 0.95 | 17.25 | 0.33 | 5.91 |
| SLOP009 | 1 (0.20%) | 36 (7.20%) | 6 (1.20%) | 0.97 | 0.97 | 36.00 | 0.07 | 3.29 |
| SLOP006 | 13 (2.60%) | 27 (5.40%) | 7 (1.40%) | 0.68 | 0.68 | 2.08 | 1.41 | 2.06 |
| SLOP037 | 17 (3.40%) | 17 (3.40%) | 21 (4.20%) | 0.50 | 0.50 | 1.00 | 1.20 | 1.03 |
| SLOP008 | 5 (1.00%) | 15 (3.00%) | 20 (4.00%) | 0.75 | 0.75 | 3.00 | 0.47 | 1.75 |
| SLOP043 | 0 (0.00%) | 2 (0.40%) | 5 (1.00%) | 1.00* | 1.00* | inf | 0.00 | 0.10 |
| SLOP001 | 0 (0.00%) | 1 (0.20%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.05 |
| SLOP039 | 2 (0.40%) | 1 (0.20%) | 1 (0.20%) | 0.33* | 0.33* | 0.50 | 0.13 | 0.05 |
| SLOP002 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP003 | 0 (0.00%) | 0 (0.00%) | 36 (7.20%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP004 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP040 | 0 (0.00%) | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 17 (3.40%) | 73 (14.60%) | 65 (13.00%) | 0.81 | 0.81 | 4.29 |  |  |
| **any rule** | 38 (7.60%) | 139 (27.80%) | 126 (25.20%) | 0.79 | 0.79 | 3.66 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | GPT-4o-mini |
| --- | --- |
| SLOP006 | 14.4% |
| SLOP009 | 22.9% |
| SLOP042 | 26.3% |

#### semeval13 / typescript

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /KLoC | ai /KLoC |
|---|---|---|---|---|---|---|---|
| SLOP042 | 14 (2.80%) | 79 (15.80%) | 0.85 | 0.85 | 5.64 | 0.89 | 6.50 |
| SLOP009 | 3 (0.60%) | 29 (5.80%) | 0.91 | 0.91 | 9.67 | 0.13 | 1.70 |
| SLOP005 | 3 (0.60%) | 19 (3.80%) | 0.86 | 0.86 | 6.33 | 0.13 | 1.23 |
| SLOP037 | 2 (0.40%) | 10 (2.00%) | 0.83* | 0.83* | 5.00 | 0.13 | 0.47 |
| SLOP038 | 6 (1.20%) | 5 (1.00%) | 0.45* | 0.45* | 0.83 | 0.27 | 0.21 |
| SLOP043 | 5 (1.00%) | 5 (1.00%) | 0.50* | 0.50* | 1.00 | 0.22 | 0.21 |
| SLOP002 | 0 (0.00%) | 2 (0.40%) | 1.00* | 1.00* | inf | 0.00 | 0.08 |
| SLOP039 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.04 |
| SLOP001 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP003 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP004 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP007 | 2 (0.40%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.27 | 0.00 |
| SLOP008 | 1 (0.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.04 | 0.00 |
| SLOP040 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 8 (1.60%) | 47 (9.40%) | 0.85 | 0.85 | 5.88 |  |  |
| **any rule** | 30 (6.00%) | 126 (25.20%) | 0.81 | 0.81 | 4.20 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | GPT-4o | GPT-4o-mini | Qwen/Qwen2.5-Coder-7B-Instruct |
| --- | --- | --- | --- |
| SLOP009 | 7.1% | 19.1% | 9.1% |
| SLOP042 | 3.6% | 51.7% | 4.5% |


### wetbench-pt

- Source: https://huggingface.co/datasets/cs928346/WETBench
- Paper: arXiv 2507.03373
- License: CC-BY-NC-SA-4.0
- Natural language: pt
- Generators: GPT-4o mini, Gemini 2.0 Flash, Qwen2.5-7B, Mistral-7B (2024)
- Human split: unverified -- Portuguese Wikipedia paragraphs from 2024 revisions
- Revision: n/a
- Files fetched: 1000
  - prose: ai 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: gemini 125, gpt 125, mistral 125, qwen 125
- Portuguese Wikipedia mixes pt-PT and pt-BR; paragraphs are short so document-level rules like SLOP041 (needs 200 words) rarely apply.

#### wetbench-pt / prose

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /1k words | ai /1k words |
|---|---|---|---|---|---|---|---|
| SLOP033 | 46 (9.20%) | 12 (2.40%) | 0.21 | 0.21 | 0.26 | 1.13 | 0.28 |
| SLOP030 | 5 (1.00%) | 4 (0.80%) | 0.44* | 0.44* | 0.80 | 0.10 | 0.09 |
| SLOP017 | 0 (0.00%) | 3 (0.60%) | 1.00* | 1.00* | inf | 0.00 | 0.07 |
| SLOP018 | 10 (2.00%) | 1 (0.20%) | 0.09* | 0.09* | 0.10 | 0.38 | 0.11 |
| SLOP025 | 1 (0.20%) | 1 (0.20%) | 0.50* | 0.50* | 1.00 | 0.02 | 0.02 |
| SLOP035 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.02 |
| SLOP036 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.02 |
| SLOP041 | 1 (0.20%) | 1 (0.20%) | 0.50* | 0.50* | 1.00 | 0.02 | 0.02 |
| SLOP011 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP012 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP013 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP014 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP015 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP016 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP019 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP020 | 5 (1.00%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.10 | 0.00 |
| SLOP021 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP022 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP023 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP024 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP026 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP027 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP028 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP029 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP031 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP032 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP034 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 0 (0.00%) | 0 (0.00%) | -- | -- | -- |  |  |
| **any rule** | 61 (12.20%) | 23 (4.60%) | 0.27 | 0.27 | 0.38 |  |  |
