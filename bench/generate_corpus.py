# /// script
# requires-python = ">=3.10"
# dependencies = ["anthropic"]
# ///
"""Synthesize AI-authored corpus cells for cells no public dataset covers.

usage: uv run bench/generate_corpus.py [--cells synth-pt-wiki,synth-pt-essay,synth-tsx,synth-en-readme,synth-rust]
                                        [--limit N] [--dir DIR] [--workers N]
                                        [--model MODEL] [--yes]
       uv run bench/generate_corpus.py --self-check

This calls the Claude API and costs money -- it prints the request count and an
estimated cost before it sends a single request, and refuses to proceed without
--yes or an interactive "y". bench/score_corpus.py (not touched by this script)
scores rule hit rates on human-vs-machine corpora; five cells it would want --
Brazilian Portuguese prose (two ways), TSX, README-style Markdown, and Rust --
have no public labelled dataset, so this generator fills the "machine" side
from a public seed (a topic, an essay theme, a component brief, a program
brief) and a default-style prompt with no system prompt, because the crate
measures default model style, not a persona.

Every output file is raw model output kept verbatim: code fences, "Here's the
component you asked for" preambles, everything. SLOP001-SLOP004 exist to catch
exactly that pasted-assistant-output residue, so stripping it here would erase
the signal those rules are measured against. The scorer labels every file under
a cell's `ai/` directory as synthesized, never as "this file is AI-written" --
this tool reports per-rule hit rates, it does not accuse a file of authorship.
"""
import argparse
import ast
import concurrent.futures
import csv
import json
import os
import random
import shutil
import sys
import threading
import urllib.request

import anthropic

# Fixed so a rerun with the same --limit picks the same files across machines
# and across resumed runs; not tuned for any distribution property.
SHUFFLE_SEED = 20260904

# $/MTok, in then out. Only the default model is priced; an override prints a
# same-price assumption rather than failing, since this is a cost estimate, not
# a bill.
PRICE_PER_MTOK = {"claude-opus-5": (5.0, 25.0)}

# Placeholders for the spend-gate estimate, not measurements yet -- nobody has run this
# generator against these five brief sets. index.json now records real per-item usage
# (see apply_result); once a run exists, replace these with the observed average.
ASSUMED_INPUT_TOKENS = 200
ASSUMED_OUTPUT_TOKENS = 800

# Sent as `max_tokens` on every request (see call_claude); also the estimate's ceiling,
# since a refusal or a long completion both bill up to this many output tokens.
MAX_TOKENS = 4000

PT_WIKI_SEED_URL = (
    "https://huggingface.co/datasets/cs928346/WETBench/resolve/main/WikiPS/paragraphs_pt.jsonl"
)
PT_ESSAY_SEED_URL = (
    "https://raw.githubusercontent.com/rafaelanchieta/essay/master/essay-br/prompts.csv"
)

TSX_BRIEFS = [
    "a paginated data table with sortable columns and a row-selection checkbox column",
    "a debounced search input that queries an API and shows a dropdown of results",
    "a multi-step form wizard with per-step validation and a progress indicator",
    "a modal dialog component with focus trapping and escape-to-close",
    "a toast notification system with auto-dismiss and stacking",
    "a dark/light theme toggle backed by a React context",
    "an infinite-scroll list that fetches more items as the user scrolls",
    "a drag-and-drop file upload zone with progress bars per file",
    "a tabbed panel component with keyboard arrow-key navigation",
    "a collapsible sidebar navigation menu with nested items",
    "a rich-text editor toolbar wrapping a contenteditable div",
    "a date-range picker with a two-month calendar view",
    "a virtualized list that renders only the visible rows for 10k+ items",
    "a star-rating input component with half-star support",
    "a color picker with a hex input and a saturation/hue canvas",
    "a breadcrumb navigation component derived from the current route",
    "a skeleton loading placeholder for a card grid",
    "a form field component with label, helper text, and error state",
    "a combobox with type-ahead filtering and keyboard selection",
    "a stepper component for a checkout flow with clickable completed steps",
    "a chart wrapper around a canvas that renders a simple bar chart from props",
    "a countdown timer component that calls a callback on completion",
    "a copy-to-clipboard button with a temporary \"copied\" tooltip",
    "a responsive image gallery with a lightbox on click",
    "a kanban board with draggable cards between columns",
    "a rating-and-review list component with pagination",
    "an accordion component where only one panel can be open at a time",
    "a sticky table header that stays visible while scrolling the body",
    "a password input with a show/hide toggle and strength meter",
    "a notification bell icon with an unread-count badge and dropdown",
    "a resizable split-pane layout with a draggable divider",
    "a command palette triggered by a keyboard shortcut",
    "a currency input that formats digits as the user types",
    "a file tree view component with expand/collapse and icons per file type",
    "a rating widget used inline in a product card",
    "a sortable multi-select tag input",
    "a progress bar component with indeterminate and determinate modes",
    "a login form with client-side validation and a submit-disabled state while pending",
    "a pagination control with page-size selection",
    "a context menu that appears on right-click with a list of actions",
    "a form-driven filter sidebar for a product listing page",
    "a user avatar component that falls back to initials when no image is set",
    "a markdown preview pane that renders alongside a textarea",
    "a toggle switch group for enabling/disabling several settings",
    "a sticky \"back to top\" button that appears after scrolling",
    "a table of contents component generated from page headings",
    "a rating-summary bar chart showing the distribution of 1-5 star reviews",
    "a multi-tag filter chip row with removable chips",
    "a two-column responsive layout that stacks on small screens",
    "a form component for editing a list of key-value pairs",
    "a live character counter for a textarea with a max-length warning",
    "a segmented control component for switching between three views",
    "a confirmation dialog hook that returns a promise resolved on the user's choice",
    "an error boundary component with a retry button",
    "a lazy-loaded image component with a blurred placeholder",
    "a sidebar cart drawer that slides in from the right",
    "a rating input styled as a slider instead of stars",
    "a table with expandable row details",
    "a form wizard summary step that lists all previously entered values",
    "a responsive navbar that collapses into a hamburger menu on mobile",
]

