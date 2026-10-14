"""The dataset registry: `DATASETS` names, for each entry, a `kind` (the calling convention
`orchestrate.fetch_dataset` dispatches on) and a `fetch` callable (the implementation -- a
function from `fetchers.py` or `git.py`), plus its natural language, source, license, caveats
and the `cells` it contributes. `SYNTH_CELLS` and `discover_synth_datasets` fold in whatever
`bench/generate_corpus.py` has written under `<dir>/synth/`.

Rejected datasets, and why, so nobody re-proposes them:
  - HumanVsAICode: function-level Python, redundant with AIGCodeSet/Droid/CodeMirage.
  - CSC15011: no dataset card, no stated license.
  - AICD-Bench: rows carry only `code` and `label`, no language column to split cells by.
  - MultiAIGCD: the archive it ships from is unpublished.
  - HybridCodeAuthorship: no public download under that name.
  - Whodunit: labels live in filenames, not a queryable column.
  - RAID: 21 GB, and its `/filter` endpoint errored on every query tried this session; its
    attack-robustness column is worth a phase-2 pass once `/filter` is stable.
  - DetectRL: no stated license.
  - OUTFOX: ships as pickles, not a stdlib-readable format.
  - MGTBench, MGTBench 2.0: Google Drive / parquet-only with the viewer disabled.
  - MultiSocial: Zenodo, request-access only.
  - LLM-DetectAIve: no published data.
  - COLING multilingual GT detection: verified to carry no Portuguese config.
  - IberAuTexTification, MULTITuDE: gated behind a request-access form.
  - Carolina: distributed via a download script, not a stable direct URL.
  - python-docs-pt-br: `.po` gettext catalogs need a real parser this crate does not carry;
    phase 2.
  - Wikipedia-PT (TucanoBR, 2023 dump) and the legacy `wikipedia` 20220301.pt snapshot: both
    postdate the human bar, and no pre-2020 Portuguese snapshot is reachable without parsing
    a full XML dump.

CoDET-M4 ships both `code` and a `cleaned_code` companion column with comments and docstrings
stripped; not every `code` row keeps them intact. SLOP001-004, SLOP042 and SLOP043 read
comments/docstrings almost exclusively, so the report prints `n/a` for those six rules on
this dataset instead of a number that would measure the extraction pass, not the code.
"""
import os

from .common import JS_PROXY
from .fetchers import (
    fetch_aidev, fetch_aigcodeset, fetch_apt_eval, fetch_beemo, fetch_diplomatrix,
    fetch_essay_br, fetch_faidset, fetch_ghostbuster, fetch_gpt2_output, fetch_hc3,
    fetch_hf_filter_cell, fetch_hf_text_rows, fetch_mage, fetch_naples_code, fetch_pan25, fetch_wetbench_pt,
)
from .git import fetch_git_before_cell, fetch_git_cell, fetch_git_pinned_set

UI_EXCLUDE = ("components/ui/", ".min.", "node_modules/", "/dist/", "/build/", ".next/",
              "/__tests__/", ".test.", ".spec.", ".stories.")

AIGCODESET_BASE = "https://huggingface.co/datasets/basakdemirok/AIGCodeSet/resolve/main/data"

