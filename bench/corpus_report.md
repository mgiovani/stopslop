## Per-rule hit rate across the corpus registry

- Binary: stopslop 0.5.1, run as `stopslop <dir> --format json --stats --no-config --select ALL`
- --limit: 500  **-- a deterministic spread sample, never a reported number, when non-zero and below a dataset's full size**
- aigcodeset revision: b4d4e69fdbcb
- antd-tsx revision: 4.17.4
- codemirage revision: 174c25ffa9e9
- codet_m4 revision: 4d4e665037cb
- cpython-doc revision: v3.10.0
- cpython-lib revision: v3.10.0
- diplomatrix revision: n/a
- droid revision: 9a42843be994
- essay-br revision: n/a
- ghostbuster revision: 86ebd7259055
- go-std revision: go1.17.3
- hc3 revision: 4d0ff18143b5
- mage revision: 342663f0a2b7
- rosetta revision: 11d8b38cbd90
- rust-book revision: 15aef431579a
- rust-std revision: 1.57.0
- typescript-src revision: v4.5.4
- wetbench-pt revision: n/a
- wikipedia-pt revision: 81aeee176c2c

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

## Summary, by lang

#### python

| rule | aigcodeset H% | aigcodeset A% | codemirage H% | codemirage A% | codet_m4 H% | codet_m4 A% | cpython-lib H% | cpython-lib A% | droid H% | droid A% | rosetta H% | rosetta A% |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| SLOP001 | 0.00% | 0.00% | 0.00% | 0.00% | n/a | n/a | 0.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP002 | 0.00% | 0.00% | 0.00% | 0.00% | n/a | n/a | 0.20% |  | 0.00% | 0.60% | 0.20% |  |
| SLOP003 | 0.00% | 0.20% | 0.00% | 5.00% | n/a | n/a | 0.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP004 | 0.00% | 0.00% | 0.00% | 0.00% | n/a | n/a | 0.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP006 | 0.20% | 0.40% | 5.20% | 13.40% | 2.00% | 0.00% | 29.46% |  | 2.40% | 7.00% | 2.60% |  |
| SLOP008 | 0.00% | 0.00% | 4.60% | 4.00% | 0.40% | 1.60% | 9.42% |  | 1.00% | 1.60% | 1.00% |  |
| SLOP009 | 0.00% | 0.00% | 0.60% | 5.00% | 0.20% | 1.20% | 0.60% |  | 0.20% | 5.60% | 0.60% |  |
| SLOP037 | 5.40% | 4.20% | 1.60% | 2.60% | 3.20% | 7.00% | 3.21% |  | 4.20% | 4.00% | 1.20% |  |
| SLOP039 | 1.00% | 0.40% | 0.20% | 1.60% | 0.80% | 0.20% | 1.00% |  | 0.20% | 0.00% | 0.40% |  |
| SLOP040 | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.80% |  | 0.00% | 0.40% | 0.00% |  |
| SLOP042 | 0.20% | 0.40% | 6.40% | 13.60% | n/a | n/a | 7.01% |  | 1.60% | 13.20% | 1.20% |  |
| SLOP043 | 0.00% | 0.00% | 5.80% | 0.20% | n/a | n/a | 29.66% |  | 0.40% | 1.20% | 0.20% |  |
| **any Tier A rule** | 0.20% | 0.60% | 10.00% | 24.60% | 2.60% | 2.80% | 33.47% |  | 3.60% | 12.80% | 4.20% |  |
| **any rule** | 6.80% | 5.60% | 22.80% | 37.40% | 8.80% | 15.20% | 47.49% |  | 10.00% | 26.60% | 7.20% |  |

#### go

| rule | codemirage H% | codemirage A% | droid H% | droid A% | go-std H% | go-std A% | rosetta H% | rosetta A% |
|---|---|---|---|---|---|---|---|---|
| SLOP001 | 0.00% | 0.00% | 0.00% | 0.00% | 0.20% |  | 0.00% |  |
| SLOP002 | 0.00% | 0.20% | 0.00% | 0.00% | 0.00% |  | 0.00% |  |
| SLOP003 | 0.00% | 7.40% | 0.00% | 0.00% | 0.00% |  | 0.00% |  |
| SLOP004 | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% |  |
| SLOP005 | 0.20% | 0.20% | 0.20% | 0.40% | 0.00% |  | 0.00% |  |
| SLOP008 | 1.20% | 2.20% | 0.20% | 0.40% | 3.60% |  | 0.80% |  |
| SLOP009 | 1.20% | 7.80% | 0.40% | 6.40% | 0.60% |  | 0.80% |  |
| SLOP037 | 9.80% | 7.60% | 2.00% | 7.00% | 0.40% |  | 3.60% |  |
| SLOP039 | 0.40% | 1.80% | 0.00% | 0.00% | 2.40% |  | 0.00% |  |
| SLOP042 | 6.00% | 11.60% | 0.60% | 13.20% | 3.00% |  | 2.40% |  |
| SLOP043 | 2.40% | 0.20% | 0.00% | 0.00% | 18.40% |  | 1.80% |  |
| **any Tier A rule** | 2.40% | 17.20% | 0.80% | 7.00% | 4.40% |  | 1.60% |  |
| **any rule** | 18.60% | 33.40% | 3.40% | 23.80% | 24.60% |  | 9.20% |  |