README_BRIEFS = [
    "a CLI that deduplicates lines in a file",
    "a CLI that converts CSV files to JSON",
    "a CLI that renames files in bulk using a regex pattern",
    "a CLI that checks for broken links in a Markdown file",
    "a CLI that compresses images in a directory to a target size",
    "a CLI that generates a changelog from conventional commits",
    "a CLI that watches a directory and runs a command on file changes",
    "a CLI that finds and deletes duplicate files by content hash",
    "a CLI that converts Markdown to a styled PDF",
    "a CLI that lints JSON files for trailing commas and formatting",
    "a CLI that measures the size of each top-level folder in a directory",
    "a CLI that generates a random password with configurable rules",
    "a CLI that tails multiple log files with color-coded prefixes",
    "a CLI that validates .env files against a schema",
    "a CLI that converts between YAML and JSON",
    "a CLI that batch-renames image files by their EXIF date",
    "a CLI that checks a website's SSL certificate expiration",
    "a CLI that extracts TODO comments from a codebase into a report",
    "a CLI that diffs two JSON files and prints a readable summary",
    "a CLI that generates boilerplate for a new npm package",
    "a CLI that pings a list of hosts and reports latency",
    "a CLI that splits a large CSV file into smaller chunks",
    "a CLI that counts lines of code per language in a repository",
    "a CLI that schedules a reminder and sends a desktop notification",
    "a CLI that archives files older than N days into a zip",
    "a CLI that converts a directory of images to WebP",
    "a CLI that finds unused dependencies in a Node.js project",
    "a CLI that generates a QR code from a URL or text",
    "a CLI that formats SQL files consistently",
    "a CLI that checks for outdated dependencies across multiple package managers",
    "a CLI that encrypts and decrypts files with a passphrase",
    "a CLI that generates a sitemap.xml from a list of URLs",
    "a CLI that batch-resizes images to a target width",
    "a CLI that converts a Postman collection to a curl script",
    "a CLI that finds large files in a directory tree",
    "a CLI that syncs two directories one-way, like a mini rsync",
    "a CLI that validates JSON Schema documents against sample data",
    "a CLI that extracts frames from a video at a fixed interval",
    "a CLI that generates a .gitignore file from a list of technologies",
    "a CLI that checks spelling in Markdown files against a custom dictionary",
    "a CLI that converts a directory of Markdown files into a single HTML page",
    "a CLI that batch-converts audio files between formats",
    "a CLI that reports git contributors and commit counts for a repository",
    "a CLI that validates that all internal links in a docs site resolve",
    "a CLI that generates dummy test data as CSV or JSON",
    "a CLI that watches a REST API endpoint and alerts on status changes",
    "a CLI that merges multiple PDF files into one",
    "a CLI that extracts all URLs from a set of text files",
    "a CLI that generates a favicon set from a single source image",
    "a CLI that checks Dockerfiles for common anti-patterns",
    "a CLI that converts a directory tree into a text-based outline",
    "a CLI that batch-updates copyright headers across source files",
    "a CLI that finds duplicate lockfile entries across a monorepo",
    "a CLI that generates release notes by grouping merged pull requests",
    "a CLI that validates that environment variables referenced in code are documented",
    "a CLI that converts between CSV and SQLite",
    "a CLI that reports which files in a repo have no test coverage",
    "a CLI that batch-renames git branches by a naming convention",
    "a CLI that generates a project README skeleton from a package.json",
    "a CLI that checks for hardcoded secrets in a codebase",
]