DATASETS = {
    "aigcodeset": {
        "kind": "url_csv", "natlang": None, "dataset": "basakdemirok/AIGCodeSet",
        "url": "https://huggingface.co/datasets/basakdemirok/AIGCodeSet",
        "paper": "arXiv 2412.16594", "license": "CDLA-Permissive-2.0",
        "generator_years": "CodeLlama-34B, Codestral-22B, Gemini 1.5 Flash (2024)",
        "human_provenance": "unverified",
        "human_note": "IBM CodeNet submissions, collected through 2020 and published 2021; AtCoder "
                      "and AIZU users, unverified for assistant use",
        "caveats": [
            "The machine side is post-extraction code: the dataset authors kept the code "
            "block and dropped the surrounding chat answer, so SLOP001-SLOP004 are measured "
            "at their hardest case here, a floor rather than a typical number.",
            "Both sides are competitive-programming solutions: short, single-file, few "
            "abstractions. SLOP037, SLOP039 and SLOP040 have little to bite, and this "
            "false-positive rate does not transfer to application code.",
            "AtCoder is Japanese and many human files carry Japanese comments, which the "
            "English-lexicon rules cannot match.",
            "The machine side is entirely code that failed; the human side is one third "
            "accepted: the dataset authors matched outcome buckets deliberately.",
        ],
        "fetch": fetch_aigcodeset,
        "cells": [
            {"label": "human", "lang": "python", "url": f"{AIGCODESET_BASE}/human_selected_dataset.csv",
             "csv_label": "0", "generator": "human"},
            {"label": "ai", "lang": "python", "url": f"{AIGCODESET_BASE}/created_dataset_with_llms.csv",
             "csv_label": "1"},
        ],
    },
    "droid": {
        "kind": "hf_filter", "natlang": None,
        "dataset": "project-droid/DroidCollection", "config": "default", "split": "train",
        "text_col": "Code", "generator_col": "Generator",
        "url": "https://huggingface.co/datasets/project-droid/DroidCollection",
        "paper": "arXiv 2507.10583", "license": None,
        "generator_years": "open and API code models, 2024-25 (Generator column)",
        "human_provenance": "unverified",
        "human_note": "GitHub, LeetCode and Codeforces code collected 2024-25; the paper says the "
                      "human class may contain assistant-written code",
        "caveats": [
            "MACHINE_REFINED exists for Python only on this dataset; the adversarial split "
            "exists for Python and Rust only.",
            "DroidCollection ships no plain machine-generated Rust; the adversarial label is "
            "the only AI Rust it has.",
        ],
        "fetch": fetch_hf_filter_cell,
        "cells": [
            {"label": "human", "lang": "python", "generator": "human",
             "where": '"Language"=\'Python\' and "Label"=\'HUMAN_GENERATED\''},
            {"label": "ai", "lang": "python",
             "where": '"Language"=\'Python\' and "Label"=\'MACHINE_GENERATED\''},
            {"label": "ai-refined", "lang": "python",
             "where": '"Language"=\'Python\' and "Label"=\'MACHINE_REFINED\''},
            {"label": "ai-adversarial", "lang": "python",
             "where": '"Language"=\'Python\' and "Label"=\'MACHINE_GENERATED_ADVERSARIAL\''},
            {"label": "human", "lang": "go", "generator": "human",
             "where": '"Language"=\'Go\' and "Label"=\'HUMAN_GENERATED\''},
            {"label": "ai", "lang": "go",
             "where": '"Language"=\'Go\' and "Label"=\'MACHINE_GENERATED\''},
            {"label": "human", "lang": "rust", "generator": "human",
             "where": '"Language"=\'Rust\' and "Label"=\'HUMAN_GENERATED\''},
            {"label": "ai-adversarial", "lang": "rust",
             "where": '"Language"=\'Rust\' and "Label"=\'MACHINE_GENERATED_ADVERSARIAL\''},
            {"label": "human", "lang": "typescript", "generator": "human", "proxy": JS_PROXY,
             "where": '"Language"=\'JavaScript\' and "Label"=\'HUMAN_GENERATED\''},
            {"label": "ai", "lang": "typescript", "proxy": JS_PROXY,
             "where": '"Language"=\'JavaScript\' and "Label"=\'MACHINE_GENERATED\''},
        ],
    },
    "semeval13": {
        "kind": "hf_filter", "natlang": None,
        "dataset": "DaniilOr/SemEval-2026-Task13", "config": "C", "split": "train",
        "text_col": "code", "generator_col": "generator",
        "url": "https://huggingface.co/datasets/DaniilOr/SemEval-2026-Task13",
        "paper": None, "license": "Apache-2.0",
        "generator_years": "Qwen2.5-Coder, DeepSeek-Coder, Llama 3.x, GPT-4o and other 2024-25 "
                           "models (generator column)",
        "human_provenance": "unverified",
        "human_note": "Droid-derived GitHub, LeetCode and Codeforces code collected 2024-25",
        "caveats": [
            "Config C is the four-way subtask: label 0 human, 1 machine, 2 hybrid (human and "
            "model in one file), 3 adversarially humanized machine code. Hybrid and "
            "adversarial are robustness splits and are never pooled with plain ai.",
            "Subtask C ships no Rust and no TypeScript; its JavaScript is written as .ts like "
            "the other JavaScript cells.",
        ],
        "fetch": fetch_hf_filter_cell,
        "cells": [
            {"label": "human", "lang": "python", "generator": "human",
             "where": '"language"=\'Python\' and "label"=0'},
            {"label": "ai", "lang": "python", "where": '"language"=\'Python\' and "label"=1'},
            {"label": "ai-hybrid", "lang": "python", "where": '"language"=\'Python\' and "label"=2'},
            {"label": "ai-adversarial", "lang": "python", "where": '"language"=\'Python\' and "label"=3'},
            {"label": "human", "lang": "go", "generator": "human", "where": '"language"=\'Go\' and "label"=0'},
            {"label": "ai", "lang": "go", "where": '"language"=\'Go\' and "label"=1'},
            {"label": "human", "lang": "typescript", "generator": "human", "proxy": JS_PROXY,
             "where": '"language"=\'JavaScript\' and "label"=0'},
            {"label": "ai", "lang": "typescript", "proxy": JS_PROXY,
             "where": '"language"=\'JavaScript\' and "label"=1'},
        ],
    },
    "codemirage": {
        "kind": "hf_filter", "natlang": None,
        "dataset": "HanxiGuo/CodeMirage", "config": "default", "split": "train",
        "text_col": "code", "generator_col": "source",
        "url": "https://huggingface.co/datasets/HanxiGuo/CodeMirage",
        "paper": "arXiv 2506.11059", "license": "CC-BY-NC-ND-4.0",
        "generator_years": "ten LLMs, 2025 (source column)",
        "human_provenance": "unverified",
        "human_note": "CodeParrot github-code-clean, a GitHub snapshot from May 2022",
        "caveats": ["CC-BY-NC-ND-4.0 forbids redistributing a derivative; measurement only, "
                    "nothing from this dataset is republished here beyond aggregate counts.",
                    "Every AI file is regenerated to match its paired human file's line count "
                    "and size and filtered to BLEU < 0.5 against it, so length-based tells "
                    "are suppressed by construction."],
        "fetch": fetch_hf_filter_cell,
        "cells": [
            {"label": "human", "lang": "python", "generator": "human",
             "where": '"language"=\'Python\' and "source"=\'Human\''},
            {"label": "ai", "lang": "python",
             "where": '"language"=\'Python\' and "source"<>\'Human\' and "variant"=\'Normal\''},
            {"label": "ai-paraphrased", "lang": "python",
             "where": '"language"=\'Python\' and "source"<>\'Human\' and "variant"=\'Paraphrased\''},
            {"label": "human", "lang": "go", "generator": "human",
             "where": '"language"=\'Go\' and "source"=\'Human\''},
            {"label": "ai", "lang": "go",
             "where": '"language"=\'Go\' and "source"<>\'Human\' and "variant"=\'Normal\''},
            {"label": "ai-paraphrased", "lang": "go",
             "where": '"language"=\'Go\' and "source"<>\'Human\' and "variant"=\'Paraphrased\''},
            {"label": "human", "lang": "typescript", "generator": "human", "proxy": JS_PROXY,
             "where": '"language"=\'JavaScript\' and "source"=\'Human\''},
            {"label": "ai", "lang": "typescript", "proxy": JS_PROXY,
             "where": '"language"=\'JavaScript\' and "source"<>\'Human\' and "variant"=\'Normal\''},
            {"label": "ai-paraphrased", "lang": "typescript", "proxy": JS_PROXY,
             "where": '"language"=\'JavaScript\' and "source"<>\'Human\' and "variant"=\'Paraphrased\''},
        ],
    },
    "codet_m4": {
        "kind": "hf_filter", "natlang": None,
        "dataset": "DaniilOr/CoDET-M4", "config": "default", "split": "train",
        "text_col": "code", "generator_col": "model",
        "url": "https://huggingface.co/datasets/DaniilOr/CoDET-M4",
        "paper": "arXiv 2503.13733", "license": "MIT",
        "generator_years": "GPT-4o, Llama 3, Qwen and other 2024 models (model column)",
        "human_provenance": "unverified",
        "human_note": "LeetCode and Codeforces solutions, undated, plus CodeSearchNet (2019)",
        "na_rules": {"SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP042", "SLOP043",
                     "SLOP045"},
        "caveats": [
            "Comments and docstrings are stripped by the publisher's extraction pass, so "
            "SLOP001-SLOP004, SLOP042 and SLOP043 print `n/a` here instead of a number that "
            "would just measure the extraction, not the code. SLOP045 too: deleting comment "
            "lines rewrites the line and block lengths it measures.",
        ],
        "fetch": fetch_hf_filter_cell,
        "cells": [
            {"label": "human", "lang": "python", "generator": "human",
             "where": '"language"=\'python\' and "model"=\'human\''},
            {"label": "ai", "lang": "python",
             "where": '"language"=\'python\' and "model"<>\'human\''},
        ],
    },
    "naples-code": {
        "kind": "url_jsonl", "natlang": "en",
        "url_file": "https://zenodo.org/records/15423067/files/python_dataset.jsonl",
        "url": "https://zenodo.org/records/15423067",
        "paper": "arXiv 2508.21634", "license": "CC-BY-4.0",
        "generator_years": "gpt-3.5-turbo (2023), DeepSeek-Coder-Instruct-33B and "
                           "Qwen2.5-Coder-Instruct-32B (2024)",
        "human_provenance": "pinned-2019",
        "human_note": "HMCorp: 16,928 non-forked GitHub repos sorted by stars, filtered from "
                      "CodeSearchNet (2019)",
        "na_rules": {"SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP042", "SLOP043",
                     "SLOP045"},
        "caveats": [
            "`docstring` ships as its own column, separate from `human_code`, so "
            "SLOP001-SLOP004, SLOP042, SLOP043 and SLOP045 print `n/a` here the same way they do on "
            "codet_m4: the extraction pass strips comments and docstrings out of the code "
            "columns, not the model.",
            "The file is 651 MB; `fetch_naples_code` streams it and stops once both samples "
            "are full, so this is a head sample of file order rather than the spread sample "
            "every HF-backed dataset above gets through `/filter`.",
            "The Java half of this Zenodo record is not registered: stopslop has no Java lang.",
        ],
        "fetch": fetch_naples_code,
        "cells": [
            {"label": "human", "lang": "python", "generator": "human"},
            {"label": "ai", "lang": "python"},
        ],
    },
    "rosetta": {
        "kind": "hf_filter", "natlang": None,
        "dataset": "christopher/rosetta-code", "config": "default", "split": "train",
        "text_col": "code", "generator_col": None,
        "url": "https://huggingface.co/datasets/christopher/rosetta-code",
        "paper": None, "license": "GFDL",
        "generator_years": None,
        "human_provenance": "unverified",
        "human_note": "Rosetta Code wiki solutions in a 2022-23 snapshot; individual edits are undated",
        "caveats": ["Human-only: every task/language pair is a solution someone wrote for the "
                    "Rosetta Code wiki, so this dataset measures the false-positive rate alone.",
                    "About nine in ten Python solutions are Python 2 and do not parse under "
                    "Python 3, so any parse-validity signal has nothing to separate here."],
        "fetch": fetch_hf_filter_cell,
        "cells": [
            {"label": "human", "lang": "python", "generator": "human", "where": '"language_name"=\'Python\''},
            {"label": "human", "lang": "go", "generator": "human", "where": '"language_name"=\'Go\''},
            {"label": "human", "lang": "rust", "generator": "human", "where": '"language_name"=\'Rust\''},
            {"label": "human", "lang": "typescript", "generator": "human",
             "where": '"language_name"=\'TypeScript\''},
        ],
    },
    "hairosetta": {
        "kind": "hf_filter", "natlang": "en",
        "dataset": "isThisYouLLM/H-AIRosettaMP", "config": "default", "split": "train",
        "text_col": "code", "generator_col": None,
        "url": "https://huggingface.co/datasets/isThisYouLLM/H-AIRosettaMP",
        "paper": "arXiv 2412.14611", "license": "MIT",
        "generator_years": "StarCoder2 (2024)",
        "human_provenance": "unverified",
        "human_note": "Rosetta Code wiki solutions, retrieved 2022-07-01, after Copilot shipped",
        "caveats": [
            "The ai side is StarCoder2 translating a human solution from another language "
            "into this one (named in the `set` column, e.g. `Rust_from_Java`), not writing "
            "from a task prompt; translated code may carry different tells than prompted "
            "code, so every ai cell here is labelled `ai-translated`, never plain `ai`.",
            "Rosetta Code is already registered on its own (`rosetta`); this is the "
            "derivative that adds the ai half, not a re-proposal.",
        ],
        "fetch": fetch_hf_filter_cell,
        "cells": [
            {"label": "human", "lang": "python", "generator": "human",
             "where": '"language_name"=\'Python\' and "target"=\'Human_written\''},
            {"label": "ai-translated", "lang": "python", "generator": "starcoder2",
             "where": '"language_name"=\'Python\' and "target"=\'Ai_generated\''},
            {"label": "human", "lang": "go", "generator": "human",
             "where": '"language_name"=\'Go\' and "target"=\'Human_written\''},
            {"label": "ai-translated", "lang": "go", "generator": "starcoder2",
             "where": '"language_name"=\'Go\' and "target"=\'Ai_generated\''},
            {"label": "human", "lang": "rust", "generator": "human",
             "where": '"language_name"=\'Rust\' and "target"=\'Human_written\''},
            {"label": "ai-translated", "lang": "rust", "generator": "starcoder2",
             "where": '"language_name"=\'Rust\' and "target"=\'Ai_generated\''},
            {"label": "human", "lang": "typescript", "generator": "human", "proxy": JS_PROXY,
             "where": '"language_name"=\'JavaScript\' and "target"=\'Human_written\''},
            {"label": "ai-translated", "lang": "typescript", "generator": "starcoder2", "proxy": JS_PROXY,
             "where": '"language_name"=\'JavaScript\' and "target"=\'Ai_generated\''},
        ],
    },
    "go-std": {
        "kind": "git_tag", "natlang": None,
        "repo": "https://github.com/golang/go", "tag": "go1.13",
        "url": "https://github.com/golang/go", "paper": None, "license": "BSD-3-Clause",
        "generator_years": None,
        "human_provenance": "pinned-2019",
        "human_note": "Go 1.13 standard library, tagged September 2019",
        "caveats": [],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "go", "generator": "human",
                   "subdir": "src", "glob": "*.go", }],
        "exclude": ("testdata/", "_test.go", "/vendor/"),
    },
    "cpython-lib": {
        "kind": "git_tag", "natlang": None,
        "repo": "https://github.com/python/cpython", "tag": "v3.8.0",
        "url": "https://github.com/python/cpython", "paper": None, "license": "PSF-2.0",
        "generator_years": None,
        "human_provenance": "pinned-2019",
        "human_note": "CPython 3.8.0 Lib, tagged October 2019",
        "caveats": ["Shares its clone with cpython-doc (same tag)."],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "python", "generator": "human",
                   "subdir": "Lib", "glob": "*.py"}],
        "exclude": ("/test/", "/tests/", "lib2to3/tests", "idlelib/idle_test"),
    },
    "rust-std": {
        "kind": "git_tag", "natlang": None,
        "repo": "https://github.com/rust-lang/rust", "tag": "1.40.0",
        "url": "https://github.com/rust-lang/rust", "paper": None, "license": "MIT OR Apache-2.0",
        "generator_years": None,
        "human_provenance": "pinned-2019",
        "human_note": "Rust 1.40.0 libstd, libcore and liballoc, tagged December 2019",
        "caveats": [],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "rust", "generator": "human",
                   "subdir": ("src/libstd", "src/libcore", "src/liballoc"), "glob": "*.rs"}],
        "exclude": ("/tests/", "/benches/"),
    },
    "typescript-src": {
        "kind": "git_tag", "natlang": None,
        "repo": "https://github.com/microsoft/TypeScript", "tag": "v3.7.2",
        "url": "https://github.com/microsoft/TypeScript", "paper": None, "license": "Apache-2.0",
        "generator_years": None,
        "human_provenance": "pinned-2019",
        "human_note": "TypeScript 3.7.2 compiler sources, tagged November 2019",
        "caveats": [],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "typescript", "generator": "human",
                   "subdir": "src", "glob": "*.ts"}],
        "exclude": ("/tests/", "testRunner", "harness"),
    },
    "antd-tsx": {
        "kind": "git_tag", "natlang": None,
        "repo": "https://github.com/ant-design/ant-design", "tag": "3.26.0",
        "url": "https://github.com/ant-design/ant-design", "paper": None, "license": "MIT",
        "generator_years": None,
        "human_provenance": "pinned-2019",
        "human_note": "Ant Design 3.26.0 components, tagged December 2019",
        "caveats": [],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "tsx", "generator": "human",
                   "subdir": "components", "glob": "*.tsx"}],
        "exclude": ("/__tests__/", "/demo/", ".test.tsx"),
    },
    "bootstrap-css": {
        "kind": "git_tag", "natlang": None,
        "repo": "https://github.com/twbs/bootstrap", "tag": "v4.4.1",
        "url": "https://github.com/twbs/bootstrap", "paper": None, "license": "MIT",
        "generator_years": None,
        "human_provenance": "pinned-2019",
        "human_note": "Bootstrap 4.4.1 stylesheets and example pages, tagged November 2019",
        "caveats": ["A framework's own CSS plus its demo pages, not a landing-page corpus; a "
                    "stylesheet can mention a violet hex as a documented example."],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "css", "generator": "human",
                   "subdir": ("dist/css", "site/docs/4.4/examples", "site/docs/4.4/assets/css"),
                   "glob": "*.css"}],
        "exclude": (".min.",),
    },
    "codemirror-css": {
        "kind": "git_tag", "natlang": None,
        "repo": "https://github.com/codemirror/CodeMirror", "tag": "5.50.0",
        "url": "https://github.com/codemirror/CodeMirror", "paper": None, "license": "MIT",
        "generator_years": None,
        "human_provenance": "pinned-2019",
        "human_note": "CodeMirror 5.50.0 editor, theme and addon stylesheets, tagged November 2019",
        "caveats": ["Editor themes: color-heavy and gradient-light, so a color rule's false "
                    "positive rate here is a worst case for hex tells."],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "css", "generator": "human",
                   "subdir": ("lib", "theme", "addon"), "glob": "*.css"}],
        "exclude": (),
    },
    "highlightjs-css": {
        "kind": "git_tag", "natlang": None,
        "repo": "https://github.com/highlightjs/highlight.js", "tag": "9.17.1",
        "url": "https://github.com/highlightjs/highlight.js", "paper": None, "license": "BSD-3-Clause",
        "generator_years": None,
        "human_provenance": "pinned-2019",
        "human_note": "highlight.js 9.17.1 syntax themes, tagged January 2020",
        "caveats": ["Syntax-highlighting themes: a palette per file, no layout CSS."],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "css", "generator": "human",
                   "subdir": "src/styles", "glob": "*.css"}],
        "exclude": (),
    },
    "design2code": {
        "kind": "hf_text_rows", "natlang": None, "dataset": "SALT-NLP/Design2Code-hf",
        "text_col": "text",
        "url": "https://huggingface.co/datasets/SALT-NLP/Design2Code-hf",
        "paper": "arXiv 2403.03163", "license": "CC-BY-4.0",
        "generator_years": None,
        "human_provenance": "unverified",
        "human_note": "484 real web pages sampled from the C4 validation split (Common Crawl, "
                      "April 2019) and cleaned by the authors; some pages carry Bootstrap 5 "
                      "custom properties (2021), so a later re-crawl cannot be ruled out",
        "caveats": ["The authors replaced images with placeholders and trimmed scripts, so "
                    "page furniture reads cleaner than the live web.",
                    "Fetched at run time and never committed to this repo."],
        "fetch": fetch_hf_text_rows,
        "cells": [{"label": "human", "lang": "html", "generator": "human"}],
    },
    "websight-v01": {
        "kind": "hf_text_rows", "natlang": None, "dataset": "HuggingFaceM4/WebSight",
        "text_col": "text", "split": "train",
        "url": "https://huggingface.co/datasets/HuggingFaceM4/WebSight",
        "paper": "arXiv 2403.09029", "license": "CC-BY-4.0",
        "generator_years": "code LLM, 2024 (v0.1: HTML with inline CSS)",
        "human_provenance": None,
        "caveats": ["Every page is synthetic: an LLM expanded a short idea into a page.",
                    "Fetched at run time and never committed to this repo."],
        "fetch": fetch_hf_text_rows,
        "cells": [{"label": "ai", "lang": "html", "config": "v0.1", "generator": "websight-v0.1"}],
    },
    "websight-v02": {
        "kind": "hf_text_rows", "natlang": None, "dataset": "HuggingFaceM4/WebSight",
        "text_col": "text", "split": "train",
        "url": "https://huggingface.co/datasets/HuggingFaceM4/WebSight",
        "paper": "arXiv 2403.09029", "license": "CC-BY-4.0",
        "generator_years": "code LLM, 2024 (v0.2: HTML with Tailwind classes)",
        "human_provenance": None,
        "caveats": ["Every page is synthetic and styled with Tailwind utility classes, so "
                    "class-list rules have far more to read here than on the v0.1 pages.",
                    "Fetched at run time and never committed to this repo."],
        "fetch": fetch_hf_text_rows,
        "cells": [{"label": "ai", "lang": "html", "config": "v0.2", "generator": "websight-v0.2"}],
    },
    "hc3": {
        "kind": "hf_rows", "natlang": "en",
        "dataset": "Hello-SimpleAI/HC3",
        "url": "https://huggingface.co/datasets/Hello-SimpleAI/HC3",
        "paper": "arXiv 2301.07597", "license": "CC-BY-SA-4.0",
        "generator_years": "ChatGPT (December 2022)",
        "human_provenance": "pinned-2019",
        "human_domains": ["reddit_eli5", "finance", "open_qa"],
        "human_note": "ELI5 (2019), FiQA (2018) and WikiQA (2015) answers; the wiki_csai (2022) and "
                      "medicine (2020) human rows are dropped, their ChatGPT rows kept",
        "caveats": ["`domain` is the HC3 config (wiki_csai, open_qa, reddit_eli5, finance, "
                    "medicine); every ai file is ChatGPT, so the per-generator table has "
                    "exactly one column here.",
                    "Answers were pasted out of the API payload: about one in seven ChatGPT "
                    "files carries a literal backslash-n paragraph break, a pipeline artifact "
                    "rather than model style."],
        "fetch": fetch_hc3,
        "cells": [
            {"label": "human", "lang": "prose"},
            {"label": "ai", "lang": "prose", "generator": "chatgpt"},
        ],
    },
    "mage": {
        "kind": "hf_rows", "natlang": "en",
        "dataset": "yaful/MAGE",
        "url": "https://huggingface.co/datasets/yaful/MAGE",
        "paper": "arXiv 2305.13242", "license": "Apache-2.0 on the card, CC-BY-4.0 in the repo",
        "generator_years": "27 LLMs from GPT-J to GPT-3.5 and LLaMA (2019-2022)",
        "human_provenance": "pinned-2019",
        "human_domains_excluded": ["sci_gen"],
        "human_note": "nine pre-2020 corpora (CMV, Yelp, XSum, TLDR, ELI5, WritingPrompts, ROC, "
                      "HellaSwag, SQuAD); the SciGen human rows (arXiv through 2021) are dropped",
        "caveats": ["MAGE's own license terms conflict across its source domains (it "
                    "re-publishes several licensed corpora); treat this dataset as "
                    "measurement-only, nothing republished beyond aggregate counts.",
                    "Punctuation was normalized and line breaks removed before release, so "
                    "paragraph, whitespace and markdown-structure signals are gone from both "
                    "splits."],
        "fetch": fetch_mage,
        "cells": [
            {"label": "human", "lang": "prose"},
            {"label": "ai", "lang": "prose"},
        ],
    },
    "semeval24-m4": {
        "kind": "hf_filter", "natlang": "en",
        "dataset": "d0rj/SemEval2024-task8", "config": "subtaskA_monolingual", "split": "train",
        "text_col": "text", "generator_col": "model",
        "url": "https://huggingface.co/datasets/d0rj/SemEval2024-task8",
        "paper": "arXiv 2404.14183 (task overview), arXiv 2305.14902 (corpus)",
        "license": "Apache-2.0",
        "generator_years": "ChatGPT, GPT-3 davinci, Cohere, Dolly (2023)",
        "human_provenance": "pinned-2019",
        "human_note": "PeerRead (2007-2017) plus pre-2020 arXiv, Reddit and WikiHow text; the "
                      "wikipedia human rows are dropped, their machine rows kept, the same "
                      "treatment HC3's wiki_csai config gets",
        "caveats": [
            "WikiHow is CC-BY-NC-SA and Wikipedia CC-BY-SA upstream, which the Apache-2.0 "
            "mirror label does not override; never quote a line from this dataset in an issue.",
        ],
        "fetch": fetch_hf_filter_cell,
        "cells": [
            {"label": "human", "lang": "prose", "generator": "human",
             "where": '"label"=0 and "source"<>\'wikipedia\''},
            {"label": "ai", "lang": "prose", "where": '"label"=1'},
        ],
    },
    "gpt2-output": {
        "kind": "url_jsonl", "natlang": "en",
        "url_human": "https://openaipublic.azureedge.net/gpt-2/output-dataset/v1/webtext.test.jsonl",
        "url_ai": "https://openaipublic.azureedge.net/gpt-2/output-dataset/v1/xl-1542M.test.jsonl",
        "url": "https://github.com/openai/gpt-2-output-dataset",
        "paper": None, "license": "MIT",
        "generator_years": "GPT-2 1542M (2019)",
        "human_provenance": "pinned-2019",
        "human_note": "WebText: Reddit-outbound links with karma >= 3, scraped through "
                      "December 2017",
        "caveats": [
            "Top-K 40 sampling shifts the part-of-speech distribution (underuses proper "
            "nouns, overuses pronouns) per OpenAI's own detection.md, so a pronoun or opener "
            "rule can look good here for a sampling reason rather than a style one; the "
            "plain xl-1542M file is registered, not a -k40 variant.",
            "Documents near 500 characters detect about 15% worse per the same note, so "
            "length is a confound on this dataset.",
        ],
        "fetch": fetch_gpt2_output,
        "cells": [
            {"label": "human", "lang": "prose", "generator": "human"},
            {"label": "ai", "lang": "prose", "generator": "gpt-2-xl-1542m"},
        ],
    },
    "ghostbuster": {
        "kind": "git_multi", "natlang": "en",
        "repo": "https://github.com/vivek3141/ghostbuster-data",
        "url": "https://github.com/vivek3141/ghostbuster-data",
        "paper": "arXiv 2305.15047", "license": "CC-BY-3.0",
        "generator_years": "gpt-3.5-turbo and claude (2023)",
        "human_provenance": "pinned-2019",
        "human_domains": ["reuter", "wp"],
        "human_note": "Reuters 50-50 news (1996-97) and r/WritingPrompts stories (2017-18); the "
                      "undated IvyPanda essays are dropped from the human split",
        "caveats": ["gpt_prompt*/gpt_semantic/gpt_writing variants are skipped; only the "
                    "plain gpt/ and claude/ generations are counted as `ai`.",
                    "Every split carries a sibling logprobs/ tree of GPT-2 token/score dumps, "
                    "two thirds of the .txt files under human/; they are excluded, because they "
                    "are model output about a document rather than the document.",
                    "AI documents were generated from prompts derived from the paired human "
                    "document with a target length, so length is matched by construction."],
        "fetch": fetch_ghostbuster,
        "cells": [
            {"label": "human", "lang": "prose"},
            {"label": "ai", "lang": "prose"},
        ],
        "exclude": ("gpt_prompt", "gpt_semantic", "gpt_writing", "logprobs"),
    },
    "faidset": {
        "kind": "url_table", "natlang": "en",
        "dataset": "ngocminhta/FAIDSet",
        "url_file": "https://huggingface.co/datasets/ngocminhta/FAIDSet/resolve/main/test.jsonl",
        "url": "https://huggingface.co/datasets/ngocminhta/FAIDSet",
        "paper": "arXiv 2505.14271", "license": "MIT",
        "generator_years": "GPT-4o, Gemini 2, Llama 3 and DeepSeek V3/R1 (2024-25; model column)",
        "human_provenance": "unverified",
        "human_note": "undated academic theses and essays collected 2024-25; English rows only",
        "caveats": ["The published `test.jsonl` mixes English and Vietnamese with no language "
                    "column; rows holding a Vietnamese-only letter are dropped before "
                    "sampling.",
                    "`ai-collab` is the dataset's human-LLM collaborative class, a robustness "
                    "split never pooled with plain ai."],
        "fetch": fetch_faidset,
        "cells": [
            {"label": "human", "lang": "prose"},
            {"label": "ai", "lang": "prose"},
            {"label": "ai-collab", "lang": "prose"},
        ],
    },
    "beemo": {
        "kind": "hf_rows", "natlang": "en",
        "dataset": "toloka/beemo",
        "url": "https://huggingface.co/datasets/toloka/beemo",
        "paper": "arXiv 2411.04032", "license": "MIT (edits); prompts and human outputs CC-BY-NC-4.0",
        "generator_years": "GPT-4o, Llama 3.1 70B, Mixtral, Gemma, Mistral 7B, Zephyr (2023-24)",
        "human_provenance": "verified-authors",
        "human_note": "No Robots (2023): responses written by expert annotators to the same prompts "
                      "the models answered",
        "caveats": ["`ai-human-edited` is the model output after an expert edit; `ai-llm-edited` "
                    "is the model output rewritten by GPT-4o or Llama 3.1 70B. Both are "
                    "robustness splits and never pooled with plain ai.",
                    "Every split answers the same 2,187 prompts, so topic is matched by "
                    "construction."],
        "fetch": fetch_beemo,
        "cells": [
            {"label": "human", "lang": "prose"},
            {"label": "ai", "lang": "prose"},
            {"label": "ai-human-edited", "lang": "prose"},
            {"label": "ai-llm-edited", "lang": "prose"},
        ],
    },
    "aidev": {
        "kind": "hf_rows", "natlang": "en",
        "dataset": "hao-li/AIDev",
        "url": "https://huggingface.co/datasets/hao-li/AIDev",
        "paper": "arXiv 2507.15003", "license": "CC-BY-4.0",
        "generator_years": "Claude Code, OpenAI Codex, Cursor, Devin, Copilot and Jules agents (2025)",
        "human_provenance": None,
        "human_note": None,
        "caveats": ["AI-only: pull-request descriptions written by autonomous coding agents on "
                    "public GitHub repositories; the technical-prose baseline is the pinned "
                    "cpython-doc and rust-book cells.",
                    "A description is Markdown a maintainer reads; SLOP029/SLOP035 style rules "
                    "see their natural habitat here."],
        "fetch": fetch_aidev,
        "cells": [{"label": "ai", "lang": "prose"}],
    },
    "apt-eval": {
        "kind": "url_table", "natlang": "en",
        "dataset": "smksaha/apt-eval",
        "url_file": "https://huggingface.co/datasets/smksaha/apt-eval/resolve/main/merged_apt_eval_dataset.csv",
        "url_human": "https://huggingface.co/datasets/smksaha/apt-eval/resolve/main/original.csv",
        "url": "https://huggingface.co/datasets/smksaha/apt-eval",
        "paper": "arXiv 2502.15666", "license": "CC-BY-4.0",
        "generator_years": "GPT-4o, Llama 3.1 70B, Llama 3 8B and DeepSeek-V3 as polishers (2024-25)",
        "human_provenance": "unverified",
        "human_note": "human blog, email, news, review and speech texts drawn from MixSet and "
                      "similar 2024 collections",
        "caveats": ["`ai-polished` is the human text after an LLM polish of a stated degree "
                    "(`domain` records domain/polish type/degree); there is no plain-ai split, "
                    "so this dataset measures how far a light polish moves each rule."],
        "fetch": fetch_apt_eval,
        "cells": [
            {"label": "human", "lang": "prose"},
            {"label": "ai-polished", "lang": "prose"},
        ],
    },
    "pan25": {
        "kind": "url_table", "natlang": "en",
        "url_file": "https://zenodo.org/records/14962653/files/"
                    "pan25-generative-ai-detection-task1-train.zip",
        "url": "https://zenodo.org/records/14962653",
        "paper": None,
        "license": "research use only, no redistribution (Zenodo record terms)",
        "generator_years": "23 models spanning 2023-2025, including gpt-4o, o3-mini, "
                           "gemini-2.0-flash, deepseek-r1-distill-qwen-32b, "
                           "llama-3.3-70b-instruct and gpt-4.5-preview (model column)",
        "human_provenance": "unverified",
        "human_note": "fiction, essay and news human text; the fiction/essay sources could "
                      "not be confirmed, the news text is dated 2021",
        "caveats": [
            "The dataset's license permits research use only and forbids redistribution; "
            "fetched and scored locally like CodeMirage, but never quote a line from it "
            "anywhere, including in an issue.",
            "`gpt-4-turbo-paraphrase` and `gemini-pro-paraphrase` are machine-humanized "
            "rewrites of existing ai text, not organic generations; they are labelled "
            "`ai-paraphrased` and never pooled with plain `ai`.",
        ],
        "fetch": fetch_pan25,
        "cells": [
            {"label": "human", "lang": "prose"},
            {"label": "ai", "lang": "prose"},
            {"label": "ai-paraphrased", "lang": "prose"},
        ],
    },
    "cpython-doc": {
        "kind": "git_tag", "natlang": "en",
        "repo": "https://github.com/python/cpython", "tag": "v3.8.0",
        "url": "https://github.com/python/cpython", "paper": None, "license": "PSF-2.0",
        "generator_years": None,
        "human_provenance": "pinned-2019",
        "human_note": "CPython 3.8.0 Doc, tagged October 2019",
        "caveats": ["Shares its clone with cpython-lib (same tag). Written as `.rst`: the "
                    "boldface/heading-style rules (SLOP019/SLOP021) never fire on `.rst`, "
                    "only on Md/Mdx/Html, so those two rows read 0 here by construction."],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "prose", "ext": "rst", "generator": "human",
                   "subdir": "Doc", "glob": "*.rst"}],
        "exclude": (),
    },
    "rust-book": {
        "kind": "git_before", "natlang": "en",
        "repo": "https://github.com/rust-lang/book", "before": "2020-01-01",
        "url": "https://github.com/rust-lang/book", "paper": None, "license": "MIT OR Apache-2.0",
        "generator_years": None,
        "human_provenance": "pinned-2019",
        "human_note": "rust-lang/book at its last default-branch commit before 2020-01-01",
        "caveats": ["The repo has no release tags near the cutoff; cloned shallow-since 2019 and "
                    "checked out the last commit before 2020-01-01."],
        "fetch": fetch_git_before_cell,
        "cells": [{"label": "human", "lang": "prose", "generator": "human",
                   "subdir": "src", "glob": "*.md"}],
        "exclude": (),
    },
    "react-docs": {
        "kind": "git_before", "natlang": "en",
        "repo": "https://github.com/reactjs/react.dev", "before": "2020-01-01",
        "url": "https://github.com/reactjs/react.dev", "paper": None, "license": "CC-BY-4.0",
        "generator_years": None,
        "human_provenance": "pinned-2019",
        "human_note": "react.dev docs at their last default-branch commit before 2020-01-01",
        "caveats": ["Files carry YAML frontmatter (id/title/permalink); the repo has no "
                    "release tag near the cutoff, cloned shallow-since 2019 like rust-book."],
        "fetch": fetch_git_before_cell,
        "cells": [{"label": "human", "lang": "prose", "generator": "human",
                   "subdir": "content/docs", "glob": "*.md"}],
        "exclude": (),
    },
    "k8s-docs": {
        "kind": "git_tag", "natlang": "en",
        "repo": "https://github.com/kubernetes/website", "tag": "snapshot-initial-v1.17",
        "url": "https://github.com/kubernetes/website", "paper": None, "license": "CC-BY-4.0",
        "generator_years": None,
        "human_provenance": "pinned-2019",
        "human_note": "kubernetes/website English docs, tagged December 2019",
        "caveats": ["Pages carry Hugo shortcodes (`{{% capture body %}}`); some tutorials "
                    "ship as `.html` instead of `.md`, so the glob stays `.md`-only."],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "prose", "generator": "human",
                   "subdir": "content/en/docs", "glob": "*.md"}],
        "exclude": (),
    },
    "wetbench-pt": {
        "kind": "url_jsonl", "natlang": "pt",
        "url_tmpl": "https://huggingface.co/datasets/cs928346/WETBench/resolve/main/mgt/paragraphs_pt_{model}.jsonl",
        "url": "https://huggingface.co/datasets/cs928346/WETBench",
        "paper": "arXiv 2507.03373", "license": "CC-BY-NC-SA-4.0",
        "generator_years": "GPT-4o mini, Gemini 2.0 Flash, Qwen2.5-7B, Mistral-7B (2024)",
        "human_provenance": "unverified",
        "human_note": "Portuguese Wikipedia paragraphs from 2024 revisions",
        "caveats": ["Portuguese Wikipedia mixes pt-PT and pt-BR; paragraphs are short so "
                    "document-level rules like SLOP041 (needs 200 words) rarely apply."],
        "fetch": fetch_wetbench_pt,
        "cells": [
            {"label": "human", "lang": "prose"},
            {"label": "ai", "lang": "prose"},
        ],
    },
    "diplomatrix": {
        "kind": "url_json", "natlang": "pt",
        "url": "https://huggingface.co/datasets/melll-uff/diplomatrixbr-gen/resolve/main/Diplomatrixbr.json",
        "paper": None, "license": "MIT",
        "generator_years": "GPT-4o, Claude 3, Gemini, Llama 3, Sabiá and other 2024 models",
        "human_provenance": "verified-authors",
        "human_note": "CACD diplomatic-career exam essays, handwritten by identified candidates "
                      "under supervision",
        "caveats": ["The human split holds under a hundred essays, so its rates move a full "
                    "point per file."],
        "fetch": fetch_diplomatrix,
        "cells": [
            {"label": "human", "lang": "prose"},
            {"label": "ai", "lang": "prose"},
        ],
    },
    "essay-br": {
        "kind": "url_csv", "natlang": "pt",
        "url": "https://raw.githubusercontent.com/rafaelanchieta/essay/master/essay-br/essay-br.csv",
        "paper": None, "license": "MIT",
        "generator_years": None,
        "human_provenance": "unverified",
        "human_note": "ENEM-style student essays published on a correction site 2015-2020; the "
                      "2020 tail postdates the bar and authors are anonymous",
        "caveats": ["Human-only: student essays for a national exam prompt.",
                    "ENEM pedagogy teaches the recap-connective close, so SLOP029-family "
                    "signals read as genre here, not as a tell."],
        "fetch": fetch_essay_br,
        "cells": [{"label": "human", "lang": "prose",
                   "url": "https://raw.githubusercontent.com/rafaelanchieta/essay/master/essay-br/essay-br.csv"}],
    },
    "lener-br": {
        "kind": "git_tag", "natlang": "pt",
        "repo": "https://github.com/peluz/lener-br",
        "url": "https://github.com/peluz/lener-br",
        "paper": "PROPOR 2018 (Universidade de Brasília)",
        "license": "MIT (packaging); the raw texts are public-domain Brazilian federal "
                   "statutes and court rulings (Lei 9.610/98 Art. 8)",
        "generator_years": None,
        "human_provenance": "pinned-2019",
        "human_note": "Brazilian federal statutes and court rulings dated 2008-2018 by their "
                      "own text; the repo's own commit history only starts 2020-05-15, so the "
                      "cutoff rests on the documents' stated dates, not the clone",
        "caveats": [],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "prose", "generator": "human",
                   "subdir": "leNER-Br/raw_text", "glob": "*.txt"}],
        "exclude": (),
    },
    "lovable-ui": {
        "kind": "git_pinned", "natlang": None,
        "url": "https://github.com/Anaislegousky/Memory-lane", "paper": None,
        "license": "mixed, per repo",
        "generator_years": "Lovable, commits dated 2026-06-30 to 2026-10-14",
        "human_provenance": None,
        "human_note": None,
        "caveats": [
            "28 public repos pinned by SHA. Provenance marker: at least half of each pinned repo's recent commits are authored or co-authored by gpt-engineer-app[bot], Lovable's committer identity; each pin is the newest such commit.",
            "Only .tsx, .css and .html under src/ and client/src/ (plus index.html) are scored; components/ui/ vendored shadcn primitives and *.min.* files are excluded, so no hand-copied primitive counts as builder output.",
            "The marker proves the builder created the project, not that it wrote every line: an owner may have edited files before the pin.",
        ],
        "fetch": fetch_git_pinned_set,
        "repos": [
            {"repo": "Anaislegousky/Memory-lane", "sha": "29643aacbe576b5ca6bfbb81a4431c180a40d959"},
            {"repo": "ChemayekAbraham/agent-Ops", "sha": "f796b57424e2c60597069403e5620b2277eabf8e"},
            {"repo": "Deversie-Debug/website-sync-hub", "sha": "72023a79f15ca8adc9f66c9c13db55aea54da832"},
            {"repo": "Gigid-design/flight-fare-finder", "sha": "c351803a6307618dcd20bce57b60cc7f64be47e9"},
            {"repo": "Lovable-Leapmile/leapmilewarehousemonitorapp", "sha": "48438638eca81bc89d469aef34240ace6122eb49"},
            {"repo": "MarineAnais/Site_Marine_Anais", "sha": "a6c6c662550fe8aab33c9251fd078bd419174346"},
            {"repo": "Mzero-Patrick/FC-Portal", "sha": "da9f38eb15803a4447ab4d25e82abb3793e26d90"},
            {"repo": "OysGut/animatic-studio", "sha": "91d2c5c84833663a4b8f3621554b548b08ed413a"},
            {"repo": "Prashanth-webscafe02/aasta_lifespace", "sha": "be717da5f716d5a1188bdba677e9379914706ee1"},
            {"repo": "SirAlexanderWeiss/indonesia-to-germany", "sha": "0aa8b83353aea0947e726be45c0e4df6cf913375"},
            {"repo": "Spanky-BS/pixel-perfect-pixels", "sha": "25a41454c415574c40f504a132daafab83b64690"},
            {"repo": "Valou44/valentin-exe", "sha": "60f3fbc3698d9ff02d60992087f640bc18bf31b5"},
            {"repo": "YaxielGonz/Jesy-Sistema", "sha": "f8ea4adcb2564764659e1361ad3d1b33dec87ded"},
            {"repo": "aimeniyomizero970-beep/rwf-event-hub", "sha": "b62bbd9212b85eb738d63903a5aa9b221975c06e"},
            {"repo": "anderfej18-cmd/genius-nurse-hub", "sha": "911b66d6fae48674b27ac2b2993fa4392625d109"},
            {"repo": "dantun9090-crypto/phlab-0c310b4c", "sha": "e2c55c39251ad1dd90b9a62702d1c631540b1163"},
            {"repo": "delixweb17/erasmus-planner-pro", "sha": "a1d2f2583097d28b91a355b21edfad42b91f8fe7"},
            {"repo": "evilgloomy/studioassembly", "sha": "137ccd3e7629ebb1e136ef097e3d6bd4d9f3779e"},
            {"repo": "goldealsclub/goldealsclub", "sha": "74e3bc4f45c18e1c5aa172d6754a438b2d583566"},
            {"repo": "igormoisesads-byte/remix-of-cloud-os-for-agencies", "sha": "f7f38abfc37c037242c8bbd581749e3efb359bf4"},
            {"repo": "jaythekong/easyroom", "sha": "823c86d152897fe90af02687bba4d7ee6d33814d"},
            {"repo": "jerrysuesue/flight-deal-finder", "sha": "4bfe6fadd0a5219c12a6251060c4e75ca173d559"},
            {"repo": "kozachenkoroman514/Tag-Master---Migration", "sha": "9d3029907edd5a3eb5747859f80f51063415326a"},
            {"repo": "militarygradeinsulation-hue/obsidiancoder", "sha": "91032adeb00f7103e2ea69df86bf6dcca17af96e"},
            {"repo": "overurdream-collab/arab-magic-decoder", "sha": "09db38c21326b067e4312dbcfac68d4c87305894"},
            {"repo": "rebeccamaynard1/lost-furry-friend-alerts", "sha": "e9f5d8dcfba82151ace7ccb99a20a8b2f6002984"},
            {"repo": "sanvicpalawan/kapwa-resort-webapp", "sha": "b7e04a202db6b01d795251c9035802338a53c3fc"},
            {"repo": "sar-star1/peremogabetasite", "sha": "e20ca5479f61c0ad66423f48243d70eec7b5e9f8"},
        ],
        "cells": [
            {"label": "ai", "lang": "tsx", "generator": "Lovable", "subdir": ("src", "client/src"), "glob": "*.tsx"},
            {"label": "ai", "lang": "css", "generator": "Lovable", "subdir": ("src", "client/src"), "glob": "*.css"},
            {"label": "ai", "lang": "html", "generator": "Lovable", "subdir": ("src", "client/src", "index.html", "client/index.html"), "glob": "*.html"},
        ],
        "exclude": UI_EXCLUDE,
    },
    "bolt-ui": {
        "kind": "git_pinned", "natlang": None,
        "url": "https://github.com/Gaganpraveen/AuraM", "paper": None,
        "license": "mixed, per repo",
        "generator_years": "bolt.new, commits dated 2024-10-08 to 2026-03-26",
        "human_provenance": None,
        "human_note": None,
        "caveats": [
            "12 public repos pinned by SHA. Provenance marker: each pinned repo carries .bolt/config.json or .bolt/prompt at the pinned HEAD, files bolt.new writes into every project; repos are capped at 15 commits to skip heavy human follow-up.",
            "Only .tsx, .css and .html under src/ and client/src/ (plus index.html) are scored; components/ui/ vendored shadcn primitives and *.min.* files are excluded, so no hand-copied primitive counts as builder output.",
            "The marker proves the builder created the project, not that it wrote every line: an owner may have edited files before the pin.",
        ],
        "fetch": fetch_git_pinned_set,
        "repos": [
            {"repo": "Gaganpraveen/AuraM", "sha": "c7cd57c331ef725a01d24f8703e78a458b62f76b"},
            {"repo": "JCBKPP/tldsystems2", "sha": "48331f92209e14465d63a2dfb6e7bb2fc965fa69"},
            {"repo": "Mlitrading/Third-voice-09", "sha": "622404ea41b7641a117ef58c42d72b40c4f3f291"},
            {"repo": "Rajat119/dialer-pad-task", "sha": "871edc88c6265f87317bae92ef3435a07d7ad071"},
            {"repo": "SUJENDRANJ/anyone-can-rent", "sha": "45f5f9f30bfd291892b436f4a12545f6013117d2"},
            {"repo": "ckrstars/bolt-crazy-girls-salon", "sha": "55b1951740614d9dcc573176fcd72190553ecf63"},
            {"repo": "elviradconcierge/CONCIERGE-DIGITAL", "sha": "748fef4cbbdb4492ce04c7b241fb6ce951953d74"},
            {"repo": "kende/sb1-ttkgtw", "sha": "1145a6bb3cee1c76ff988456a4da575f919dc98b"},
            {"repo": "mdmasudrana9/hidaiyaClient", "sha": "b69cf6f38b918d4aeebec55c9c067a2cd7315e7e"},
            {"repo": "meistericham/pcr_project_tracker", "sha": "ade16675a2cd6a6ce77e49c0204827fd7e1b9d73"},
            {"repo": "prem22k/MemeCoin-Roulette", "sha": "25b57c717d40c5a940be22cc7bd496e60d74d466"},
            {"repo": "umer4447/Stock-Exchange-Web-Template", "sha": "fcf453b9d94e4b0ae480b832b28800dcac942ff3"},
        ],
        "cells": [
            {"label": "ai", "lang": "tsx", "generator": "bolt.new", "subdir": ("src", "client/src"), "glob": "*.tsx"},
            {"label": "ai", "lang": "css", "generator": "bolt.new", "subdir": ("src", "client/src"), "glob": "*.css"},
            {"label": "ai", "lang": "html", "generator": "bolt.new", "subdir": ("src", "client/src", "index.html", "client/index.html"), "glob": "*.html"},
        ],
        "exclude": UI_EXCLUDE,
    },
    "same-ui": {
        "kind": "git_pinned", "natlang": None,
        "url": "https://github.com/Harshi-max/AI-Interviewer", "paper": None,
        "license": "mixed, per repo",
        "generator_years": "Same.new, commits dated 2025-05-19 to 2026-09-17",
        "human_provenance": None,
        "human_note": None,
        "caveats": [
            "12 public repos pinned by SHA. Provenance marker: each pinned repo carries .same/todos.md at the pinned HEAD, the task file the Same agent keeps; repos are capped at 15 commits to skip heavy human follow-up.",
            "Only .tsx, .css and .html under src/ and client/src/ (plus index.html) are scored; components/ui/ vendored shadcn primitives and *.min.* files are excluded, so no hand-copied primitive counts as builder output.",
            "The marker proves the builder created the project, not that it wrote every line: an owner may have edited files before the pin.",
        ],
        "fetch": fetch_git_pinned_set,
        "repos": [
            {"repo": "Harshi-max/AI-Interviewer", "sha": "b1e5fcfb511aad859d839e31535512284d063044"},
            {"repo": "Jaque152/Apoyolegal", "sha": "807150afe0a7f265cd12400e96880c8750a95ee2"},
            {"repo": "MarcusAlienx/empaquestransformados", "sha": "bf22154f35a4542a14e704d0a0337bb2b8cdfe60"},
            {"repo": "NisargIO/slack-clone", "sha": "692568477a046055ba716c2d75e7c4341701f17a"},
            {"repo": "afirstimer/viettin-fe", "sha": "a1a339e74ea95f1634167089ed89a4dece25e2a3"},
            {"repo": "akhaldi94/medical-dashboard", "sha": "7c3f19c69c4b79bfa6adbda06879d18d793a39cf"},
            {"repo": "djbrooker12/joshua-wilson-portfolio-clone", "sha": "5b9703abd31a51d2e00f645cd0d9c0099679ba39"},
            {"repo": "fullstackslife/bellaamour.me", "sha": "b34f924da6c82af082cd59e8dfd8e17093af7d0d"},
            {"repo": "holystunner/ai-agent-team", "sha": "999afba94dad3f25bebaba85ab965ce0116435ea"},
            {"repo": "ina-dot/kbeauty-b2b", "sha": "e45c90c355bffce744f697e80e05bd9abfb0e9e9"},
            {"repo": "ovalaiops/figma-design-generator", "sha": "35a1f47975c7ee4d0347a492ff6d55875a85f4be"},
            {"repo": "tuttoxa9/advahta4", "sha": "309c4f482a6db299c030f7c8fd795858f2ac11d2"},
        ],
        "cells": [
            {"label": "ai", "lang": "tsx", "generator": "Same", "subdir": ("src", "client/src"), "glob": "*.tsx"},
            {"label": "ai", "lang": "css", "generator": "Same", "subdir": ("src", "client/src"), "glob": "*.css"},
            {"label": "ai", "lang": "html", "generator": "Same", "subdir": ("src", "client/src", "index.html", "client/index.html"), "glob": "*.html"},
        ],
        "exclude": UI_EXCLUDE,
    },
    "replit-ui": {
        "kind": "git_pinned", "natlang": None,
        "url": "https://github.com/0xTariq-dev/Askolo", "paper": None,
        "license": "mixed, per repo",
        "generator_years": "Replit Agent, commits dated 2025-06-21 to 2026-10-14",
        "human_provenance": None,
        "human_note": None,
        "caveats": [
            "12 public repos pinned by SHA. Provenance marker: at least 40% of each pinned repo's recent commits carry the Replit-Commit-Author: Agent trailer; each pin is the newest such commit.",
            "Only .tsx, .css and .html under src/ and client/src/ (plus index.html) are scored; components/ui/ vendored shadcn primitives and *.min.* files are excluded, so no hand-copied primitive counts as builder output.",
            "The marker proves the builder created the project, not that it wrote every line: an owner may have edited files before the pin.",
        ],
        "fetch": fetch_git_pinned_set,
        "repos": [
            {"repo": "0xTariq-dev/Askolo", "sha": "1c102cfcde35a3078e8c178d42e250d1689ba542"},
            {"repo": "Coviningfan/ZahalNew2026", "sha": "f4533e0f8fba5cd07e75c2159abe97206445ae7e"},
            {"repo": "IAmSpring/StrudelLive", "sha": "f57bb0e098701962a5e0cb6f02249aaff18d8d1f"},
            {"repo": "MPBF/MPBF-V2", "sha": "5ffbecf1fe01b175d5e5e1653dcc5d14c530d36a"},
            {"repo": "OnFire-Health/platform-demo", "sha": "44d3c5e835f56f86d85f8f20541bcddfc039ec2c"},
            {"repo": "Schweizerpunkt-GmbH/Dental-Second-Opinion-Platform---AI-powered-dental-case-review-system", "sha": "b9746ddd7f8a55517d7663f878445f15db4ab55c"},
            {"repo": "Stackstudio-cloud/DevConnect", "sha": "a40e071b2cf4e86e03b977a8b1d87b36ccac08cf"},
            {"repo": "nyainda/Bluesky-Feed-Master", "sha": "23becbc787f6827e17ceb32f0fb3c284dff9d492"},
            {"repo": "saifshbeita/Lumi", "sha": "63cb1b4917222db68ddbebb39f5d1f2e5ddb5a98"},
            {"repo": "saikumarreddymopuri/saikumarr-portfolio", "sha": "ab56291802c2ba1f96222735efd9f97b7c33e3c1"},
            {"repo": "wwwtrendwearsshop-lab/Hi", "sha": "aadebbbb55d21f6c4c2d4607b50d5a97e94b9552"},
            {"repo": "yanliudesign/Vocalize", "sha": "8d0d0867bfee4261f4dec5b318b3c878ad2b5126"},
        ],
        "cells": [
            {"label": "ai", "lang": "tsx", "generator": "Replit Agent", "subdir": ("src", "client/src"), "glob": "*.tsx"},
            {"label": "ai", "lang": "css", "generator": "Replit Agent", "subdir": ("src", "client/src"), "glob": "*.css"},
            {"label": "ai", "lang": "html", "generator": "Replit Agent", "subdir": ("src", "client/src", "index.html", "client/index.html"), "glob": "*.html"},
        ],
        "exclude": UI_EXCLUDE,
    },
    "tailwind-tsx-2022": {
        "kind": "git_pinned", "natlang": None,
        "url": "https://github.com/vercel/commerce", "paper": None, "license": "mixed, per repo",
        "generator_years": None,
        "human_provenance": "verified-authors",
        "human_note": "11 well-known open-source Tailwind React apps by named maintainers, each pinned to its last commit before 2023-01-01 (commit dates 2022-11-19 to 2022-12-30), before AI app builders existed",
        "caveats": [
            "No pre-2020 Tailwind React baseline exists: Tailwind's React ecosystem and its violet palette postdate 2019, so the human bar is late 2022, weeks after ChatGPT launched.",
            "Large repos (cal.com, saleor/storefront) contribute proportionally more files to the pooled sample.",
            "Product apps, not landing pages: a marketing hero is where gradient text concentrates, so this baseline may understate the human rate on landing-page code.",
        ],
        "fetch": fetch_git_pinned_set,
        "repos": [
            {"repo": "calcom/cal.com", "sha": "518a4484d62482e358acfd3a168929646d942a78"},
            {"repo": "shadcn-ui/taxonomy", "sha": "0bace50fcac775e7214eab01c96f7fea90d48e8c"},
            {"repo": "vercel/commerce", "sha": "4efa50266641b4573ae4ef3360d07cc0ff21046a"},
            {"repo": "leerob/leerob.io", "sha": "675ba074e873a85f1ee6a0d195f7d20312e62b4e"},
            {"repo": "vercel/platforms", "sha": "83c330f414fc4503e814db94175fe9860083c5fe"},
            {"repo": "tailwindlabs/headlessui", "sha": "865bd57357dd9a9d2f1cc9196253b387c6f47772"},
            {"repo": "ixartz/Next-js-Boilerplate", "sha": "8660c9ba92bbafe275f4f437fbeb3f69080a1657"},
            {"repo": "Infisical/infisical", "sha": "01d969190bc24f84d2106c1db9d9bc3b22dd2dbd"},
            {"repo": "vercel/nextjs-subscription-payments", "sha": "1c03b0927403be063e6e2fcc6e715c81ce2efcf6"},
            {"repo": "saleor/storefront", "sha": "e99a39d7c269386f3ba53e8df50cbd2f7daef28a"},
            {"repo": "medusajs/nextjs-starter-medusa", "sha": "979f4e31f375d2e45365b53c551b7aa256382d23"},
        ],
        "cells": [
            {"label": "human", "lang": "tsx", "generator": "human", "subdir": ".", "glob": "*.tsx"},
            {"label": "human", "lang": "css", "generator": "human", "subdir": ".", "glob": "*.css"},
        ],
        "exclude": UI_EXCLUDE,
    },
}