#### rust

| rule | droid H% | droid A% | rosetta H% | rosetta A% | rust-std H% | rust-std A% |
|---|---|---|---|---|---|---|
| SLOP001 | 0.00% |  | 0.00% |  | 0.00% |  |
| SLOP002 | 0.00% |  | 0.00% |  | 0.00% |  |
| SLOP003 | 0.00% |  | 0.00% |  | 0.00% |  |
| SLOP004 | 0.00% |  | 0.00% |  | 0.00% |  |
| SLOP005 | 0.20% |  | 0.00% |  | 1.80% |  |
| SLOP008 | 0.00% |  | 0.00% |  | 0.00% |  |
| SLOP009 | 0.00% |  | 0.80% |  | 0.20% |  |
| SLOP037 | 0.00% |  | 0.00% |  | 0.00% |  |
| SLOP039 | 0.00% |  | 0.40% |  | 3.60% |  |
| SLOP042 | 3.80% |  | 3.00% |  | 1.20% |  |
| SLOP043 | 5.40% |  | 2.20% |  | 25.40% |  |
| **any Tier A rule** | 0.20% |  | 0.80% |  | 2.00% |  |
| **any rule** | 9.20% |  | 6.40% |  | 29.40% |  |

Plain ai Rust in this table, when present, comes only from the synthesized `synth-rust` cell; no real-world dataset registered here ships plain machine-generated Rust (see the droid caveats below).

#### typescript

| rule | codemirage (js proxy) H% | codemirage (js proxy) A% | droid (js proxy) H% | droid (js proxy) A% | rosetta H% | rosetta A% | typescript-src H% | typescript-src A% |
|---|---|---|---|---|---|---|---|---|
| SLOP001 | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% |  |
| SLOP002 | 0.00% | 0.40% | 0.00% | 0.20% | 0.00% |  | 0.00% |  |
| SLOP003 | 0.00% | 5.20% | 0.00% | 0.00% | 0.00% |  | 0.00% |  |
| SLOP004 | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% |  |
| SLOP005 | 0.63% | 4.20% | 1.20% | 5.00% | 0.00% |  | 1.42% |  |
| SLOP007 | 0.00% | 0.20% | 0.00% | 0.00% | 0.00% |  | 12.06% |  |
| SLOP008 | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.35% |  |
| SLOP009 | 0.63% | 8.60% | 0.80% | 9.80% | 0.00% |  | 0.00% |  |
| SLOP037 | 1.04% | 2.40% | 0.20% | 1.40% | 0.00% |  | 0.35% |  |
| SLOP038 | 2.51% | 4.00% | 1.40% | 1.80% | 0.00% |  | 0.00% |  |
| SLOP039 | 0.21% | 0.60% | 0.20% | 0.00% | 0.00% |  | 3.90% |  |
| SLOP040 | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 1.77% |  |
| SLOP042 | 1.88% | 13.00% | 3.80% | 20.80% | 5.13% |  | 12.41% |  |
| SLOP043 | 3.34% | 0.60% | 0.80% | 0.80% | 0.00% |  | 31.56% |  |
| **any Tier A rule** | 1.25% | 17.20% | 2.00% | 13.60% | 0.00% |  | 12.41% |  |
| **any rule** | 9.81% | 33.00% | 8.40% | 32.60% | 5.13% |  | 37.59% |  |

#### tsx

| rule | antd-tsx H% | antd-tsx A% |
|---|---|---|
| SLOP001 | 0.00% |  |
| SLOP002 | 0.00% |  |
| SLOP003 | 0.00% |  |
| SLOP004 | 0.00% |  |
| SLOP005 | 0.20% |  |
| SLOP007 | 7.80% |  |
| SLOP008 | 0.00% |  |
| SLOP009 | 0.00% |  |
| SLOP037 | 0.00% |  |
| SLOP038 | 0.60% |  |
| SLOP039 | 0.00% |  |
| SLOP040 | 0.00% |  |
| SLOP042 | 1.40% |  |
| SLOP043 | 0.00% |  |
| **any Tier A rule** | 7.80% |  |
| **any rule** | 9.40% |  |