RUST_BRIEFS = [
    "a CLI that tails a log file with a regex filter",
    "an LRU cache with generic keys",
    "a TOML config loader with defaults",
    "a CLI that computes word frequency counts from stdin",
    "a thread-safe bounded queue built on a Mutex and Condvar",
    "a CLI that converts CSV files to JSON",
    "a simple HTTP health-check poller that reports latency",
    "a recursive-descent parser for arithmetic expressions",
    "a CLI that finds duplicate files by content hash",
    "a generic binary search tree with in-order iteration",
    "a CLI that renames files in bulk using a regex pattern",
    "a rate limiter using a token bucket algorithm",
    "a CLI that watches a directory and runs a command on file changes",
    "a simple key-value store backed by an append-only log file",
    "a CLI that validates JSON Schema documents against sample data",
    "a graph struct with breadth-first search over generic node ids",
    "a CLI that measures the size of each top-level folder in a directory",
    "a doubly linked list implemented with Rc<RefCell<>>",
    "a CLI that generates a random password with configurable rules",
    "a simple round-robin task scheduler for async closures",
    "a CLI that pings a list of hosts and reports latency",
    "a trie-based autocomplete engine",
    "a CLI that splits a large CSV file into smaller chunks",
    "a simple in-memory pub/sub event bus with generic message types",
    "a CLI that counts lines of code per language in a repository",
    "a circular buffer implementation for a fixed-size ring",
    "a CLI that archives files older than N days into a zip",
    "a union-find (disjoint set) data structure with path compression",
    "a CLI that finds unused dependencies in a Node.js project's package.json",
    "a simple bloom filter over string keys",
    "a CLI that generates a QR code from a URL or text",
    "a priority queue implemented as a binary heap",
    "a CLI that formats SQL files consistently",
    "a simple actor-style worker pool using channels",
    "a CLI that checks for outdated dependencies across multiple package managers",
    "a minimal Markdown-to-HTML converter",
    "a CLI that encrypts and decrypts files with a passphrase",
    "a simple LRU-based DNS resolver cache",
    "a CLI that generates a sitemap.xml from a list of URLs",
    "a state machine implementation for a traffic light controller",
    "a CLI that batch-resizes images to a target width",
    "a simple B-tree index over sorted integer keys",
    "a CLI that converts a Postman collection to a curl script",
    "a memory-mapped file reader that counts newline-delimited records",
    "a CLI that finds large files in a directory tree",
    "a simple consistent-hashing ring for sharding keys",
    "a CLI that syncs two directories one-way, like a mini rsync",
    "an interval tree for overlapping range queries",
    "a CLI that extracts frames from a video at a fixed interval",
    "a simple CSV parser that handles quoted fields and embedded commas",
    "a CLI that generates a .gitignore file from a list of technologies",
    "a skip list implementation with probabilistic leveling",
    "a CLI that checks spelling in Markdown files against a custom dictionary",
    "a simple write-ahead log with fsync-based durability",
    "a CLI that converts a directory of Markdown files into a single HTML page",
    "a generic stack-based virtual machine for a tiny bytecode language",
    "a CLI that reports git contributors and commit counts for a repository",
    "a simple LFU cache with generic keys",
    "a CLI that validates that environment variables referenced in code are documented",
    "a generic segment tree supporting range-sum queries",
]


def fetch(url, dest):
    """Download once, landing the file atomically so a killed download can't be mistaken for a cached one.

    ponytail: streaming copyfileobj + 120s timeout duplicated from bench/score_corpus.py's
    fetch rather than imported -- this file must keep running as a standalone `uv run`
    script (PEP 723, `anthropic` only), which importing its sibling would break.
    """
    if not os.path.exists(dest):
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        part = f"{dest}.part"
        with urllib.request.urlopen(url, timeout=120) as resp, open(part, "wb") as out:
            shutil.copyfileobj(resp, out)
        os.replace(part, dest)
    return dest


def pt_wiki_seeds(cache_dir):
    path = fetch(PT_WIKI_SEED_URL, os.path.join(cache_dir, "paragraphs_pt.jsonl"))
    seen, seeds = set(), []
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            row = json.loads(line)
            key = (row["page_title"], row["section_title"])
            if key in seen:
                continue
            seen.add(key)
            seeds.append({"id": f"{row['page_title']} / {row['section_title']}", "row": row})
    return seeds