# Synthesized cells written by bench/generate_corpus.py (not part of this file's ownership);
# included automatically when present, silently absent otherwise.
SYNTH_CELLS = {
    "synth-pt-wiki": {"lang": "prose", "natlang": "pt"},
    "synth-pt-essay": {"lang": "prose", "natlang": "pt"},
    "synth-tsx": {"lang": "tsx", "natlang": None},
    "synth-ts": {"lang": "typescript", "natlang": None},
    "synth-en-readme": {"lang": "prose", "natlang": "en"},
    "synth-rust": {"lang": "rust", "natlang": None},
}


def discover_synth_datasets(root):
    found = {}
    for name, spec in SYNTH_CELLS.items():
        cell_dir = os.path.join(root, "synth", name, "ai", spec["lang"])
        if not os.path.isdir(cell_dir):
            continue
        found[name] = {
            "kind": "local", "natlang": spec["natlang"], "url": None, "paper": None, "license": None,
            "synthesized": True,
            "caveats": ["synthesized by bench/generate_corpus.py, not a real-world sample."],
            "cells": [{"label": "ai", "lang": spec["lang"], "dir": cell_dir}],
        }
    return found


# The two `kind` families `orchestrate.fetch_dataset` dispatches on: CELL_KINDS fetches one
# cell at a time, DATASET_KINDS fetches every cell of a dataset at once. `"local"` is the
# third, closed member `self_check` asserts against.
CELL_KINDS = ("hf_filter", "hf_text_rows", "url_csv", "git_tag", "git_before", "git_pinned")
DATASET_KINDS = ("hf_rows", "url_jsonl", "url_json", "url_table", "git_multi")