#### prose

| rule | cpython-doc H% | cpython-doc A% | diplomatrix H% | diplomatrix A% | essay-br H% | essay-br A% | ghostbuster H% | ghostbuster A% | hc3 H% | hc3 A% | mage H% | mage A% | rust-book H% | rust-book A% | wetbench-pt H% | wetbench-pt A% | wikipedia-pt H% | wikipedia-pt A% |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| SLOP011 | 2.45% |  | 0.00% | 0.00% | 0.00% |  | 0.20% | 0.00% | 0.60% | 1.20% | 0.00% | 0.00% | 0.95% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP012 | 0.00% |  | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP013 | 0.41% |  | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% | 0.20% | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP014 | 0.00% |  | 0.00% | 1.28% | 0.00% |  | 1.00% | 3.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP015 | 1.02% |  | 0.00% | 2.31% | 0.20% |  | 0.00% | 0.60% | 0.40% | 0.40% | 0.60% | 0.40% | 0.95% |  | 0.00% | 0.00% | 0.20% |  |
| SLOP016 | 0.00% |  | 0.00% | 0.51% | 0.00% |  | 0.00% | 6.80% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP017 | 11.45% |  | 7.95% | 22.05% | 1.20% |  | 4.60% | 8.20% | 0.40% | 3.80% | 0.20% | 1.40% | 8.57% |  | 0.00% | 0.60% | 1.00% |  |
| SLOP018 | 21.27% |  | 14.77% | 2.05% | 4.60% |  | 12.40% | 12.40% | 6.40% | 0.20% | 0.00% | 0.00% | 21.90% |  | 2.00% | 0.20% | 15.80% |  |
| SLOP019 | 0.00% |  | 0.00% | 0.77% | 0.00% |  | 0.20% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP020 | 0.61% |  | 65.91% | 2.05% | 23.20% |  | 9.80% | 4.20% | 1.40% | 0.00% | 0.00% | 0.00% | 98.10% |  | 1.00% | 0.00% | 4.20% |  |
| SLOP021 | 0.00% |  | 0.00% | 1.54% | 0.00% |  | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 56.19% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP022 | 0.82% |  | 2.27% | 7.44% | 2.20% |  | 1.20% | 0.40% | 0.00% | 0.20% | 3.40% | 4.80% | 0.95% |  | 0.00% | 0.00% | 0.40% |  |
| SLOP023 | 0.00% |  | 0.00% | 0.00% | 0.00% |  | 0.40% | 0.00% | 0.00% | 0.00% | 0.20% | 0.20% | 0.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP024 | 4.70% |  | 0.00% | 11.54% | 0.20% |  | 0.80% | 2.40% | 1.60% | 0.80% | 0.60% | 1.80% | 1.90% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP025 | 0.61% |  | 0.00% | 1.03% | 1.60% |  | 0.60% | 1.60% | 0.20% | 0.60% | 0.80% | 1.80% | 0.00% |  | 0.20% | 0.20% | 0.40% |  |
| SLOP026 | 0.00% |  | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP027 | 6.75% |  | 0.00% | 0.51% | 0.00% |  | 0.60% | 0.00% | 0.00% | 1.00% | 0.00% | 0.80% | 2.86% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP028 | 2.04% |  | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.40% | 0.00% | 0.40% | 0.20% | 0.20% | 1.90% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP029 | 0.00% |  | 0.00% | 0.26% | 0.40% |  | 0.00% | 0.00% | 0.20% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP030 | 46.83% |  | 29.55% | 1.28% | 1.20% |  | 18.60% | 12.20% | 10.20% | 9.00% | 22.60% | 30.60% | 15.24% |  | 1.00% | 0.80% | 25.60% |  |
| SLOP031 | 0.00% |  | 0.00% | 0.26% | 0.00% |  | 0.20% | 0.80% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP032 | 1.23% |  | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.20% | 0.20% | 0.40% | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP033 | 26.79% |  | 38.64% | 6.41% | 46.60% |  | 70.60% | 68.60% | 14.20% | 21.80% | 10.20% | 31.40% | 18.10% |  | 9.20% | 2.40% | 22.20% |  |
| SLOP034 | 31.49% |  | 0.00% | 0.00% | 0.00% |  | 0.60% | 0.20% | 0.40% | 0.00% | 0.00% | 1.20% | 20.00% |  | 0.00% | 0.00% | 0.00% |  |
| SLOP035 | 0.00% |  | 0.00% | 0.77% | 0.00% |  | 0.00% | 0.40% | 0.00% | 0.00% | 0.00% | 0.00% | 0.00% |  | 0.00% | 0.20% | 0.00% |  |
| SLOP036 | 13.91% |  | 0.00% | 0.00% | 0.00% |  | 0.80% | 0.00% | 0.40% | 0.40% | 1.20% | 1.60% | 0.95% |  | 0.00% | 0.20% | 0.00% |  |
| SLOP041 | 47.44% |  | 25.00% | 71.79% | 1.20% |  | 2.80% | 9.80% | 3.80% | 19.20% | 0.40% | 14.00% | 61.90% |  | 0.20% | 0.20% | 3.60% |  |
| **any Tier A rule** | 2.66% |  | 0.00% | 0.00% | 0.00% |  | 0.20% | 0.00% | 0.60% | 1.20% | 0.00% | 0.00% | 0.95% |  | 0.00% | 0.00% | 0.00% |  |
| **any rule** | 79.14% |  | 92.05% | 87.95% | 63.40% |  | 94.00% | 89.80% | 30.00% | 42.80% | 32.80% | 59.20% | 98.10% |  | 12.20% | 4.60% | 52.60% |  |