def pt_wiki_prompt(seed):
    row = seed["row"]
    section = row["section_title"].strip("= ").strip()
    return (
        f"Escreva um parágrafo enciclopédico em português brasileiro sobre "
        f'"{row["page_title"]}", especificamente a seção "{section}". '
        "O texto deve ter o estilo de um artigo da Wikipédia em português, "
        "com tom neutro e informativo."
    )


def pt_essay_seeds(cache_dir):
    path = fetch(PT_ESSAY_SEED_URL, os.path.join(cache_dir, "essay_prompts.csv"))
    with open(path, encoding="utf-8", newline="") as fh:
        rows = list(csv.DictReader(fh))
    seeds = []
    for row in rows:
        paragraphs = ast.literal_eval(row["description"])
        seeds.append({"id": row["id"], "row": {"paragraphs": paragraphs}})
    return seeds


def pt_essay_prompt(seed):
    collectanea = "\n\n".join(seed["row"]["paragraphs"])
    return (
        "Escreva uma redação dissertativo-argumentativa nos moldes do ENEM, em "
        "português brasileiro, a partir da seguinte proposta:\n\n"
        f"{collectanea}\n\n"
        "Produza um texto completo, com introdução, desenvolvimento e conclusão, "
        "seguindo a norma culta da língua portuguesa."
    )


def brief_seeds(briefs):
    return [{"id": brief, "row": {"brief": brief}} for brief in briefs]


def tsx_prompt(seed):
    brief = seed["row"]["brief"]
    return (
        f"Write a complete TypeScript React component file for: {brief}\n\n"
        "Return a single .tsx file implementing this component."
    )


def readme_prompt(seed):
    brief = seed["row"]["brief"]
    return (
        f"Write the README for the following small command-line tool:\n\n{brief}\n\n"
        "Produce a complete README.md for the project."
    )


def rust_prompt(seed):
    brief = seed["row"]["brief"]
    return (
        f"Write a complete Rust program for: {brief}\n\n"
        "Return a single .rs source file implementing this."
    )


# Each cell: how to load its seed pool, how to turn one seed into a prompt, and the
# output extension / lang subdirectory the file lands under. Keys are the on-disk
# names bench/score_corpus.py's SYNTH_CELLS expects; self_check's copy checks this.
CELLS = {
    "synth-pt-wiki": {"seeds": pt_wiki_seeds, "prompt": pt_wiki_prompt, "ext": "md", "lang": "prose"},
    "synth-pt-essay": {"seeds": pt_essay_seeds, "prompt": pt_essay_prompt, "ext": "md", "lang": "prose"},
    "synth-tsx": {"seeds": lambda _cache: brief_seeds(TSX_BRIEFS), "prompt": tsx_prompt, "ext": "tsx", "lang": "tsx"},
    "synth-en-readme": {"seeds": lambda _cache: brief_seeds(README_BRIEFS), "prompt": readme_prompt, "ext": "md", "lang": "prose"},
    "synth-rust": {"seeds": lambda _cache: brief_seeds(RUST_BRIEFS), "prompt": rust_prompt, "ext": "rs", "lang": "rust"},
}

# Pre-rename short names kept working as aliases so an existing --cells invocation
# does not break; the on-disk names above are the ones the scorer actually looks for.
CELL_ALIASES = {
    "pt-wiki": "synth-pt-wiki",
    "pt-essay": "synth-pt-essay",
    "tsx": "synth-tsx",
    "en-readme": "synth-en-readme",
    "rust": "synth-rust",
}


def resolve_cells(raw):
    """Parse a comma-separated --cells value into validated on-disk cell names.

    Splits and dedupes before resolving aliases (a rerun should not double-plan a name
    typed twice), resolves each short alias to its on-disk name, dedupes again in case
    an alias and its canonical name were both given, then validates every name against
    CELLS. Raises ValueError naming the bad entry; never raises for an empty string.
    """
    names = list(dict.fromkeys(c for c in raw.split(",") if c))
    resolved = list(dict.fromkeys(CELL_ALIASES.get(c, c) for c in names))
    for name in resolved:
        if name not in CELLS:
            raise ValueError(f"unknown cell {name!r}, choose from {sorted(CELLS)}")
    return resolved


def select_seeds(seeds, limit):
    """First `limit` seeds after a fixed-seed shuffle, so selection is stable across runs."""
    ordered = list(seeds)
    random.Random(SHUFFLE_SEED).shuffle(ordered)
    return ordered[:limit]


def price_for(model):
    if model in PRICE_PER_MTOK:
        return PRICE_PER_MTOK[model]
    print(f"note: no price on file for {model!r}, estimating at claude-opus-5 rates", file=sys.stderr)
    return PRICE_PER_MTOK["claude-opus-5"]


