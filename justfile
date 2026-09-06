# Commands for building, checking and benchmarking stopslop.
#
# `just` is a task runner, not a crate dependency: nothing here is required to build or use
# the linter, and every recipe is one command you can also run by hand.
#
# The corpus recipes split on the network boundary. `corpus-fetch` downloads and materializes
# every cell and writes `target/corpus/fetched.json`; `corpus-score` reads that manifest and
# never touches the network, so scoring a rule change is offline and reproducible. Hugging
# Face cells authenticate through HF_TOKEN (or HF_API_TOKEN) when it is set. Datasets are
# never committed, and neither is the rendered HTML: several of them forbid redistributing a
# derivative. The only committed output is bench/corpus_report.md.
#
# Override any variable per run: `just limit=50 corpus-fetch`.

bin := "target/release/stopslop"
corpus := "target/corpus"
limit := "500"

# List the recipes.
default:
    @just --list

# Build the release binary the corpus recipes lint with.
build:
    cargo build --release

# The three CI gates plus the dogfood run, in CI's order.
check: build
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    {{bin}} .

# Download and materialize every corpus cell (needs the network).
corpus-fetch:
    python3 bench/score_corpus.py --dir {{corpus}} --limit {{limit}} --fetch-only --skip-failures

# Lint every fetched cell; writes bench/corpus_report.md and results.json.
corpus-score: build
    python3 bench/score_corpus.py --dir {{corpus}} --bin {{bin}} --no-fetch \
        --report bench/corpus_report.md --json {{corpus}}/results.json

# Measure the candidate tells, the per-message breakdown and the phrase differential.
corpus-analyze:
    python3 bench/analyze_corpus.py --dir {{corpus}} --results {{corpus}}/results.json \
        --candidates bench/candidates.toml --json {{corpus}}/analysis.json --report {{corpus}}/analysis.md

# Render the standalone scorecard at target/corpus/report.html.
corpus-html:
    python3 bench/render_report.py --results {{corpus}}/results.json \
        --analysis {{corpus}}/analysis.json --out {{corpus}}/report.html

# The whole benchmark: fetch, score, analyse, render.
corpus: corpus-fetch corpus-score corpus-analyze corpus-html

# Synthesize the AI splits no public dataset covers (calls the Anthropic API, costs money).
corpus-generate:
    uv run bench/generate_corpus.py --dir {{corpus}} --cells pt-wiki,pt-essay,tsx,rust,en-readme --limit 200

# Drop the materialized cells and generated outputs, keeping the download cache.
corpus-clean:
    find {{corpus}} -mindepth 1 -maxdepth 1 ! -name cache -exec rm -rf {} +

# Assert the bench scripts' own invariants.
corpus-self-check:
    python3 bench/score_corpus.py --self-check
    python3 bench/analyze_corpus.py --self-check
    python3 bench/render_report.py --self-check