## Per-dataset detail

### aigcodeset

- Source: https://huggingface.co/datasets/basakdemirok/AIGCodeSet
- Paper: arXiv 2412.16594
- License: CDLA-Permissive-2.0
- Natural language: n/a (code)
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


### antd-tsx

- Source: https://github.com/ant-design/ant-design
- License: MIT
- Natural language: n/a (code)
- Revision: 4.17.4
- Files fetched: 500
  - tsx: human 500 (0 blank dropped)

#### antd-tsx / tsx

| rule | human hits |
|---|---|
| SLOP007 | 39 (7.80%) |
| SLOP042 | 7 (1.40%) |
| SLOP038 | 3 (0.60%) |
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
| **any Tier A rule** | 39 (7.80%) |
| **any rule** | 47 (9.40%) |


### codemirage

- Source: https://huggingface.co/datasets/HanxiGuo/CodeMirage
- License: CC-BY-NC-ND-4.0
- Natural language: n/a (code)
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


### cpython-doc

- Source: https://github.com/python/cpython
- License: PSF-2.0
- Natural language: en
- Revision: v3.10.0
- Files fetched: 489
  - prose: human 489 (0 blank dropped)
- Shares its clone with cpython-lib (same tag). Written as `.rst`: the boldface/heading-style rules (SLOP019/SLOP021) never fire on `.rst`, only on Md/Mdx/Html, so those two rows read 0 here by construction.

#### cpython-doc / prose

| rule | human hits |
|---|---|
| SLOP041 | 232 (47.44%) |
| SLOP030 | 229 (46.83%) |
| SLOP034 | 154 (31.49%) |
| SLOP033 | 131 (26.79%) |
| SLOP018 | 104 (21.27%) |
| SLOP036 | 68 (13.91%) |
| SLOP017 | 56 (11.45%) |
| SLOP027 | 33 (6.75%) |
| SLOP024 | 23 (4.70%) |
| SLOP011 | 12 (2.45%) |
| SLOP028 | 10 (2.04%) |
| SLOP032 | 6 (1.23%) |
| SLOP015 | 5 (1.02%) |
| SLOP022 | 4 (0.82%) |
| SLOP020 | 3 (0.61%) |
| SLOP025 | 3 (0.61%) |
| SLOP013 | 2 (0.41%) |
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
| **any Tier A rule** | 13 (2.66%) |
| **any rule** | 387 (79.14%) |


### cpython-lib

- Source: https://github.com/python/cpython
- License: PSF-2.0
- Natural language: n/a (code)
- Revision: v3.10.0
- Files fetched: 499
  - python: human 499 (1 blank dropped)
- Shares its clone with cpython-doc (same tag).

#### cpython-lib / python

| rule | human hits |
|---|---|
| SLOP043 | 148 (29.66%) |
| SLOP006 | 147 (29.46%) |
| SLOP008 | 47 (9.42%) |
| SLOP042 | 35 (7.01%) |
| SLOP037 | 16 (3.21%) |
| SLOP039 | 5 (1.00%) |
| SLOP040 | 4 (0.80%) |
| SLOP009 | 3 (0.60%) |
| SLOP002 | 1 (0.20%) |
| SLOP001 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| **any Tier A rule** | 167 (33.47%) |
| **any rule** | 237 (47.49%) |


### diplomatrix