def estimate_cost(n_requests, model, output_tokens=ASSUMED_OUTPUT_TOKENS):
    price_in, price_out = price_for(model)
    return n_requests * (ASSUMED_INPUT_TOKENS / 1e6 * price_in + output_tokens / 1e6 * price_out)


class AuthUnresolved(Exception):
    """A bad key (API 401, anthropic.AuthenticationError) and no key at all raise
    differently -- the zero-arg client only validates lazily, and with nothing to
    resolve at all it raises a bare TypeError from request-header assembly before
    any HTTP call, not a typed SDK exception. Both are the same fatal condition
    from this script's point of view, so call_claude folds them into one type."""


def call_claude(client, model, prompt):
    """One request. Returns {"stop_reason", "text", "served_by", "input_tokens", "output_tokens"}
    or {"error": ...}; raises AuthUnresolved."""
    try:
        response = client.beta.messages.create(
            model=model,
            max_tokens=MAX_TOKENS,
            betas=["server-side-fallback-2026-07-01"],
            fallbacks="default",
            messages=[{"role": "user", "content": prompt}],
        )
    except anthropic.BadRequestError as e:
        return {"error": f"bad request: {e.message}"}
    except anthropic.AuthenticationError as e:
        raise AuthUnresolved(str(e)) from None
    except TypeError as e:
        # ponytail: matching the SDK's own client-side message rather than adding
        # a preflight `ant auth status` subprocess call. Upgrade path: if this
        # string ever changes upstream, add that preflight check instead.
        if "authentication method" not in str(e):
            raise
        raise AuthUnresolved(str(e)) from None
    except anthropic.PermissionDeniedError as e:
        return {"error": f"permission denied: {e.message}"}
    except anthropic.NotFoundError as e:
        return {"error": f"model not found: {e.message}"}
    except anthropic.RateLimitError as e:
        return {"error": f"rate limited: {e.message}"}
    except anthropic.APIStatusError as e:
        return {"error": f"api error {e.status_code}: {e.message}"}
    except anthropic.APIConnectionError as e:
        return {"error": f"connection error: {e}"}
    text = "".join(block.text for block in response.content if block.type == "text")
    # `usage` is absent on the fake client self_check exercises, never on a real response.
    usage = getattr(response, "usage", None)
    return {
        "stop_reason": response.stop_reason,
        "text": text,
        "served_by": response.model,
        "input_tokens": getattr(usage, "input_tokens", None),
        "output_tokens": getattr(usage, "output_tokens", None),
    }


AUTH_HELP = "no Anthropic credentials found -- run `ant auth login` or export ANTHROPIC_API_KEY"


def plan_cell(cell_name, cfg, cache_dir, out_dir, limit):
    """Selected seeds paired with their target filename and any already-recorded index entry."""
    seeds = select_seeds(cfg["seeds"](cache_dir), limit)
    cell_dir = os.path.join(out_dir, "synth", cell_name)
    index_path = os.path.join(cell_dir, "index.json")
    index = json.load(open(index_path)) if os.path.exists(index_path) else {}
    plan = []
    for i, seed in enumerate(seeds, start=1):
        rel = f"ai/{cfg['lang']}/{i:05d}.{cfg['ext']}"
        # An entry with only an `error` key recorded a transient failure (rate limit,
        # connection error) and must be retried; only a `stop_reason` (a written file
        # or a refusal) is terminal.
        done = "stop_reason" in index.get(rel, {})
        plan.append({"cell": cell_name, "cfg": cfg, "seed": seed, "rel": rel, "done": done})
    return cell_dir, index, plan


def run_one(client, model, item):
    prompt = item["cfg"]["prompt"](item["seed"])
    try:
        result = call_claude(client, model, prompt)
    except AuthUnresolved:
        return item, {"auth_error": True}
    return item, result


def persist_index(cell_dir, index):
    """Write `index` to `<cell_dir>/index.json`, landing it atomically like `fetch` does,
    so a process killed mid-write never leaves a truncated ledger the next run would parse."""
    os.makedirs(cell_dir, exist_ok=True)
    path = os.path.join(cell_dir, "index.json")
    part = f"{path}.part"
    with open(part, "w", encoding="utf-8") as fh:
        json.dump(index, fh, indent=2, sort_keys=True)
        fh.write("\n")
    os.replace(part, path)


# ponytail: one lock for every cell, not one per cell_dir. apply_result is I/O bound, so
# cross-cell contention costs nothing measurable; split per-cell if that stops holding.
_INDEX_LOCK = threading.Lock()