- Source: https://huggingface.co/datasets/melll-uff/diplomatrixbr-gen/resolve/main/Diplomatrixbr.json
- License: MIT
- Natural language: pt
- Revision: n/a
- Files fetched: 478
  - prose: ai 390 (0 blank dropped), human 88 (0 blank dropped)
    - ai generators: gpt4o_temp03 10, gpt4o_temp05 10, gpt4o_temp07 10, command_r_plus_08_2024_temp03 10, command_r_plus_08_2024_temp05 10, command_r_plus_08_2024_temp07 10, gemma_27b_temp03 10, gemma_27b_temp05 10

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


### droid

- Source: https://huggingface.co/datasets/project-droid/DroidCollection
- Paper: arXiv 2507.10583
- License: not stated on the dataset card
- Natural language: n/a (code)
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
- Revision: n/a
- Files fetched: 500
  - prose: human 500 (0 blank dropped)
- Human-only: student essays for a national exam prompt.

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


### ghostbuster

- Source: https://github.com/vivek3141/ghostbuster-data
- Paper: arXiv 2305.15047
- License: CC-BY-3.0
- Natural language: en
- Revision: 86ebd7259055
- Files fetched: 1000
  - prose: ai 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: gpt 286, claude 214
- gpt_prompt*/gpt_semantic/gpt_writing variants are skipped; only the plain gpt/ and claude/ generations are counted as `ai`.

#### ghostbuster / prose

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /1k words | ai /1k words |
|---|---|---|---|---|---|---|---|
| SLOP033 | 353 (70.60%) | 343 (68.60%) | 0.49 | 0.49 | 0.97 | 5.62 | 6.20 |
| SLOP018 | 62 (12.40%) | 62 (12.40%) | 0.50 | 0.50 | 1.00 | 0.25 | 0.24 |
| SLOP030 | 93 (18.60%) | 61 (12.20%) | 0.40 | 0.40 | 0.66 | 0.18 | 0.13 |
| SLOP041 | 14 (2.80%) | 49 (9.80%) | 0.78 | 0.78 | 3.50 | 0.03 | 0.10 |
| SLOP017 | 23 (4.60%) | 41 (8.20%) | 0.64 | 0.64 | 1.78 | 0.04 | 0.08 |
| SLOP016 | 0 (0.00%) | 34 (6.80%) | 1.00 | 1.00 | inf | 0.00 | 0.07 |
| SLOP020 | 49 (9.80%) | 21 (4.20%) | 0.30 | 0.30 | 0.43 | 0.09 | 0.04 |
| SLOP014 | 5 (1.00%) | 15 (3.00%) | 0.75 | 0.75 | 3.00 | 0.01 | 0.03 |
| SLOP024 | 4 (0.80%) | 12 (2.40%) | 0.75* | 0.75* | 3.00 | 0.01 | 0.03 |
| SLOP025 | 3 (0.60%) | 8 (1.60%) | 0.73* | 0.73* | 2.67 | 0.01 | 0.02 |
| SLOP031 | 1 (0.20%) | 4 (0.80%) | 0.80* | 0.80* | 4.00 | 0.00 | 0.01 |
| SLOP015 | 0 (0.00%) | 3 (0.60%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP022 | 6 (1.20%) | 2 (0.40%) | 0.25* | 0.25* | 0.33 | 0.01 | 0.00 |
| SLOP028 | 0 (0.00%) | 2 (0.40%) | 1.00* | 1.00* | inf | 0.00 | 0.00 |
| SLOP035 | 0 (0.00%) | 2 (0.40%) | 1.00* | 1.00* | inf | 0.00 | 0.00 |
| SLOP032 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.00 |
| SLOP034 | 3 (0.60%) | 1 (0.20%) | 0.25* | 0.25* | 0.33 | 0.01 | 0.00 |
| SLOP011 | 1 (0.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.00 | 0.00 |
| SLOP012 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP013 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP019 | 1 (0.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.00 | 0.00 |
| SLOP021 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP023 | 2 (0.40%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.00 | 0.00 |
| SLOP026 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP027 | 3 (0.60%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.01 | 0.00 |
| SLOP029 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP036 | 4 (0.80%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.01 | 0.00 |
| **any Tier A rule** | 1 (0.20%) | 0 (0.00%) | 0.00 | 0.00 | 0.00 |  |  |
| **any rule** | 470 (94.00%) | 449 (89.80%) | 0.49 | 0.49 | 0.96 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | claude | gpt |
| --- | --- | --- |
| SLOP016 | 0.5% | 11.5% |
| SLOP017 | 7.5% | 8.7% |
| SLOP018 | 16.4% | 9.4% |
| SLOP020 | 6.5% | 2.4% |
| SLOP030 | 7.0% | 16.1% |
| SLOP033 | 68.7% | 68.5% |
| SLOP041 | 7.5% | 11.5% |


### go-std

- Source: https://github.com/golang/go
- License: BSD-3-Clause
- Natural language: n/a (code)
- Revision: go1.17.3
- Files fetched: 500
  - go: human 500 (0 blank dropped)
- go1.17.3 predates Copilot's June 2021 public preview cutoff era only loosely; treat every git-tag corpus's false-positive rate as an upper bound on how AI-assisted the tag's code could be, not a guarantee of zero.

#### go-std / go

| rule | human hits |
|---|---|
| SLOP043 | 92 (18.40%) |
| SLOP008 | 18 (3.60%) |
| SLOP042 | 15 (3.00%) |
| SLOP039 | 12 (2.40%) |
| SLOP009 | 3 (0.60%) |
| SLOP037 | 2 (0.40%) |
| SLOP001 | 1 (0.20%) |
| SLOP002 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP005 | 0 (0.00%) |
| **any Tier A rule** | 22 (4.40%) |
| **any rule** | 123 (24.60%) |


### hc3

- Source: https://huggingface.co/datasets/Hello-SimpleAI/HC3
- Paper: arXiv 2301.07597
- License: CC-BY-SA-4.0
- Natural language: en
- Revision: 4d0ff18143b5
- Files fetched: 1000
  - prose: ai 500 (0 blank dropped), human 500 (0 blank dropped)
- `domain` is the HC3 config (wiki_csai, open_qa, reddit_eli5, finance, medicine); every ai file is ChatGPT, so the per-generator table has exactly one column here.

#### hc3 / prose

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /1k words | ai /1k words |
|---|---|---|---|---|---|---|---|
| SLOP033 | 71 (14.20%) | 109 (21.80%) | 0.61 | 0.61 | 1.54 | 1.38 | 1.77 |
| SLOP041 | 19 (3.80%) | 96 (19.20%) | 0.83 | 0.83 | 5.05 | 0.28 | 1.19 |
| SLOP030 | 51 (10.20%) | 45 (9.00%) | 0.47 | 0.47 | 0.88 | 0.75 | 0.56 |
| SLOP017 | 2 (0.40%) | 19 (3.80%) | 0.90 | 0.90 | 9.50 | 0.03 | 0.24 |
| SLOP011 | 3 (0.60%) | 6 (1.20%) | 0.67* | 0.67* | 2.00 | 0.04 | 0.07 |
| SLOP027 | 0 (0.00%) | 5 (1.00%) | 1.00* | 1.00* | inf | 0.00 | 0.06 |
| SLOP024 | 8 (1.60%) | 4 (0.80%) | 0.33* | 0.33* | 0.50 | 0.12 | 0.05 |
| SLOP025 | 1 (0.20%) | 3 (0.60%) | 0.75* | 0.75* | 3.00 | 0.01 | 0.04 |
| SLOP015 | 2 (0.40%) | 2 (0.40%) | 0.50* | 0.50* | 1.00 | 0.03 | 0.02 |
| SLOP028 | 0 (0.00%) | 2 (0.40%) | 1.00* | 1.00* | inf | 0.00 | 0.02 |
| SLOP032 | 1 (0.20%) | 2 (0.40%) | 0.67* | 0.67* | 2.00 | 0.01 | 0.02 |
| SLOP036 | 2 (0.40%) | 2 (0.40%) | 0.50* | 0.50* | 1.00 | 0.03 | 0.05 |
| SLOP013 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP018 | 32 (6.40%) | 1 (0.20%) | 0.03 | 0.03 | 0.03 | 0.88 | 0.01 |
| SLOP022 | 0 (0.00%) | 1 (0.20%) | 1.00* | 1.00* | inf | 0.00 | 0.01 |
| SLOP012 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP014 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP016 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP019 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP020 | 7 (1.40%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.10 | 0.00 |
| SLOP021 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP023 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP026 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP029 | 1 (0.20%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.01 | 0.00 |
| SLOP031 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| SLOP034 | 2 (0.40%) | 0 (0.00%) | 0.00* | 0.00* | 0.00 | 0.03 | 0.00 |
| SLOP035 | 0 (0.00%) | 0 (0.00%) | -- | -- | -- | 0.00 | 0.00 |
| **any Tier A rule** | 3 (0.60%) | 6 (1.20%) | 0.67 | 0.67 | 2.00 |  |  |
| **any rule** | 150 (30.00%) | 214 (42.80%) | 0.59 | 0.59 | 1.43 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | chatgpt |
| --- | --- |
| SLOP030 | 9.0% |
| SLOP033 | 21.8% |
| SLOP041 | 19.2% |


### mage

- Source: https://huggingface.co/datasets/yaful/MAGE
- Paper: arXiv 2305.13242
- License: Apache-2.0 on the card, CC-BY-4.0 in the repo
- Natural language: en
- Revision: 342663f0a2b7
- Files fetched: 1000
  - prose: ai 500 (0 blank dropped), human 500 (0 blank dropped)
    - ai generators: 13B 100, gpt_j 100, opt_350m 100, opt_2.7b 100, text-davinci-002 53, flan_t5_base 47
- MAGE's own license terms conflict across its source domains (it re-publishes several licensed corpora); treat this dataset as measurement-only, nothing republished beyond aggregate counts.

#### mage / prose

| rule | human hits | ai hits | precision @1:1 | precision @prior | lift | human /1k words | ai /1k words |
|---|---|---|---|---|---|---|---|
| SLOP033 | 51 (10.20%) | 157 (31.40%) | 0.75 | 0.75 | 3.08 | 0.94 | 1.75 |
| SLOP030 | 113 (22.60%) | 153 (30.60%) | 0.58 | 0.58 | 1.35 | 1.54 | 0.99 |
| SLOP041 | 2 (0.40%) | 70 (14.00%) | 0.97 | 0.97 | 35.00 | 0.03 | 0.45 |
| SLOP022 | 17 (3.40%) | 24 (4.80%) | 0.59 | 0.59 | 1.41 | 0.23 | 0.16 |
| SLOP024 | 3 (0.60%) | 9 (1.80%) | 0.75* | 0.75* | 3.00 | 0.04 | 0.06 |
| SLOP025 | 4 (0.80%) | 9 (1.80%) | 0.69* | 0.69* | 2.25 | 0.05 | 0.06 |
| SLOP036 | 6 (1.20%) | 8 (1.60%) | 0.57* | 0.57* | 1.33 | 0.08 | 0.05 |
| SLOP017 | 1 (0.20%) | 7 (1.40%) | 0.88* | 0.88* | 7.00 | 0.01 | 0.05 |
| SLOP034 | 0 (0.00%) | 6 (1.20%) | 1.00* | 1.00* | inf | 0.00 | 0.04 |
| SLOP027 | 0 (0.00%) | 4 (0.80%) | 1.00* | 1.00* | inf | 0.00 | 0.03 |
| SLOP015 | 3 (0.60%) | 2 (0.40%) | 0.40* | 0.40* | 0.67 | 0.04 | 0.01 |
| SLOP023 | 1 (0.20%) | 1 (0.20%) | 0.50* | 0.50* | 1.00 | 0.01 | 0.01 |
| SLOP028 | 1 (0.20%) | 1 (0.20%) | 0.50* | 0.50* | 1.00 | 0.01 | 0.01 |
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
| **any rule** | 164 (32.80%) | 296 (59.20%) | 0.64 | 0.64 | 1.80 |  |  |

Per-generator (ai, >= 20 files, rule >= 20 hits):

| rule | 13B | flan_t5_base | gpt_j | opt_2.7b | opt_350m | text-davinci-002 |
| --- | --- | --- | --- | --- | --- | --- |
| SLOP022 | 12.0% | 6.4% | 4.0% | 0.0% | 3.0% | 3.8% |
| SLOP030 | 78.0% | 8.5% | 13.0% | 5.0% | 13.0% | 75.5% |
| SLOP033 | 13.0% | 0.0% | 35.0% | 63.0% | 43.0% | 5.7% |
| SLOP041 | 67.0% | 0.0% | 0.0% | 0.0% | 0.0% | 5.7% |


### rosetta

- Source: https://huggingface.co/datasets/christopher/rosetta-code
- License: GFDL
- Natural language: n/a (code)
- Revision: 11d8b38cbd90
- Files fetched: 1539
  - go: human 500 (0 blank dropped)
  - python: human 500 (0 blank dropped)
  - rust: human 500 (0 blank dropped)
  - typescript: human 39 (0 blank dropped)
- Human-only: every task/language pair is a solution someone wrote for the Rosetta Code wiki, so this dataset measures the false-positive rate alone.

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


### rust-book

- Source: https://github.com/rust-lang/book
- License: MIT OR Apache-2.0
- Natural language: en
- Revision: 15aef431579a
- Files fetched: 105
  - prose: human 105 (0 blank dropped)
- No 2021 tag exists on this repo; cloned shallow-since 2020 and checked out the last commit before 2022-01-01 on the default branch instead.

#### rust-book / prose

| rule | human hits |
|---|---|
| SLOP020 | 103 (98.10%) |
| SLOP041 | 65 (61.90%) |
| SLOP021 | 59 (56.19%) |
| SLOP018 | 23 (21.90%) |
| SLOP034 | 21 (20.00%) |
| SLOP033 | 19 (18.10%) |
| SLOP030 | 16 (15.24%) |
| SLOP017 | 9 (8.57%) |
| SLOP027 | 3 (2.86%) |
| SLOP024 | 2 (1.90%) |
| SLOP028 | 2 (1.90%) |
| SLOP011 | 1 (0.95%) |
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
| **any Tier A rule** | 1 (0.95%) |
| **any rule** | 103 (98.10%) |


### rust-std

- Source: https://github.com/rust-lang/rust
- License: MIT OR Apache-2.0
- Natural language: n/a (code)
- Revision: 1.57.0
- Files fetched: 500
  - rust: human 500 (0 blank dropped)

#### rust-std / rust

| rule | human hits |
|---|---|
| SLOP043 | 127 (25.40%) |
| SLOP039 | 18 (3.60%) |
| SLOP005 | 9 (1.80%) |
| SLOP042 | 6 (1.20%) |
| SLOP009 | 1 (0.20%) |
| SLOP001 | 0 (0.00%) |
| SLOP002 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP008 | 0 (0.00%) |
| SLOP037 | 0 (0.00%) |
| **any Tier A rule** | 10 (2.00%) |
| **any rule** | 147 (29.40%) |


### typescript-src

- Source: https://github.com/microsoft/TypeScript
- License: Apache-2.0
- Natural language: n/a (code)
- Revision: v4.5.4
- Files fetched: 282
  - typescript: human 282 (2 blank dropped)

#### typescript-src / typescript

| rule | human hits |
|---|---|
| SLOP043 | 89 (31.56%) |
| SLOP042 | 35 (12.41%) |
| SLOP007 | 34 (12.06%) |
| SLOP039 | 11 (3.90%) |
| SLOP040 | 5 (1.77%) |
| SLOP005 | 4 (1.42%) |
| SLOP008 | 1 (0.35%) |
| SLOP037 | 1 (0.35%) |
| SLOP001 | 0 (0.00%) |
| SLOP002 | 0 (0.00%) |
| SLOP003 | 0 (0.00%) |
| SLOP004 | 0 (0.00%) |
| SLOP009 | 0 (0.00%) |
| SLOP038 | 0 (0.00%) |
| **any Tier A rule** | 35 (12.41%) |
| **any rule** | 106 (37.59%) |


### wetbench-pt

- Source: https://huggingface.co/datasets/cs928346/WETBench
- License: CC-BY-NC-SA-4.0
- Natural language: pt
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


### wikipedia-pt

- Source: https://huggingface.co/datasets/TucanoBR/wikipedia-PT
- License: CC-BY-SA-3.0
- Natural language: pt
- Revision: 81aeee176c2c
- Files fetched: 500
  - prose: human 500 (0 blank dropped)
- Human-only.

#### wikipedia-pt / prose

| rule | human hits |
|---|---|
| SLOP030 | 128 (25.60%) |
| SLOP033 | 111 (22.20%) |
| SLOP018 | 79 (15.80%) |
| SLOP020 | 21 (4.20%) |
| SLOP041 | 18 (3.60%) |
| SLOP017 | 5 (1.00%) |
| SLOP022 | 2 (0.40%) |
| SLOP025 | 2 (0.40%) |
| SLOP015 | 1 (0.20%) |
| SLOP011 | 0 (0.00%) |
| SLOP012 | 0 (0.00%) |
| SLOP013 | 0 (0.00%) |
| SLOP014 | 0 (0.00%) |
| SLOP016 | 0 (0.00%) |
| SLOP019 | 0 (0.00%) |
| SLOP021 | 0 (0.00%) |
| SLOP023 | 0 (0.00%) |
| SLOP024 | 0 (0.00%) |
| SLOP026 | 0 (0.00%) |
| SLOP027 | 0 (0.00%) |
| SLOP028 | 0 (0.00%) |
| SLOP029 | 0 (0.00%) |
| SLOP031 | 0 (0.00%) |
| SLOP032 | 0 (0.00%) |
| SLOP034 | 0 (0.00%) |
| SLOP035 | 0 (0.00%) |
| SLOP036 | 0 (0.00%) |
| **any Tier A rule** | 0 (0.00%) |
| **any rule** | 263 (52.60%) |