def apply_result(cell_dir, index, item, result, model, counts):
    """Apply one item's result to `index` and `counts`, and persist `index.json` before
    returning -- a crash right after this call still has the result on disk, paid-for
    results are never lost to a later exception in the run loop."""
    rel = item["rel"]
    with _INDEX_LOCK:
        if "error" in result:
            index[rel] = {"generator": model, "seed": item["seed"]["id"], "error": result["error"]}
            counts["error"] += 1
            persist_index(cell_dir, index)
            return
        entry = {"generator": model, "seed": item["seed"]["id"], "stop_reason": result["stop_reason"]}
        if result["stop_reason"] == "refusal":
            index[rel] = entry
            counts["refused"] += 1
            persist_index(cell_dir, index)
            return
        entry["served_by"] = result["served_by"]
        if result.get("input_tokens") is not None:
            entry["input_tokens"] = result["input_tokens"]
            counts["input_tokens"] += result["input_tokens"]
        if result.get("output_tokens") is not None:
            entry["output_tokens"] = result["output_tokens"]
            counts["output_tokens"] += result["output_tokens"]
        index[rel] = entry
        path = os.path.join(cell_dir, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(result["text"])
        persist_index(cell_dir, index)
        counts["written"] += 1
        if result["stop_reason"] == "max_tokens":
            counts["truncated"] += 1


def confirm(n_pending, model, auto_yes):
    # A flat per-request guess under-counts a long request: print a range instead, floor
    # from the assumed average, ceiling from `max_tokens` (MAX_TOKENS) actually sent.
    floor = estimate_cost(n_pending, model)
    ceiling = estimate_cost(n_pending, model, MAX_TOKENS)
    print(f"about to make {n_pending} request(s), estimated cost ${floor:.4f} (assumed, "
          f"{ASSUMED_INPUT_TOKENS} input / {ASSUMED_OUTPUT_TOKENS} output tokens per request) "
          f"to ${ceiling:.4f} (ceiling, max_tokens={MAX_TOKENS} output tokens per request)")
    if n_pending == 0:
        return True
    if auto_yes:
        return True
    if not sys.stdin.isatty():
        print("refusing to spend money non-interactively without --yes", file=sys.stderr)
        return False
    return input("proceed? [y/N] ").strip().lower() == "y"


def run(args):
    cache_dir = os.path.join(args.dir, "cache", "seeds")
    plans = []
    for cell_name in args.cells:
        cell_dir, index, plan = plan_cell(cell_name, CELLS[cell_name], cache_dir, args.dir, args.limit)
        plans.append((cell_name, cell_dir, index, plan))

    pending = [(cell_dir, index, item) for _cell, cell_dir, index, plan in plans for item in plan if not item["done"]]
    if not confirm(len(pending), args.model, args.yes):
        return 1
    if not pending:
        for cell_name, _cell_dir, _index, plan in plans:
            print(f"{cell_name}: {len(plan)} selected, 0 pending (already done)")
        return 0

    client = anthropic.Anthropic()
    counts_by_cell = {
        cell_name: {"written": 0, "refused": 0, "truncated": 0, "error": 0, "input_tokens": 0, "output_tokens": 0}
        for cell_name, *_ in plans
    }
    cell_dir_by_name = {cell_name: cell_dir for cell_name, cell_dir, _index, _plan in plans}
    index_by_name = {cell_name: index for cell_name, _cell_dir, index, _plan in plans}

    # apply_result persists index.json per result now; this try/finally only guarantees the
    # summary line still prints, even on a raise (KeyboardInterrupt included), below.
    try:
        # The first request doubles as an auth probe: run it synchronously so a bad
        # credential fails fast with one message instead of after --workers failures.
        cell_dir0, index0, item0 = pending[0]
        _, result0 = run_one(client, args.model, item0)
        if result0.get("auth_error"):
            print(f"error: {AUTH_HELP}", file=sys.stderr)
            return 2
        apply_result(cell_dir0, index0, item0, result0, args.model, counts_by_cell[item0["cell"]])

        rest = pending[1:]
        # No `with`: the executor's __exit__ joins every worker, so Ctrl-C would still wait
        # for each in-flight request to finish before this function could return.
        pool = concurrent.futures.ThreadPoolExecutor(max_workers=args.workers)
        futures = [pool.submit(run_one, client, args.model, item) for _cd, _idx, item in rest]
        try:
            for future in concurrent.futures.as_completed(futures):
                item, result = future.result()
                if result.get("auth_error"):
                    print(f"error: {AUTH_HELP}", file=sys.stderr)
                    pool.shutdown(wait=False, cancel_futures=True)
                    return 2
                apply_result(cell_dir_by_name[item["cell"]], index_by_name[item["cell"]], item, result, args.model, counts_by_cell[item["cell"]])
        except KeyboardInterrupt:
            # cancel_futures only drops futures that had not started yet; a request already
            # in flight keeps running on the API and is still billed. main() exits with
            # os._exit so the interpreter does not join those workers either.
            pool.shutdown(wait=False, cancel_futures=True)
            raise
        pool.shutdown(wait=True)
    finally:
        for cell_name, *_rest in plans:
            c = counts_by_cell[cell_name]
            print(f"{cell_name}: written {c['written']}, refused {c['refused']}, truncated {c['truncated']}, errored {c['error']}")
        total_in = sum(c["input_tokens"] for c in counts_by_cell.values())
        total_out = sum(c["output_tokens"] for c in counts_by_cell.values())
        if total_in or total_out:
            print(f"measured usage: {total_in} input tokens, {total_out} output tokens across this run")
    return 0


def self_check():
    """No network, no API calls: seed-selection determinism, cost arithmetic, index.json
    shape, write-through persistence, retry-on-error planning, --cells dedupe, and the
    on-disk contract bench/score_corpus.py's SYNTH_CELLS expects."""

    # 1. Seed selection is deterministic and actually permutes a 60-item pool.
    a = select_seeds(brief_seeds(TSX_BRIEFS), 60)
    b = select_seeds(brief_seeds(TSX_BRIEFS), 60)
    assert [s["id"] for s in a] == [s["id"] for s in b], "same seed pool + same SHUFFLE_SEED must select identically"
    assert [s["id"] for s in a] != [s["row"]["brief"] for s in brief_seeds(TSX_BRIEFS)], "selection should not equal input order"
    assert len(select_seeds(brief_seeds(TSX_BRIEFS), 5)) == 5
    assert len(RUST_BRIEFS) == 60, f"synth-rust wants 60 distinct briefs, has {len(RUST_BRIEFS)}"
    assert len(set(RUST_BRIEFS)) == len(RUST_BRIEFS), "RUST_BRIEFS has a duplicate"

    # 2. Cost estimate arithmetic, computed by hand against the same constants.
    got = estimate_cost(10, "claude-opus-5")
    want = 10 * (200 / 1e6 * 5.0 + 800 / 1e6 * 25.0)
    assert abs(got - want) < 1e-12, (got, want)
    assert estimate_cost(0, "claude-opus-5") == 0.0

    # 3. index.json shape from a fake client cycling refusal / max_tokens / end_turn, and
    #    write-through: index.json on disk matches in-memory state after every applied
    #    result, not only at the end of a run.
    class FakeBlock:
        def __init__(self, text):
            self.type = "text"
            self.text = text

    class FakeResponse:
        def __init__(self, stop_reason, text, model):
            self.stop_reason = stop_reason
            self.content = [] if stop_reason == "refusal" else [FakeBlock(text)]
            self.model = model

    class FakeMessages:
        def __init__(self):
            self.calls = 0

        def create(self, **_kwargs):
            self.calls += 1
            cycle = ["end_turn", "refusal", "max_tokens"][(self.calls - 1) % 3]
            return FakeResponse(cycle, f"body {self.calls}", "claude-opus-5-fake")

    class FakeBeta:
        def __init__(self):
            self.messages = FakeMessages()

    class FakeClient:
        def __init__(self):
            self.beta = FakeBeta()

    import tempfile

    def index_on_disk(cell_dir):
        with open(os.path.join(cell_dir, "index.json"), encoding="utf-8") as fh:
            return json.load(fh)

    with tempfile.TemporaryDirectory() as tmp:
        client = FakeClient()
        cfg = CELLS["synth-en-readme"]
        seeds = select_seeds(brief_seeds(README_BRIEFS), 3)
        plan = [{"cell": "synth-en-readme", "cfg": cfg, "seed": s, "rel": f"ai/prose/{i:05d}.md", "done": False} for i, s in enumerate(seeds, start=1)]
        cell_dir = os.path.join(tmp, "synth", "synth-en-readme")
        index = {}
        counts = {"written": 0, "refused": 0, "truncated": 0, "error": 0, "input_tokens": 0, "output_tokens": 0}
        for item in plan:
            _, result = run_one(client, "claude-opus-5", item)
            apply_result(cell_dir, index, item, result, "claude-opus-5", counts)
            assert index_on_disk(cell_dir) == index, "index.json on disk must match in-memory state after every applied result"

        # FakeResponse carries no `usage`, so the token counters stay at 0 here; the shape
        # match still catches a counts dict that drifts out of sync with apply_result.
        assert counts == {"written": 2, "refused": 1, "truncated": 1, "error": 0, "input_tokens": 0, "output_tokens": 0}, counts
        assert len(index) == 3
        for rel, entry in index.items():
            assert set(entry) >= {"generator", "seed", "stop_reason"}, entry
            if entry["stop_reason"] == "refusal":
                assert not os.path.exists(os.path.join(cell_dir, rel))
            else:
                assert entry["served_by"] == "claude-opus-5-fake"
                assert os.path.exists(os.path.join(cell_dir, rel))

        # A crash right after the last applied result must not lose it: apply_result
        # already wrote it to disk before this exception is ever raised.
        before = index_on_disk(cell_dir)
        crashed = False
        try:
            apply_result(cell_dir, index, plan[0], {"error": "connection error: boom"}, "claude-opus-5", counts)
            raise RuntimeError("simulated crash right after an applied result")
        except RuntimeError:
            crashed = True
        assert crashed, "the simulated crash should have propagated past apply_result"
        after = index_on_disk(cell_dir)
        assert after != before, "the simulated error result should have changed the ledger"
        assert after[plan[0]["rel"]]["error"] == "connection error: boom"

    # 4. plan_cell retries an error-only entry (transient failure) but treats an entry
    #    carrying a stop_reason (written file or refusal) as terminal.
    with tempfile.TemporaryDirectory() as tmp:
        rel0 = "ai/prose/00001.md"
        cell_dir = os.path.join(tmp, "synth", "synth-en-readme")
        os.makedirs(cell_dir, exist_ok=True)
        with open(os.path.join(cell_dir, "index.json"), "w", encoding="utf-8") as fh:
            json.dump({rel0: {"generator": "m", "seed": "x", "error": "rate limited: slow down"}}, fh)
        _cd, _idx, plan = plan_cell("synth-en-readme", CELLS["synth-en-readme"], tmp, tmp, 1)
        assert plan[0]["rel"] == rel0
        assert plan[0]["done"] is False, "an error-only index entry must be retried, not treated as done"

        with open(os.path.join(cell_dir, "index.json"), "w", encoding="utf-8") as fh:
            json.dump({rel0: {"generator": "m", "seed": "x", "stop_reason": "refusal"}}, fh)
        _cd, _idx, plan = plan_cell("synth-en-readme", CELLS["synth-en-readme"], tmp, tmp, 1)
        assert plan[0]["done"] is True, "an entry with a stop_reason is terminal"

    # 5. --cells dedupes (including an alias colliding with its canonical name) before
    #    validating, and rejects an unknown name.
    assert resolve_cells("tsx,tsx,synth-tsx") == ["synth-tsx"]
    assert resolve_cells("pt-wiki,en-readme,pt-wiki") == ["synth-pt-wiki", "synth-en-readme"]
    rejected = False
    try:
        resolve_cells("bogus")
    except ValueError:
        rejected = True
    assert rejected, "resolve_cells must reject an unknown cell name"

    # 6. bench/score_corpus.py's SYNTH_CELLS is the authority for the on-disk contract
    # (name -> lang subdirectory). Imported only here so the normal run path still needs
    # nothing beyond this file's own `anthropic` dependency to stay a standalone script.
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import score_corpus

    assert set(CELLS) == set(score_corpus.SYNTH_CELLS), "CELLS keys must equal bench/score_corpus.py's SYNTH_CELLS names"
    for name, spec in score_corpus.SYNTH_CELLS.items():
        assert CELLS[name]["lang"] == spec["lang"], f"{name}: lang must be {spec['lang']!r} to match SYNTH_CELLS"

    print("self-check ok")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cells", default=",".join(CELLS), help="comma-separated cell names")
    parser.add_argument("--limit", type=int, default=200, help="max files per cell")
    parser.add_argument("--dir", default="target/corpus", help="output root")
    parser.add_argument("--workers", type=int, default=4, help="concurrent API requests")
    parser.add_argument("--model", default="claude-opus-5", help="Anthropic model id to generate with")
    parser.add_argument("--yes", action="store_true", help="skip the confirmation prompt")
    parser.add_argument("--self-check", action="store_true", help="run offline self-checks and exit")
    args = parser.parse_args()

    if args.self_check:
        self_check()
        return 0

    try:
        args.cells = resolve_cells(args.cells)
    except ValueError as e:
        parser.error(str(e))
    try:
        return run(args)
    except KeyboardInterrupt:
        # Every applied result is already on disk (write-through index.json); the only
        # thing left running is in-flight requests, which a normal exit would wait for.
        print("interrupted; in-flight requests still finish and are billed", file=sys.stderr)
        sys.stderr.flush()
        os._exit(130)


if __name__ == "__main__":
    sys.exit(main())
