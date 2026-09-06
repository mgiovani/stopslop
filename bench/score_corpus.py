"""Per-rule hit rate for the registered rules across a labelled human-vs-AI corpus registry.

usage: python3 bench/score_corpus.py --fetch-only --skip-failures   # network: materialize cells
       python3 bench/score_corpus.py --no-fetch --report bench/corpus_report.md --json target/corpus/results.json
       python3 bench/score_corpus.py [options] > report.md            # both steps in one run
       python3 bench/score_corpus.py --self-check

Each registered dataset in `DATASETS` names a `kind` (how to fetch it), a natural language
axis, its source URL/license, known caveats, and the `cells` it contributes -- one cell per
(label, lang) pair actually lintable. Every fetch is cached under `<dir>/cache/<dataset>/` as
the raw page/file the network returned, so a rerun never touches the network and reproduces
byte-identical class directories; `<dir>/<dataset>/<label>/<lang>/` is rebuilt from that cache
every fetch (`rmtree` first, as the original AIGCodeSet-only version of this script did), one
`NNNNN.<ext>` file per sample plus an `index.json` mapping filename to `{generator, words,
...}`. `--fetch-only` stops after writing the cells and a `fetched.json` manifest;
`--no-fetch` scores whatever that manifest lists without touching the network, so the two
halves can run as separate `just` recipes. Paste the relevant table into a PR body; this is a
measurement tool, not a gate, so CI never runs it and the report defaults to stdout.

The corpus lands under `target/` with numbered filenames on purpose. `paths::is_test_path`
skips the nine path-gated rules for any path holding a segment such as `tests`, `fixtures` or
`examples`, or a filename starting `test_`; `target/corpus/<dataset>/human/python/00001.py`
trips none of that. `target/` being gitignored costs nothing here, because the walk
ignore-filters only below an explicit root, and it keeps the corpus out of the dogfood run.

A human split counts as human only when its text is dated 2019 or earlier (`pinned-2019`) or
was written by identified people under controlled conditions (`verified-authors`). Anything
collected once coding assistants and chat models were in general use is `unverified`: it is
still fetched, scored and reported, but below a divider, and the pooled human rates that
feed lift and precision use the verified splits only. Every dataset with a human cell
declares `human_provenance` and a one-line `human_note`; `human_domains` restricts a
multi-source human split to its pre-2020 sources.

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
import argparse
import ast
import csv
import fnmatch
import hashlib
import json
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile

HF_FILTER_URL = "https://datasets-server.huggingface.co/filter"
HF_ROWS_URL = "https://datasets-server.huggingface.co/rows"
HF_API_URL = "https://huggingface.co/api/datasets"
# datasets-server builds its DuckDB index lazily and answers 500 meanwhile; a six-step ladder
# (~3 minutes) lost nine Droid and CodeMirage cells in one 500-per-cell run on 2026-09-05.
RETRY_SLEEPS = (5, 10, 20, 40, 60, 60, 90, 120, 120, 120)
# `/filter` gets a short ladder because `/rows` (no server-side index) is the fallback; on
# 2026-09-05 the Droid and CodeMirage indexes stayed "loading" for over an hour.
FILTER_RETRY_SLEEPS = (5, 10, 20)
# Pages a `/rows` scan may read before giving up on a cell: 600 pages = 60k rows, enough
# for 500 rows of a 2% cell on a shuffled dataset; a dataset sorted by language yields less.
SCAN_MAX_PAGES = 600
# Windows a scan must spread a common cell over before it may stop (see `hf_rows_scan`).
SCAN_MIN_PAGES = 50
# Judgment call, not calibrated against real precision/recall data: one more finding moves a
# ratio built from single-digit counts a long way, so rows/cells under this many hits are
# marked and read off their raw counts instead.
LOW_SUPPORT = 20
# Judgment call, not calibrated: per-generator table, a generator with fewer files than this
# reads as noise, not a measurement.
MIN_GENERATOR_FILES = 20
# Flagged lines kept per rule per cell in results.json, spread across the flagged files; the
# HTML page shows them side by side, so more than a screen's worth per split is unread.
EXAMPLES_PER_RULE = 14
# Judgment call, not calibrated: enough lines each side of a flagged line to show the
# enclosing statement in the HTML example card without approaching a whole file.
SNIPPET_CONTEXT = 2
# Judgment call, not calibrated: long enough that clip() rarely needs to cut a normal source
# line, short enough that one long line does not dominate an HTML example card.
LINE_MAX = 230
MANIFEST = "fetched.json"
RESULTS_SCHEMA = 1
VERIFIED = ("pinned-2019", "verified-authors")
PROVENANCE = VERIFIED + ("unverified",)
PROVENANCE_NOTE = (
    "The splits below were collected after coding assistants and chat models came into "
    "general use, so nobody can guarantee their human side was written without one unless "
    "the dataset's authors verified it. They are fetched, scored and read exactly like the "
    "splits above, but they stay out of the pooled human rates, the lift and precision "
    "columns, and the takeaways."
)
# Rules whose backticked span is a library API, not a user identifier: keep it, it is the
# panel entry. Collapsing it elsewhere is what keeps the breakdown to one row per message.
KEEP_BACKTICKS = ("SLOP037", "SLOP038")

EXT = {"python": "py", "go": "go", "rust": "rs", "typescript": "ts", "tsx": "tsx", "prose": "md"}
JS_PROXY = "javascript written as .ts; SLOP007 cannot fire"

# Applicable rules per lang, hand-transcribed from each RuleDef.langs in src/rules/*.rs.
# SLOP010 needs a manifest no cell carries, so it never fires; left out on purpose.
# ponytail: add a langs column to `--list-rules`, read it here once that exists.
APPLICABLE = {
    "python": (
        "SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP006", "SLOP008",
        "SLOP009", "SLOP037", "SLOP039", "SLOP040", "SLOP042", "SLOP043",
    ),
    "go": (
        "SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP005", "SLOP008",
        "SLOP009", "SLOP037", "SLOP039", "SLOP042", "SLOP043",
    ),
    "rust": (
        "SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP005", "SLOP008",
        "SLOP009", "SLOP037", "SLOP039", "SLOP042", "SLOP043",
    ),
    "typescript": (
        "SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP005", "SLOP007", "SLOP008",
        "SLOP009", "SLOP037", "SLOP038", "SLOP039", "SLOP040", "SLOP042", "SLOP043",
    ),
    "tsx": (
        "SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP005", "SLOP007", "SLOP008",
        "SLOP009", "SLOP037", "SLOP038", "SLOP039", "SLOP040", "SLOP042", "SLOP043",
    ),
    "prose": (
        "SLOP011", "SLOP012", "SLOP013", "SLOP014", "SLOP015", "SLOP016", "SLOP017",
        "SLOP018", "SLOP019", "SLOP020", "SLOP021", "SLOP022", "SLOP023", "SLOP024",
        "SLOP025", "SLOP026", "SLOP027", "SLOP028", "SLOP029", "SLOP030", "SLOP031",
        "SLOP032", "SLOP033", "SLOP034", "SLOP035", "SLOP036", "SLOP041",
    ),
}
UNSCOREABLE_NOTE = (
    "SLOP010 names every code `Lang` too, but its check needs a dependency manifest a "
    "per-file corpus cell has no reason to carry, so it can never fire here and the "
    "tables leave it out rather than show a rule that structurally cannot score."
)


def sample_indices(n_total, limit):
    """Evenly spaced positions across `[0, n_total)`; `limit` falsy or `>= n_total` means all.

    `i * n_total // limit` is strictly increasing for `0 <= i < limit <= n_total`, so this
    always returns exactly `limit` distinct, ascending indices spread over the whole range --
    a deterministic spread sample, never a head slice.
    """
    if not limit or limit >= n_total:
        return list(range(n_total))
    return [i * n_total // limit for i in range(limit)]


def spread_pair(human_items, ai_items, limit):
    """{"human": ..., "ai": ...}, each independently spread-sampled to `limit` -- the shared
    tail of every fetcher that splits one source into a human list and an ai list."""
    return {
        "human": [human_items[i] for i in sample_indices(len(human_items), limit)],
        "ai": [ai_items[i] for i in sample_indices(len(ai_items), limit)],
    }


def fetch(url, dest, what, skip_failures, headers=None):
    """Download once with `hf_get_cached`'s retry ladder, and land the file atomically.

    A download killed halfway leaves a truncated file the next run would treat as cached, then
    report a smaller corpus with nothing to say why. Retries a connection error or 5xx on
    `RETRY_SLEEPS`; a non-retryable 4xx, or exhausted retries, is a hard `SystemExit` naming
    `what` unless `skip_failures`, which returns `None` instead so the caller can skip this
    item. `headers` forwards to the request -- pass `hf_headers()` for a Hugging Face resolve
    URL, since a gated dataset needs the same bearer token there as on `/rows` and `/filter`.
    """
    if os.path.exists(dest):
        return dest
    os.makedirs(os.path.dirname(dest) or ".", exist_ok=True)
    last_err = None
    for attempt, delay in enumerate((0,) + RETRY_SLEEPS):
        if delay:
            time.sleep(delay)
        part = f"{dest}.part"
        try:
            req = urllib.request.Request(url, headers=headers or {})
            # `fetch` streams a whole file (csv/jsonl/json resolves up to tens of MB); a
            # page-sized API response would time out well before this, so the longer bound
            # only bites a genuine stall, not a slow-but-live transfer.
            with urllib.request.urlopen(req, timeout=120) as resp, open(part, "wb") as out:
                shutil.copyfileobj(resp, out)
        except urllib.error.HTTPError as e:
            last_err = f"HTTP {e.code}"
            if e.code >= 500:
                continue
            break  # not retryable: a real 4xx means the URL itself is wrong
        except (urllib.error.URLError, TimeoutError) as e:
            last_err = f"connection error: {getattr(e, 'reason', e)}"
            continue
        os.replace(part, dest)
        return dest
    if skip_failures:
        return None
    raise SystemExit(f"{what}: giving up: {last_err}")


def hf_headers():
    token = os.environ.get("HF_TOKEN") or os.environ.get("HF_API_TOKEN")
    return {"Authorization": f"Bearer {token}"} if token else {}


def _looks_retryable(body):
    """Any `{"error": ...}` body is transient server state, never data.

    datasets-server only puts an `error` key on a failed response, and the wording varies
    ("index is loading", "Unexpected error", "Authentication check ... timed out"), so
    matching phrases missed real cases; caching such a body as a page would crash the next
    run on a missing `rows` key.
    """
    try:
        return bool(json.loads(body).get("error"))
    except (ValueError, AttributeError):
        # ValueError covers json.JSONDecodeError (malformed JSON) and UnicodeDecodeError (a
        # non-UTF-8 body); AttributeError covers valid JSON with no `.get`, e.g. a bare list.
        return False


def hf_get_cached(url, cache_path, what, skip_failures, sleeps=RETRY_SLEEPS):
    """GET `url` as JSON, cached at `cache_path` so a rerun never touches the network.

    Retries a 5xx, or a 200/500 body shaped like `{"error": "...index is loading..."}` /
    `"...Unexpected error..."`, on `RETRY_SLEEPS`. A non-retryable 4xx, or exhausted retries,
    is a hard `SystemExit` naming `what` unless `skip_failures`, which returns `None` instead
    so the caller can mark that one cell "not fetched" and carry on.
    """
    if os.path.exists(cache_path):
        with open(cache_path, encoding="utf-8") as fh:
            return json.load(fh)
    last_err = None
    for attempt, delay in enumerate((0,) + tuple(sleeps)):
        if delay:
            time.sleep(delay)
        try:
            req = urllib.request.Request(url, headers=hf_headers())
            # `/rows` and `/filter` answer with at most 100 rows of JSON; a real answer lands
            # in seconds, so a shorter bound than fetch()'s file download surfaces a stalled
            # request sooner, before the retry ladder below even starts.
            with urllib.request.urlopen(req, timeout=60) as resp:
                body, status = resp.read(), resp.status
        except urllib.error.HTTPError as e:
            body, status = e.read(), e.code
        except (urllib.error.URLError, TimeoutError) as e:
            # A read (not connect) timeout surfaces as a bare `TimeoutError`, not wrapped in
            # `URLError` -- datasets-server can sit on a slow query past the socket timeout
            # without ever answering, same failure mode as the "index is loading" 500s.
            last_err = f"connection error: {getattr(e, 'reason', e)}"
            continue
        if status >= 500 or _looks_retryable(body):
            last_err = f"HTTP {status}: {body[:200]!r}"
            continue
        if status >= 400:
            last_err = f"HTTP {status}: {body[:200]!r}"
            break  # not retryable: a real 4xx means the query itself is wrong
        os.makedirs(os.path.dirname(cache_path) or ".", exist_ok=True)
        part = f"{cache_path}.part"
        with open(part, "wb") as fh:
            fh.write(body)
        os.replace(part, cache_path)
        return json.loads(body)
    if skip_failures:
        return None
    raise SystemExit(f"{what}: giving up: {last_err}")


def _slug(text):
    return hashlib.sha1(text.encode("utf-8")).hexdigest()[:16]


def spread_page_offsets(total, pages):
    """`pages` non-overlapping `(offset, length)` windows into `[0, total)`, each up to 100 rows.

    Centers every window in an equal share of the range -- `share = total / pages`, `offset =
    int((i + 0.5) * share) - 50` clamped to `[0, total - 100]` -- so a spread sample reaches
    across the whole range even from a single page (`pages == 1` previously fell back to
    offset 0, the literal head, contradicting every caller's "spread sample" docstring). A
    share narrower than a 100-row window (`total < 100 * pages`, which is always true once
    `pages` is enough to cover all of `total`) has no room to center a window without
    spilling into its neighbor, so this falls back to the share itself as the window; the
    shares still partition `[0, total)` exactly, with no gap and no overlap.
    """
    if pages <= 0 or total <= 0:
        return []
    share = total / pages
    if total >= 100 * pages:
        return [(min(max(0, int((i + 0.5) * share) - 50), total - 100), 100) for i in range(pages)]
    bounds = [int(round(i * share)) for i in range(pages + 1)]
    bounds[-1] = total
    return [(bounds[i], bounds[i + 1] - bounds[i]) for i in range(pages)]


def _windows_overlap(windows):
    ordered = sorted(windows)
    return any(a[0] + a[1] > b[0] for a, b in zip(ordered, ordered[1:]))


def _paged_rows(call_page, total, want, skip_failures):
    """Fetch `ceil(want/100)` pages at `spread_page_offsets(total, pages)`, dedupe by
    `row_idx`, and slice to `want`.

    A window landing on a share boundary can round to a row its neighbor already covered even
    when the windows themselves don't overlap, and a duplicate row becomes a duplicate file in
    the materialized cell -- dedupe before the slice, not after, so `want` counts unique rows.
    """
    # ponytail: every returned row lives in memory at once, so `--limit 0` buffers a whole
    # cell before `write_cell` ever touches disk; stream pages straight into `write_cell` if a
    # future dataset makes a full cell too big to hold at once.
    pages = max(1, math.ceil(want / 100))
    seen, rows = set(), []
    for offset, length in spread_page_offsets(total, pages):
        page = call_page(offset, length)
        if page is None:
            return None
        for r in page["rows"]:
            idx = r.get("row_idx")
            if idx in seen:
                continue
            seen.add(idx)
            rows.append(r["row"])
    return rows[:want]


def hf_filter_sample(dataset, config, split, where, limit, cache_dir, what, skip_failures):
    """Spread-sample up to `limit` rows matching `where` via `/filter`.

    `/filter` caps a single page at 100 rows, so this issues `ceil(limit/100)` pages at the
    non-overlapping offsets `spread_page_offsets` centers across the whole matching range --
    `--limit` samples across the dataset instead of reading its head, even from a single page.
    Returns `None` (not raising) when `skip_failures` and every retry on the probe or a page is
    exhausted, or when the probe matches 0 rows.
    """
    def call(offset, length):
        params = {"dataset": dataset, "config": config, "split": split, "where": where,
                  "offset": offset, "length": length}
        url = f"{HF_FILTER_URL}?{urllib.parse.urlencode(params)}"
        # `length` belongs in the cache key: the length=1 probe and a length=100 page both
        # start at offset 0 on one page, and without it the probe's 1-row body would shadow
        # the real page on disk.
        cache_path = os.path.join(cache_dir, f"filter_{_slug(where)}_{offset}_{length}.json")
        return hf_get_cached(url, cache_path, f"{what} (offset {offset})", True, FILTER_RETRY_SLEEPS)

    # A scanned cell stays scanned: the `/filter` index comes and goes between runs and the
    # two paths draw different rows. Delete the marker to let the cell try `/filter` again.
    if os.path.exists(scan_marker(cache_dir, where)):
        return hf_rows_scan(dataset, config, split, where, limit, cache_dir, what, skip_failures)
    probe = call(0, 1)
    if probe is None:
        return hf_rows_scan(dataset, config, split, where, limit, cache_dir, what, skip_failures)
    total = probe["num_rows_total"]
    if total == 0:
        if skip_failures:
            return None
        raise SystemExit(f"{what}: where={where!r} matched 0 rows on the probe call")
    want = total if not limit else min(limit, total)
    rows = _paged_rows(call, total, want, True)
    if rows is None:
        return hf_rows_scan(dataset, config, split, where, want, cache_dir, what, skip_failures)
    return rows


def scan_marker(cache_dir, where):
    return os.path.join(cache_dir, f"scan_{_slug(where)}")


WHERE_TERM = re.compile(r'"(\w+)"\s*(=|<>)\s*(?:\'([^\']*)\'|(-?\d+))')


def where_predicate(where):
    """Row predicate for the `and`-joined `"col"='v'` / `"col"<>'v'` / `"col"=1` clauses the
    registry writes; that is the whole grammar `/filter` sees from this file, so nothing else
    parses. Values compare as strings because `/rows` may type MAGE's `label` either way."""
    terms = WHERE_TERM.findall(where)
    if not terms or len(terms) != where.lower().count(" and ") + 1:
        raise SystemExit(f"where clause is not scannable: {where!r}")
    wanted = [(c, op, s if num == "" else num) for c, op, s, num in terms]
    return lambda row: all(
        (str(row.get(c)) == v) if op == "=" else (str(row.get(c)) != v) for c, op, v in wanted
    )


def scan_order(pages):
    """Visit spread windows in golden-ratio order so an early stop still covers the range.

    Walking the windows front to back and stopping at `want` would sample the dataset's head
    whenever the cell is common; this permutation is deterministic and low-discrepancy.
    """
    return sorted(range(pages), key=lambda i: ((i + 1) * 0.6180339887498949) % 1.0)


def hf_rows_scan(dataset, config, split, where, want, cache_dir, what, skip_failures):
    """Fallback for a `/filter` index that never finishes loading: page through `/rows`, which
    needs no index, at spread offsets and keep the rows `where` accepts until `want`.

    Pages are cached per dataset, not per cell, so every cell of one dataset shares one scan.
    A cell rarer than `want / (100 * SCAN_MAX_PAGES)` comes back short and says so on stderr.
    At most `want / SCAN_MIN_PAGES` rows are kept per page so a common cell still draws from
    many windows: MAGE stores whole `src` blocks contiguously, and a head-only sample of its
    human split read 54% SLOP030 where the spread sample reads 23%.
    """
    keep = where_predicate(where)
    probe = hf_rows_page(dataset, config, split, 0, 1, cache_dir, what, skip_failures)
    if probe is None:
        return None
    total = probe["num_rows_total"]
    sys.stderr.write(f"{what}: /filter unavailable, scanning /rows\n")
    windows = spread_page_offsets(total, min(SCAN_MAX_PAGES, math.ceil(total / 100)))
    per_page = max(1, math.ceil(want / SCAN_MIN_PAGES)) if want else 100
    rows, seen = [], set()
    for i in scan_order(len(windows)):
        offset, length = windows[i]
        page = hf_rows_page(dataset, config, split, offset, length, cache_dir, what, skip_failures)
        if page is None:
            return None
        taken = 0
        for r in page["rows"]:
            if r["row_idx"] in seen or taken >= per_page:
                continue
            seen.add(r["row_idx"])
            if keep(r["row"]):
                rows.append(r["row"])
                taken += 1
        if want and len(rows) >= want:
            break
    if want and len(rows) < want:
        sys.stderr.write(f"{what}: scan found {len(rows)} of {want} rows in {len(windows)} pages\n")
    if not rows:
        if skip_failures:
            return None
        raise SystemExit(f"{what}: scan found no rows for where={where!r}")
    with open(scan_marker(cache_dir, where), "w", encoding="utf-8") as fh:
        fh.write(where)
    return rows[:want] if want else rows


def hf_rows_page(dataset, config, split, offset, length, cache_dir, what, skip_failures):
    params = {"dataset": dataset, "config": config, "split": split, "offset": offset, "length": length}
    url = f"{HF_ROWS_URL}?{urllib.parse.urlencode(params)}"
    # See `hf_filter_sample.call`: `length` and `split` must both be in the cache key, or a
    # length=1 probe, or a page from a different split, can shadow the real page at offset 0.
    cache_path = os.path.join(cache_dir, f"rows_{config}_{split}_{offset}_{length}.json")
    return hf_get_cached(url, cache_path, f"{what} (offset {offset})", skip_failures)


def hf_rows_sample(dataset, config, split, limit, cache_dir, what, skip_failures):
    """Spread-sample up to `limit` raw rows via `/rows` (no `where`); caller flattens each row."""
    probe = hf_rows_page(dataset, config, split, 0, 1, cache_dir, what, skip_failures)
    if probe is None:
        return None
    total = probe["num_rows_total"]
    if total == 0:
        return []
    want = total if not limit else min(limit, total)
    return _paged_rows(
        lambda offset, length: hf_rows_page(dataset, config, split, offset, length, cache_dir, what, skip_failures),
        total, want, skip_failures,
    )


def hf_revision(dataset_id, cache_dir, skip_failures):
    cache_path = os.path.join(cache_dir, "revision.json")
    data = hf_get_cached(f"{HF_API_URL}/{dataset_id}", cache_path, f"{dataset_id} revision", skip_failures)
    return data["sha"][:12] if data else "unknown (fetch failed)"


def git_clone_ref(repo, ref, dest, what, skip_failures):
    """`git clone --depth 1 [--branch ref]` once, landed atomically.

    A cache hit (`dest/.git` already present) skips the network entirely. Otherwise this
    clones into `dest + '.part'` and only `os.replace`s it onto `dest` once `git clone` exits
    0, so a clone killed halfway never leaves a `.git` directory the next run's cache check
    would mistake for a complete one; a leftover `.part` from an earlier kill is removed and
    redone rather than reused.
    """
    if os.path.isdir(os.path.join(dest, ".git")):
        return dest
    part = f"{dest}.part"
    shutil.rmtree(part, ignore_errors=True)
    os.makedirs(os.path.dirname(dest) or ".", exist_ok=True)
    cmd = ["git", "clone", "--depth", "1", "--quiet"]
    if ref:
        cmd += ["--branch", ref]
    cmd += [repo, part]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        shutil.rmtree(part, ignore_errors=True)
        if skip_failures:
            return None
        raise SystemExit(f"{what}: git clone failed:\n{proc.stderr}")
    os.replace(part, dest)
    return dest


def git_clone_before(repo, cutoff_date, dest, what, skip_failures):
    """Clone the default branch, check out its last commit before `cutoff_date`, and land the
    result atomically.

    Used for a repo with no tag near the wanted date (rust-lang/book): `--shallow-since` one
    year back keeps the clone small while still reaching a commit before `cutoff_date`. Clone
    and checkout both happen inside `dest + '.part'`, moved onto `dest` only once both exit 0,
    matching `git_clone_ref`'s atomicity; a leftover `.part` is removed and redone.
    """
    if os.path.isdir(os.path.join(dest, ".git")):
        return dest
    part = f"{dest}.part"
    shutil.rmtree(part, ignore_errors=True)
    os.makedirs(os.path.dirname(dest) or ".", exist_ok=True)
    since = f"{int(cutoff_date[:4]) - 1}-{cutoff_date[5:]}"
    proc = subprocess.run(
        ["git", "clone", "--quiet", f"--shallow-since={since}", repo, part],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        shutil.rmtree(part, ignore_errors=True)
        if skip_failures:
            return None
        raise SystemExit(f"{what}: git clone failed:\n{proc.stderr}")
    log = subprocess.run(
        ["git", "-C", part, "log", f"--until={cutoff_date}T00:00:00", "-1", "--format=%H"],
        capture_output=True, text=True,
    )
    sha = log.stdout.strip()
    if not sha:
        shutil.rmtree(part, ignore_errors=True)
        if skip_failures:
            return None
        raise SystemExit(f"{what}: no commit before {cutoff_date} in the shallow history")
    checkout = subprocess.run(["git", "-C", part, "checkout", "--quiet", sha], capture_output=True, text=True)
    if checkout.returncode != 0:
        shutil.rmtree(part, ignore_errors=True)
        if skip_failures:
            return None
        raise SystemExit(f"{what}: git checkout failed:\n{checkout.stderr}")
    os.replace(part, dest)
    return dest


def git_head_sha(clone_dir):
    return subprocess.run(
        ["git", "-C", clone_dir, "rev-parse", "--short=12", "HEAD"],
        capture_output=True, text=True,
    ).stdout.strip()


def git_files(clone_dir, subdirs, glob_pat, exclude=()):
    """Every file under `clone_dir/<subdir>` for each of `subdirs` (one string or several)
    matching `glob_pat`, minus any path containing an `exclude` substring, sorted for
    determinism. Several subdirs exist for Rust 1.40, whose std lived in three `src/lib*`
    crates before the `library/` move."""
    if isinstance(subdirs, str):
        subdirs = (subdirs,)
    matches = []
    for subdir in subdirs:
        root = os.path.join(clone_dir, subdir)
        if not os.path.isdir(root):
            continue
        for dirpath, _dirnames, filenames in os.walk(root):
            for name in fnmatch.filter(filenames, glob_pat):
                rel = os.path.relpath(os.path.join(dirpath, name), clone_dir).replace(os.sep, "/")
                if any(x in rel for x in exclude):
                    continue
                matches.append(rel)
    return sorted(matches)


def read_text_files(clone_dir, files, limit):
    idx = sample_indices(len(files), limit)
    items = []
    for i in idx:
        rel = files[i]
        with open(os.path.join(clone_dir, rel), encoding="utf-8", errors="replace") as fh:
            text = fh.read()
        items.append((text, {"generator": "human", "domain": rel}))
    return items


# --------------------------------------------------------------------------------------
# Per-dataset fetchers. Each returns either a flat list of (text, meta) for a single-cell
# dataset, or a dict label -> list[(text, meta)] for a multi-cell one; `None` (or a `None`
# value for one label) means "skipped under --skip-failures", never a crash.
# --------------------------------------------------------------------------------------

def fetch_hf_filter_cell(ds, cell, limit, cache_dir, skip_failures):
    what = f"{ds['name']}/{cell['label']}/{cell['lang']}"
    rows = hf_filter_sample(
        ds["dataset"], ds.get("config", "default"), ds.get("split", "train"),
        cell["where"], limit, cache_dir, what, skip_failures,
    )
    if rows is None:
        return None
    text_col, gen_col = ds["text_col"], ds.get("generator_col")
    gen_literal = cell.get("generator")
    items = []
    for row in rows:
        gen = gen_literal if gen_literal is not None else (row.get(gen_col) or "unknown")
        items.append((row.get(text_col) or "", {"generator": gen}))
    return items


def fetch_aigcodeset(ds, cell, limit, cache_dir, skip_failures):
    what = f"{ds['name']}/{cell['label']}/{cell['lang']}"
    path = fetch(cell["url"], os.path.join(cache_dir, os.path.basename(cell["url"])),
                  what, skip_failures, hf_headers())
    if path is None:
        return None
    rows = []
    with open(path, encoding="utf-8", newline="") as fh:
        for row in csv.DictReader(fh):
            if row["label"] != cell["csv_label"]:
                raise SystemExit(f"{path}: expected label {cell['csv_label']}, saw {row['label']!r}")
            rows.append(row)
    items = []
    for i in sample_indices(len(rows), limit):
        row = rows[i]
        gen = cell.get("generator") or row.get("LLM") or "unknown"
        items.append((row["code"], {"generator": gen, "outcome": row.get("status_in_folder")}))
    return items


def fetch_essay_br(ds, cell, limit, cache_dir, skip_failures):
    what = f"{ds['name']}/{cell['label']}/{cell['lang']}"
    path = fetch(cell["url"], os.path.join(cache_dir, "essay-br.csv"), what, skip_failures)
    if path is None:
        return None
    with open(path, encoding="utf-8", newline="") as fh:
        rows = list(csv.DictReader(fh))
    items, malformed = [], 0
    for i in sample_indices(len(rows), limit):
        row = rows[i]
        try:
            paragraphs = ast.literal_eval(row["essay"])
            text = "\n\n".join(paragraphs)
        except (KeyError, SyntaxError, ValueError, TypeError):
            malformed += 1
            continue
        items.append((text, {"generator": "human", "domain": row.get("score")}))
    if malformed:
        sys.stderr.write(f"{what}: skipped {malformed} malformed row(s)\n")
    return items


def shared_clone_dir(cache_dir, repo, ref):
    """One clone per (repo, ref), shared across every dataset that names it -- cpython-lib
    and cpython-doc name the same tag of the same repo and would otherwise pay for (and
    store) the same ~120 MB clone twice."""
    git_cache = os.path.join(os.path.dirname(cache_dir), "_git")
    return os.path.join(git_cache, _slug(f"{repo}@{ref or ''}"))


def materialize_from_clone(ds, cell, clone_dir, limit, skip_failures):
    """`git_files` + `read_text_files` tail shared by every git-based fetcher once its clone
    dir is ready -- `fetch_git_cell` and `fetch_git_before_cell` differ only in how they get
    there."""
    files = git_files(clone_dir, cell["subdir"], cell["glob"], ds.get("exclude", ()))
    if not files:
        if skip_failures:
            return None
        raise SystemExit(f"{ds['name']}/{cell['label']}/{cell['lang']}: 0 files under {cell['subdir']}")
    return read_text_files(clone_dir, files, limit)


def clone_dir_for(ds, cache_dir):
    """One clone per (repo, tag-or-cutoff); a repo pinned by date gets its own key so moving
    the cutoff re-clones instead of reusing the checkout the old cutoff produced."""
    ref = ds.get("tag") or (f"before-{ds['before']}" if ds.get("before") else None)
    return shared_clone_dir(cache_dir, ds["repo"], ref)


def fetch_git_cell(ds, cell, limit, cache_dir, skip_failures):
    clone_dir = clone_dir_for(ds, cache_dir)
    if git_clone_ref(ds["repo"], ds.get("tag"), clone_dir, ds["name"], skip_failures) is None:
        return None
    return materialize_from_clone(ds, cell, clone_dir, limit, skip_failures)


def fetch_git_before_cell(ds, cell, limit, cache_dir, skip_failures):
    clone_dir = clone_dir_for(ds, cache_dir)
    if git_clone_before(ds["repo"], ds["before"], clone_dir, ds["name"], skip_failures) is None:
        return None
    return materialize_from_clone(ds, cell, clone_dir, limit, skip_failures)


def fetch_ghostbuster(ds, limit, cache_dir, skip_failures):
    clone_dir = clone_dir_for(ds, cache_dir)
    if git_clone_ref(ds["repo"], None, clone_dir, ds["name"], skip_failures) is None:
        return None
    domains = ("essay", "reuter", "wp")
    human_domains = ds.get("human_domains") or domains
    human_items, ai_items = [], []
    for domain in domains:
        if domain in human_domains:
            for rel in git_files(clone_dir, f"{domain}/human", "*.txt", ds.get("exclude", ())):
                with open(os.path.join(clone_dir, rel), encoding="utf-8", errors="replace") as fh:
                    human_items.append((fh.read(), {"generator": "human", "domain": domain}))
        for generator in ("gpt", "claude"):
            for rel in git_files(clone_dir, f"{domain}/{generator}", "*.txt", ds.get("exclude", ())):
                with open(os.path.join(clone_dir, rel), encoding="utf-8", errors="replace") as fh:
                    ai_items.append((fh.read(), {"generator": generator, "domain": domain}))
    if not human_items or not ai_items:
        if skip_failures:
            return None
        empty = "human" if not human_items else "ai"
        raise SystemExit(f"{ds['name']}/{empty}: 0 files found")
    return spread_pair(human_items, ai_items, limit)


def fetch_hc3(ds, limit, cache_dir, skip_failures):
    configs = ("wiki_csai", "open_qa", "reddit_eli5", "finance", "medicine")
    human_configs = ds.get("human_domains") or configs
    per_config = None if not limit else max(1, math.ceil(limit / len(configs)))
    human_items, ai_items = [], []
    for cfg in configs:
        rows = hf_rows_sample(ds["dataset"], cfg, "train", per_config, cache_dir, f"hc3/{cfg}", skip_failures)
        if rows is None:
            continue
        for row in rows:
            for h in (row.get("human_answers") or []) if cfg in human_configs else []:
                if h and h.strip():
                    human_items.append((h, {"generator": "human", "domain": cfg}))
            for a in row.get("chatgpt_answers") or []:
                if a and a.strip():
                    ai_items.append((a, {"generator": "chatgpt", "domain": cfg}))
    return spread_pair(human_items, ai_items, limit)


def parse_mage_src(src):
    """`<domain>_human` or `<domain>_machine_<mode>_<model>` -> (domain, generator)."""
    if src.endswith("_human"):
        return src[: -len("_human")], "human"
    marker = "_machine_"
    at = src.find(marker)
    if at == -1:
        return src, "unknown"
    domain, rest = src[:at], src[at + len(marker):]
    _mode, _sep, model = rest.partition("_")
    return domain, model or rest


def fetch_mage(ds, limit, cache_dir, skip_failures):
    out = {}
    # `human_domains` is an exclusion here: MAGE's human `src` values are `<domain>_human`,
    # and the only domain past the human bar is SciGen (arXiv papers up to 2021).
    excluded = " and ".join(f'"src"<>\'{d}_human\'' for d in ds.get("human_domains_excluded", ()))
    for label_value, key in ((1, "human"), (0, "ai")):
        where = f'"label"={label_value}' + (f" and {excluded}" if excluded and key == "human" else "")
        rows = hf_filter_sample(ds["dataset"], "default", "train", where,
                                 limit, cache_dir, f"mage/label={label_value}", skip_failures)
        if rows is None:
            return None
        items = []
        for row in rows:
            domain, generator = parse_mage_src(row.get("src", ""))
            items.append((row["text"], {"generator": generator, "domain": domain}))
        out[key] = items
    return out


# Vietnamese-only letters. FAIDSet mixes English and Vietnamese with no language column, so
# one of these in a row is what keeps it out of the English cell.
VIETNAMESE = re.compile(r"[đĐăĂơƠưƯẠ-ỹ]")

FAIDSET_LABELS = {"human": "human-written", "ai": "LLM-generated", "ai-collab": "human–LLM collaborative"}


def fetch_faidset(ds, limit, cache_dir, skip_failures):
    path = fetch(ds["url_file"], os.path.join(cache_dir, "test.jsonl"), ds["name"], skip_failures, hf_headers())
    if path is None:
        return None
    by_label = {key: [] for key in FAIDSET_LABELS}
    wanted = {v: k for k, v in FAIDSET_LABELS.items()}
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            if not line.strip():
                continue
            row = json.loads(line)
            key = wanted.get(row.get("label"))
            text = row.get("text") or ""
            if key is None or VIETNAMESE.search(text):
                continue
            generator = "human" if key == "human" else row.get("model") or "unknown"
            by_label[key].append((text, {"generator": generator}))
    return {key: [items[i] for i in sample_indices(len(items), limit)] for key, items in by_label.items()}


def parse_beemo_edits(raw):
    """`llama-3.1-70b_edits` / `gpt-4o_edits` hold a Python-literal list of one-key dicts
    (`[{'P1': "..."}]`), one polished rewrite per prompt variant."""
    try:
        edits = ast.literal_eval(raw) if isinstance(raw, str) else raw
    except (SyntaxError, ValueError):
        return []
    return [text for d in edits or () if isinstance(d, dict) for text in d.values() if isinstance(text, str)]


def fetch_beemo(ds, limit, cache_dir, skip_failures):
    rows = hf_rows_sample(ds["dataset"], "default", "train", limit, cache_dir, ds["name"], skip_failures)
    if rows is None:
        return None
    out = {"human": [], "ai": [], "ai-human-edited": [], "ai-llm-edited": []}
    for row in rows:
        model = (row.get("model") or "unknown").rsplit("/", 1)[-1]
        domain = row.get("category")
        out["human"].append((row.get("human_output") or "", {"generator": "human", "domain": domain}))
        out["ai"].append((row.get("model_output") or "", {"generator": model, "domain": domain}))
        out["ai-human-edited"].append((row.get("human_edits") or "", {"generator": f"{model}+human", "domain": domain}))
        for editor in ("gpt-4o", "llama-3.1-70b"):
            for text in parse_beemo_edits(row.get(f"{editor}_edits")):
                out["ai-llm-edited"].append((text, {"generator": f"{model}+{editor}", "domain": domain}))
    return {key: [items[i] for i in sample_indices(len(items), limit)] for key, items in out.items()}


def fetch_aidev(ds, limit, cache_dir, skip_failures):
    rows = hf_rows_sample(ds["dataset"], "pull_request", "train", limit, cache_dir, ds["name"], skip_failures)
    if rows is None:
        return None
    items = []
    for row in rows:
        body = row.get("body")
        if not body or body == "None":
            continue
        items.append((body, {"generator": row.get("agent") or "unknown", "domain": row.get("repo_url")}))
    return {"ai": items}


def fetch_apt_eval(ds, limit, cache_dir, skip_failures):
    """The polished rows and the human originals they were polished from live in two files:
    `merged_apt_eval_dataset.csv` holds only polished text, `original.csv` only the sources."""
    human_items, polished = [], []
    for url, sink in ((ds["url_human"], human_items), (ds["url_file"], polished)):
        path = fetch(url, os.path.join(cache_dir, os.path.basename(url)), ds["name"], skip_failures, hf_headers())
        if path is None:
            return None
        with open(path, encoding="utf-8", newline="") as fh:
            for row in csv.DictReader(fh):
                text = row.get("generation") or ""
                if sink is human_items:
                    sink.append((text, {"generator": "human", "domain": row.get("domain")}))
                    continue
                degree = row.get("polishing_degree") or row.get("polishing_percent") or "unstated"
                sink.append((text, {"generator": row.get("polisher") or "unknown",
                                    "domain": f"{row.get('domain')}/{row.get('polish_type')}/{degree}"}))
    return {"human": [human_items[i] for i in sample_indices(len(human_items), limit)],
            "ai-polished": [polished[i] for i in sample_indices(len(polished), limit)]}


def fetch_wetbench_pt(ds, limit, cache_dir, skip_failures):
    models = ("gemini", "gpt", "mistral", "qwen")
    human_by_revid, ai_items = {}, []
    for model in models:
        url = ds["url_tmpl"].format(model=model)
        path = fetch(url, os.path.join(cache_dir, f"paragraphs_pt_{model}.jsonl"),
                      f"{ds['name']}/{model}", skip_failures, hf_headers())
        if path is None:
            return None
        with open(path, encoding="utf-8") as fh:
            for line in fh:
                if not line.strip():
                    continue
                row = json.loads(line)
                human_by_revid.setdefault(row["revid"], row["trgt"])
                ai_items.append((row["mgt"], {"generator": model, "domain": row.get("page_title")}))
    human_items = [(text, {"generator": "human"}) for text in human_by_revid.values()]
    return spread_pair(human_items, ai_items, limit)


def fetch_diplomatrix(ds, limit, cache_dir, skip_failures):
    path = fetch(ds["url"], os.path.join(cache_dir, "Diplomatrixbr.json"), ds["name"], skip_failures, hf_headers())
    if path is None:
        return None
    with open(path, encoding="utf-8") as fh:
        data = json.load(fh)
    human_items, ai_items = [], []
    for year, entry in data.get("Candidates_Essays", {}).items():
        for cand in entry.get("Candidates", []):
            if cand.get("Essay"):
                human_items.append((cand["Essay"], {"generator": "human", "domain": year}))
    for year, entry in data.get("Models_Essays", {}).items():
        for model_name, payload in entry.get("Models", {}).items():
            if payload.get("Essay"):
                ai_items.append((payload["Essay"], {"generator": model_name, "domain": year}))
    return spread_pair(human_items, ai_items, limit)


# One row per function; every column but `docstring` a rule can read is checked in
# `naples-code`'s `na_rules`. Columns confirmed live via a byte-range GET on the file.
NAPLES_CODE_GENERATORS = ("chatgpt_code", "dsc_code", "qwen_code")


def fetch_naples_code(ds, limit, cache_dir, skip_failures):
    """`python_dataset.jsonl` is 651 MB; stream it and stop once both samples are full
    instead of downloading the whole file like `fetch()` would. The lines actually read are
    cached (`sample.head.jsonl`), so a rerun at the same or a smaller `--limit` never touches
    the network again, and only a `--limit 0` run ever reads past the first few hundred rows.

    ponytail: reading from the front of the file is a head sample, not `hf_filter_sample`'s
    spread sample -- Zenodo has no query endpoint to spread-page against. Upgrade path: fetch
    the whole file once and spread-sample the rows in memory, if head bias ever shows up.
    """
    cache_path = os.path.join(cache_dir, "sample.head.jsonl")
    want = limit or None
    if os.path.exists(cache_path):
        with open(cache_path, encoding="utf-8") as fh:
            raw_lines = fh.readlines()
    else:
        raw_lines = []
        try:
            req = urllib.request.Request(ds["url_file"])
            with urllib.request.urlopen(req, timeout=120) as resp:
                human_n = ai_n = 0
                for raw in resp:
                    line = raw.decode("utf-8", errors="replace")
                    if not line.strip():
                        continue
                    row = json.loads(line)
                    raw_lines.append(line)
                    human_n += bool(row.get("human_code"))
                    ai_n += sum(bool(row.get(c)) for c in NAPLES_CODE_GENERATORS)
                    if want and human_n >= want and ai_n >= want:
                        break
        except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError) as e:
            if skip_failures:
                return None
            raise SystemExit(f"{ds['name']}: giving up: {e}")
        part = f"{cache_path}.part"
        with open(part, "w", encoding="utf-8") as fh:
            fh.writelines(raw_lines)
        os.replace(part, cache_path)
    human_items, ai_items = [], []
    for line in raw_lines:
        row = json.loads(line)
        if row.get("human_code"):
            human_items.append((row["human_code"], {"generator": "human"}))
        for col in NAPLES_CODE_GENERATORS:
            code = row.get(col)
            if code:
                ai_items.append((code, {"generator": col[: -len("_code")]}))
    return {"human": [human_items[i] for i in sample_indices(len(human_items), limit)],
            "ai": [ai_items[i] for i in sample_indices(len(ai_items), limit)]}


def fetch_gpt2_output(ds, limit, cache_dir, skip_failures):
    """Two flat JSONL files, one per class -- the class is which file, exactly like
    `fetch_ghostbuster`'s directory layout."""
    out = {}
    for label, url, generator in (
        ("human", ds["url_human"], "human"),
        ("ai", ds["url_ai"], "gpt-2-xl-1542m"),
    ):
        path = fetch(url, os.path.join(cache_dir, os.path.basename(url)), f"{ds['name']}/{label}", skip_failures)
        if path is None:
            out[label] = None
            continue
        items = []
        with open(path, encoding="utf-8") as fh:
            for line in fh:
                if not line.strip():
                    continue
                row = json.loads(line)
                items.append((row.get("text") or "", {"generator": generator}))
        out[label] = [items[i] for i in sample_indices(len(items), limit)]
    return out


# The two machine-humanized variants: a paraphraser rewrote existing ai text rather than
# generating it, so these never pool with plain `ai`.
PAN25_PARAPHRASE_MODELS = ("gpt-4-turbo-paraphrase", "gemini-pro-paraphrase")


def fetch_pan25(ds, limit, cache_dir, skip_failures):
    """Zenodo ships one zip holding `train.jsonl` (and a `val.jsonl` this bench does not use);
    `fetch()` downloads the zip once, and the stdlib `zipfile` module reads `train.jsonl`
    straight out of it, no extraction to disk needed."""
    path = fetch(ds["url_file"], os.path.join(cache_dir, "pan25.zip"), ds["name"], skip_failures)
    if path is None:
        return None
    out = {"human": [], "ai": [], "ai-paraphrased": []}
    with zipfile.ZipFile(path) as zf, zf.open("train.jsonl") as fh:
        for line in fh:
            if not line.strip():
                continue
            row = json.loads(line)
            model = row.get("model") or "unknown"
            text = row.get("text") or ""
            domain = row.get("genre")
            if row.get("label") == 0:
                out["human"].append((text, {"generator": "human", "domain": domain}))
            elif model in PAN25_PARAPHRASE_MODELS:
                out["ai-paraphrased"].append((text, {"generator": model, "domain": domain}))
            else:
                out["ai"].append((text, {"generator": model, "domain": domain}))
    return {key: [items[i] for i in sample_indices(len(items), limit)] for key, items in out.items()}


def fetch_local_cell(directory, limit, label):
    """A plain directory of files: `--local` points anywhere, `generate_corpus.py`'s synth
    cells point at `<dir>/synth/<cell>/ai/<lang>/` and leave an `index.json` one level up
    (at `<dir>/synth/<cell>/index.json`, keyed by the `ai/<lang>/NNNNN.ext` relative path)
    recording each file's real `generator`; read it when present, fall back to `label`
    otherwise (a plain `--local` directory has no such index)."""
    if not os.path.isdir(directory):
        raise SystemExit(f"{directory}: no such directory")
    cell_dir = os.path.dirname(os.path.dirname(directory))
    index_path = os.path.join(cell_dir, "index.json")
    synth_index = {}
    if os.path.exists(index_path):
        with open(index_path, encoding="utf-8") as fh:
            synth_index = json.load(fh)
    files = sorted(
        os.path.join(dp, f) for dp, _dn, fs in os.walk(directory) for f in fs
        if not f.startswith(".")
    )
    idx = sample_indices(len(files), limit)
    items = []
    for i in idx:
        path = files[i]
        rel_to_cell = os.path.relpath(path, cell_dir).replace(os.sep, "/")
        generator = synth_index.get(rel_to_cell, {}).get("generator", label)
        with open(path, encoding="utf-8", errors="replace") as fh:
            text = fh.read()
        items.append((text, {"generator": generator, "domain": os.path.relpath(path, directory)}))
    return items


# --------------------------------------------------------------------------------------
# Materialize + lint one cell.
# --------------------------------------------------------------------------------------

def write_cell(root, items, ext):
    """Write every item as `NNNNN.<ext>` under `root` plus a sibling `<lang>.index.json`;
    rebuild both from scratch.

    A file a previous run left behind would be walked and counted as if it were still part
    of this run's sample, breaking both the walk-count guard and byte-identical reruns. The
    index lives next to `root`, not inside it: stopslop's walk would otherwise reach it, find
    no `Lang` for a bare `.json` extension, and count it in `stats.skipped`, which `run_lint`
    treats as a hard failure.
    """
    shutil.rmtree(root, ignore_errors=True)
    os.makedirs(root)
    index, written, empty = {}, 0, 0
    for text, meta in items:
        if not text or not text.strip():
            empty += 1
            continue
        name = f"{written:05d}.{ext}"
        with open(os.path.join(root, name), "w", encoding="utf-8", newline="\n") as out:
            out.write(text)
        entry = {k: v for k, v in meta.items() if v is not None}
        entry["words"] = len(text.split())
        index[name] = entry
        written += 1
    with open(f"{root}.index.json", "w", encoding="utf-8") as out:
        json.dump(index, out, indent=2, sort_keys=True)
        out.write("\n")
    return {"files": written, "empty": empty, "index": index}


def run_lint(binary, root, expected):
    """Lint one cell directory and refuse to report numbers the walk did not produce.

    An all-zero table is indistinguishable from a clean result, so a walk that reached fewer
    files than were written is a hard failure rather than a footnote.
    """
    if expected == 0:
        return [], {"files": 0, "skipped": 0, "lines": 0}
    env = dict(os.environ, STOPSLOP_NO_UPDATE_CHECK="1", CI="1")
    proc = subprocess.run(
        [binary, root, "--format", "json", "--stats", "--no-config", "--select", "ALL"],
        capture_output=True, text=True, env=env,
    )
    if proc.returncode not in (0, 1):
        raise SystemExit(f"{binary} {root}: exited {proc.returncode}\n{proc.stderr}")
    payload = json.loads(proc.stdout)
    stats = payload["stats"]
    if stats["files"] != expected or stats["skipped"]:
        raise SystemExit(f"{root}: walk saw {stats} for {expected} files written")
    return payload["findings"], stats


def cell_materialized(root):
    """Whether `root` and its sibling `<root>.index.json` (see `write_cell`) both exist --
    the pair `read_cell` and `carry_over` require before trusting a cell already on disk."""
    return os.path.isdir(root) and os.path.exists(f"{root}.index.json")


def read_cell(root, empty):
    """Recount a cell `--fetch-only` left on disk so `--no-fetch` can score it without the
    network. Dotfiles are skipped because the walk skips them too, and any other disagreement
    between the directory and its index is a half-deleted or hand-edited cell, which is an
    error rather than a smaller sample."""
    index_path = f"{root}.index.json"
    if not cell_materialized(root):
        raise SystemExit(f"{root}: cell not materialized; run --fetch-only first")
    with open(index_path, encoding="utf-8") as fh:
        index = json.load(fh)
    on_disk = {f for f in os.listdir(root) if not f.startswith(".")}
    odd = sorted(on_disk ^ set(index))
    if odd:
        raise SystemExit(f"{root}: directory and {index_path} disagree on {odd[:5]}; run --fetch-only again")
    return {"files": len(on_disk), "empty": empty, "index": index}


def tally(findings):
    """Count files hit per rule code, plus raw findings per rule code.

    Hit sets key on the bare filename (`os.path.basename`, e.g. `00013.py`), not stopslop's
    reported path (which is prefixed by whatever root was walked): every filename is unique
    within one cell directory by construction, and `generator_table` below needs to intersect
    these sets against `index.json`'s keys, which are bare filenames too.
    """
    hits, total = {}, {}
    for f in findings:
        hits.setdefault(f["code"], set()).add(os.path.basename(f["path"]))
        total[f["code"]] = total.get(f["code"], 0) + 1
    return hits, total


def normalize_message(code, message):
    """Collapse the per-file parts of a message so one rule's findings group into a handful
    of shapes: backticked identifiers become `X` (except for `KEEP_BACKTICKS`, where the span
    is the panel entry and stays verbatim, digits included) and digits outside any backtick
    span become N. Quoted spans stay, they name the panel word."""
    if code in KEEP_BACKTICKS:
        return re.sub(r"`[^`]*`|\d+(?:\.\d+)?",
                       lambda m: m.group() if m.group().startswith("`") else "N", message)
    message = re.sub(r"`[^`]*`", "`X`", message)
    return re.sub(r"\d+(\.\d+)?", "N", message)


def message_tally(findings):
    """code -> normalized message -> set of files, the per-message breakdown behind a rule's
    hit rate (which panel word or which threshold did the firing)."""
    out = {}
    for f in findings:
        out.setdefault(f["code"], {}).setdefault(normalize_message(f["code"], f["message"]), set()).add(
            os.path.basename(f["path"]))
    return out


def clip(text, col):
    """`text` cut to `LINE_MAX` characters around column `col`, with an ellipsis on each cut
    end; returns the clipped text and the offset it starts at."""
    text = text.rstrip("\n")
    if len(text) <= LINE_MAX:
        return text, 0
    start = max(0, min(len(text) - LINE_MAX, (col or 1) - 1 - LINE_MAX // 3))
    piece = text[start:start + LINE_MAX]
    return ("…" if start else "") + piece + ("…" if start + LINE_MAX < len(text) else ""), start


def snippet(path, line, col):
    """`[line_no, text, is_hit]` rows for the flagged line and `SNIPPET_CONTEXT` lines each
    side; an unreadable file yields no rows rather than a crash."""
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            lines = fh.read().split("\n")
    except OSError:
        return []
    lo, hi = max(1, line - SNIPPET_CONTEXT), min(len(lines), line + SNIPPET_CONTEXT)
    return [[n, clip(lines[n - 1], col if n == line else 1)[0], n == line] for n in range(lo, hi + 1)]


def examples_for_cell(root, findings, index):
    """Up to `EXAMPLES_PER_RULE` flagged files per rule, spread across the sorted flagged
    files, each with its first finding and a snippet; the HTML page renders these."""
    per_rule = {}
    for f in findings:
        per_rule.setdefault(f["code"], {}).setdefault(os.path.basename(f["path"]), []).append(f)
    out = {}
    for code, per_file in sorted(per_rule.items()):
        names = sorted(per_file)
        out[code] = []
        for i in sample_indices(len(names), EXAMPLES_PER_RULE):
            name, first = names[i], per_file[names[i]][0]
            out[code].append({
                "file": name, "line": first["line"], "col": first["col"], "message": first["message"],
                "fix": first.get("fix"), "meta": {k: v for k, v in index.get(name, {}).items() if k != "words"},
                "more": len(per_file[name]) - 1, "snippet": snippet(os.path.join(root, name), first["line"], first["col"]),
            })
    return out


def score_cell(root, counts, binary, dataset_name, label, lang, applicable):
    findings, stats = run_lint(binary, root, counts["files"])
    strays = sorted({f["code"] for f in findings} - set(applicable))
    if strays:
        raise SystemExit(
            f"{dataset_name}/{label}/{lang}: {strays} fired but is not in APPLICABLE[{lang!r}]; "
            "update bench/score_corpus.py"
        )
    hits, totals = tally(findings)
    words_total = sum(m["words"] for m in counts["index"].values())
    # `tier` is invariant per rule code, so filtering findings directly (rather than
    # re-deriving it from `hits`) is both simpler and cheaper. Basename-keyed to match `hits`.
    tier_a_hits = {os.path.basename(f["path"]) for f in findings if f["tier"] == "A"}
    any_hits = {os.path.basename(f["path"]) for f in findings}
    return {
        "files": counts["files"], "empty": counts["empty"], "index": counts["index"],
        "hits": hits, "totals": totals, "stats": stats, "words_total": words_total,
        "tier_a_hits": tier_a_hits, "any_hits": any_hits, "msgs": message_tally(findings),
        "examples": examples_for_cell(root, findings, counts["index"]),
    }


# --------------------------------------------------------------------------------------
# Metrics.
# --------------------------------------------------------------------------------------

def precision(rate_h, rate_ai):
    """Precision at a 1:1 class prior, exact from the two hit-rate percentages; `None` when
    neither side ever fired.

    Balancing the classes by subsampling would compute the same number while throwing away
    files, and the human side is the half that carries the false-positive evidence worth
    having.
    """
    if rate_h + rate_ai == 0:
        return None
    return rate_ai / (rate_h + rate_ai)


def precision_at_prior(hits_ai, hits_h):
    """Precision from raw hit counts at the cell's own class-size prior (not rebalanced to
    1:1): `hits_ai / (hits_ai + hits_h)` on the counts actually fetched. `None` when neither
    side ever fired."""
    if hits_ai + hits_h == 0:
        return None
    return hits_ai / (hits_ai + hits_h)


def lift(rate_h, rate_ai):
    """ai hit rate over human hit rate. `'inf'` when human is clean and ai is not; `'--'`
    when neither side fires."""
    if rate_h == 0:
        return "inf" if rate_ai > 0 else "--"
    return rate_ai / rate_h


def pct(n, total):
    return 100.0 * n / total if total else 0.0


def per_kloc(findings_n, lines):
    return 1000.0 * findings_n / lines if lines else 0.0


def per_1k_words(findings_n, words):
    return 1000.0 * findings_n / words if words else 0.0


def density(cell, code, lang):
    n = cell["totals"].get(code, 0)
    return per_1k_words(n, cell["words_total"]) if lang == "prose" else per_kloc(n, cell["stats"]["lines"])


def dist(counts):
    ordered = sorted(counts.items(), key=lambda kv: -kv[1])
    return ", ".join(f"{k} {v}" for k, v in ordered[:8]) or "none"


def fmt_ratio(x):
    if x is None:
        return "--"
    if x == "inf":
        return "inf"
    if x == "--":
        return "--"
    return f"{x:.2f}"


# --------------------------------------------------------------------------------------
# Dataset registry.
# --------------------------------------------------------------------------------------

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
            "accepted -- the dataset authors matched outcome buckets deliberately.",
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
        "na_rules": {"SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP042", "SLOP043"},
        "caveats": [
            "Comments and docstrings are stripped by the publisher's extraction pass, so "
            "SLOP001-SLOP004, SLOP042 and SLOP043 print `n/a` here instead of a number that "
            "would just measure the extraction, not the code.",
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
        "na_rules": {"SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP042", "SLOP043"},
        "caveats": [
            "`docstring` ships as its own column, separate from `human_code`, so "
            "SLOP001-SLOP004, SLOP042 and SLOP043 print `n/a` here the same way they do on "
            "codet_m4 -- the extraction pass strips comments and docstrings out of the code "
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
}

# Synthesized cells written by bench/generate_corpus.py (not part of this file's ownership);
# included automatically when present, silently absent otherwise.
SYNTH_CELLS = {
    "synth-pt-wiki": {"lang": "prose", "natlang": "pt"},
    "synth-pt-essay": {"lang": "prose", "natlang": "pt"},
    "synth-tsx": {"lang": "tsx", "natlang": None},
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


# --------------------------------------------------------------------------------------
# Orchestration.
# --------------------------------------------------------------------------------------

CELL_KINDS = ("hf_filter", "url_csv", "git_tag", "git_before")
DATASET_KINDS = ("hf_rows", "url_jsonl", "url_json", "url_table", "git_multi")


def cell_root(args, name, label, lang):
    return os.path.join(args.dir, name, label, lang)


def fetch_dataset(name, ds, args):
    """Fetch and materialize every cell of one dataset without linting it; returns
    `({(label, lang): {"ext", "files", "empty", "index"}}, not_fetched)`.

    A cell whose fetch returns `None` (retries exhausted under `--skip-failures`) never
    touches disk: destroying an already-materialized cell on a transient failure would turn
    one bad network call into a full re-fetch, so a `None` cell instead falls back to the
    last successful fetch's own files, still on disk and still listed in the previous
    manifest at this run's `--limit` -- `write_cell` is what actually rebuilds a cell, and it
    runs only on a real (possibly empty) result.
    """
    cache_dir = os.path.join(args.dir, "cache", name)
    os.makedirs(cache_dir, exist_ok=True)
    written, not_fetched = {}, []
    old_manifest = _stored_manifest(args)
    old_cells = {}
    if old_manifest and old_manifest.get("limit") == args.limit:
        old_entry = old_manifest.get("datasets", {}).get(name)
        if old_entry:
            old_cells = {(c["label"], c["lang"]): c for c in old_entry["cells"]}

    def store(label, lang, ext, items):
        root = cell_root(args, name, label, lang)
        if items is None:
            old = old_cells.get((label, lang))
            if old and cell_materialized(root):
                written[(label, lang)] = read_cell(root, old["empty"]) | {"ext": old["ext"]}
                sys.stderr.write(
                    f"not fetched: {name}/{label}/{lang} (retries exhausted; kept the cell "
                    "from the last successful fetch)\n"
                )
            else:
                not_fetched.append(f"{name}/{label}/{lang}")
                sys.stderr.write(f"not fetched: {name}/{label}/{lang} (retries exhausted; rerun to fill it)\n")
            return
        written[(label, lang)] = write_cell(root, items, ext) | {"ext": ext}

    kind = ds["kind"]
    named_ds = ds | {"name": name}
    if kind in CELL_KINDS:
        # One fetch call per declared cell: `ds["fetch"]` takes (ds, cell, limit, cache_dir,
        # skip_failures) and returns that one cell's items (or None under --skip-failures).
        for cell in ds["cells"]:
            if args.langs and cell["lang"] not in args.langs:
                continue
            items = ds["fetch"](named_ds, cell, args.limit, cache_dir, args.skip_failures)
            store(cell["label"], cell["lang"], cell.get("ext", EXT[cell["lang"]]), items)
    elif kind in DATASET_KINDS:
        # One fetch call for the whole dataset: `ds["fetch"]` takes (ds, limit, cache_dir,
        # skip_failures) and returns every declared cell's items keyed by label at once,
        # because these sources split human/ai out of one shared row stream or clone.
        if args.langs and not any(c["lang"] in args.langs for c in ds["cells"]):
            return written, not_fetched
        result = ds["fetch"](named_ds, args.limit, cache_dir, args.skip_failures)
        for cell in ds["cells"]:
            items = None if result is None else result.get(cell["label"])
            store(cell["label"], cell["lang"], cell.get("ext", EXT[cell["lang"]]), items)
    else:  # kind == "local", the last member of the closed set `self_check` asserts
        for cell in ds["cells"]:
            if args.langs and cell["lang"] not in args.langs:
                continue
            items = fetch_local_cell(cell["dir"], args.limit, cell["label"])
            store(cell["label"], cell["lang"], cell.get("ext", EXT[cell["lang"]]), items)
    return written, not_fetched


def score_dataset(name, ds, written, args):
    """Lint every materialized cell of one dataset; returns lang -> label -> cell."""
    by_lang = {}
    for (label, lang), counts in sorted(written.items()):
        root = cell_root(args, name, label, lang)
        by_lang.setdefault(lang, {})[label] = score_cell(root, counts, args.bin, name, label, lang, APPLICABLE[lang])
    return by_lang


def dataset_revision(name, ds, args):
    if ds["kind"] == "local":
        return "local"
    if ds["kind"] in ("git_tag", "git_before", "git_multi"):
        if ds.get("tag"):
            return ds["tag"]
        clone_dir = clone_dir_for(ds, os.path.join(args.dir, "cache", name))
        return git_head_sha(clone_dir) if os.path.isdir(clone_dir) else "unknown (not cloned)"
    hf_id = ds.get("dataset")
    if not hf_id:
        return "n/a"
    cache_dir = os.path.join(args.dir, "cache", name)
    os.makedirs(cache_dir, exist_ok=True)
    return hf_revision(hf_id, cache_dir, args.skip_failures)


def manifest_path(args):
    return os.path.join(args.dir, MANIFEST)


def _stored_manifest(args):
    """The previous run's manifest, or `None` when absent or written by a different schema --
    a soft lookup for `carry_over` and `fetch_dataset`'s restore path. `read_manifest`
    (`--no-fetch`) needs one to exist and raises instead."""
    path = manifest_path(args)
    if not os.path.exists(path):
        return None
    with open(path, encoding="utf-8") as fh:
        manifest = json.load(fh)
    return manifest if manifest.get("schema") == RESULTS_SCHEMA else None


def carry_over(args, selected, datasets, not_fetched):
    """Keep the manifest entries of datasets this run did not touch, so `--datasets x` fills
    one cell without erasing the record of the other twenty -- including a selected dataset
    whose refresh produced nothing (every cell of it landed in `not_fetched`), so retries
    exhausted on the one dataset asked to refresh doesn't drop it from the manifest either.
    An entry whose cells are no longer on disk, or are missing their index, is dropped rather
    than carried, because `--no-fetch` would fail on it."""
    old = _stored_manifest(args)
    if old is None:
        return not_fetched
    attempted_and_failed = {x.split("/", 1)[0] for x in not_fetched}
    stale_names = [n for n in old.get("datasets", {})
                   if n not in datasets and (n not in selected or n in attempted_and_failed)]
    if stale_names and old.get("limit") != args.limit:
        raise SystemExit(
            f"{manifest_path(args)}: fetched at --limit {old['limit']}, this run used --limit "
            f"{args.limit}; carrying {sorted(stale_names)} forward would mix sample sizes "
            f"under one manifest limit -- select them too, rerun at --limit {old['limit']}, "
            "or delete the manifest to start over at the new limit."
        )
    for name in stale_names:
        entry = old["datasets"][name]
        if all(cell_materialized(cell_root(args, name, c["label"], c["lang"])) for c in entry["cells"]):
            datasets[name] = entry
    # A cell just restored into `datasets` (above, or already by `fetch_dataset`) is no
    # longer "not fetched", whether it came from `old`'s record or this run's own failures.
    stale = {f"{name}/{c['label']}/{c['lang']}" for name, e in datasets.items() for c in e["cells"]}
    kept = [x for x in old.get("not_fetched", ()) if x.split("/")[0] not in selected and x not in stale]
    fresh = [x for x in not_fetched if x not in stale]
    return sorted(set(fresh) | set(kept))


def write_manifest(args, fetched, revisions, registry, not_fetched, selected):
    """Record what `--fetch-only` materialized so `--no-fetch` can score it later: the limit
    (a rerun must not re-declare it), per-cell counts (the walk guard needs `files`, the report
    needs `empty`), revisions (network or clone state), and the full registry entry of any
    dataset outside `DATASETS` (a `--local` or synth cell has nowhere else to live)."""
    datasets = {}
    for name, written in sorted(fetched.items()):
        entry = {"revision": revisions[name], "cells": [
            {"label": label, "lang": lang, "ext": c["ext"], "files": c["files"], "empty": c["empty"]}
            for (label, lang), c in sorted(written.items())
        ]}
        if name not in DATASETS:
            entry["registry"] = {k: v for k, v in registry[name].items() if k != "fetch"}
        datasets[name] = entry
    not_fetched = carry_over(args, set(selected), datasets, not_fetched)
    payload = {"schema": RESULTS_SCHEMA, "limit": args.limit, "datasets": datasets,
               "not_fetched": sorted(not_fetched)}
    with open(manifest_path(args), "w", encoding="utf-8") as fh:
        json.dump(payload, fh, indent=1, sort_keys=True)
        fh.write("\n")
    return payload


def read_manifest(args):
    path = manifest_path(args)
    if not os.path.exists(path):
        raise SystemExit(f"{path}: no manifest; run --fetch-only (just corpus-fetch) first")
    with open(path, encoding="utf-8") as fh:
        manifest = json.load(fh)
    if manifest.get("schema") != RESULTS_SCHEMA:
        raise SystemExit(f"{path}: schema {manifest.get('schema')!r}, this script writes {RESULTS_SCHEMA}")
    return manifest


# --------------------------------------------------------------------------------------
# Report.
# --------------------------------------------------------------------------------------

def generator_table(cell, applicable, na_rules):
    """rule x generator hit%, generators with >= MIN_GENERATOR_FILES files, rules with
    >= LOW_SUPPORT total hits in this cell."""
    by_generator = {}
    for name, meta in cell["index"].items():
        by_generator.setdefault(meta.get("generator", "unknown"), []).append(name)
    generators = sorted(g for g, files in by_generator.items() if len(files) >= MIN_GENERATOR_FILES)
    if not generators:
        return None
    rules = [c for c in applicable if c not in na_rules and len(cell["hits"].get(c, ())) >= LOW_SUPPORT]
    if not rules:
        return None
    generator_sets = {g: set(by_generator[g]) for g in generators}
    header = ["rule"] + generators
    rows = [header, ["---"] * len(header)]
    for code in rules:
        hit_files = cell["hits"].get(code, set())
        row = [code]
        for g in generators:
            files = generator_sets[g]
            row.append(f"{pct(len(hit_files & files), len(files)):.1f}%")
        rows.append(row)
    return "\n".join("| " + " | ".join(r) + " |" for r in rows)


def detail_table(by_label, lang, na_rules):
    """One markdown table for a (dataset, lang) pair: rule rows, human/ai(+robustness)
    columns, precision/prior/lift when both human and ai are present."""
    applicable = APPLICABLE[lang]
    human, ai = by_label.get("human"), by_label.get("ai")
    robustness = [l for l in ("ai-paraphrased", "ai-refined", "ai-adversarial") if l in by_label]
    header = ["rule"]
    if human:
        header.append("human hits")
    if ai:
        header.append("ai hits")
    header += [f"{l} hits" for l in robustness]
    if human and ai:
        header += ["precision @1:1", "precision @prior", "lift", "human /KLoC" if lang != "prose" else "human /1k words", "ai /KLoC" if lang != "prose" else "ai /1k words"]
    lines = ["| " + " | ".join(header) + " |", "|" + "---|" * len(header)]

    def hit_cell(cell, code):
        if cell is None:
            return None
        n = len(cell["hits"].get(code, ()))
        return n, pct(n, cell["files"])

    rows = []
    for code in applicable:
        if code in na_rules:
            row = [code] + ["n/a"] * (len(header) - 1)
            rows.append((0.0, row))
            continue
        row = [code]
        h_n = h_pct = a_n = a_pct = 0
        if human:
            h_n, h_pct = hit_cell(human, code)
            row.append(f"{h_n} ({h_pct:.2f}%)")
        if ai:
            a_n, a_pct = hit_cell(ai, code)
            row.append(f"{a_n} ({a_pct:.2f}%)")
        for label in robustness:
            n, p = hit_cell(by_label[label], code)
            row.append(f"{n} ({p:.2f}%)")
        if human and ai:
            star = "*" if 0 < h_n + a_n < LOW_SUPPORT else ""
            row.append(fmt_ratio(precision(h_pct, a_pct)) + star)
            row.append(fmt_ratio(precision_at_prior(a_n, h_n)) + star)
            row.append(fmt_ratio(lift(h_pct, a_pct)))
            row.append(f"{density(human, code, lang):.2f}")
            row.append(f"{density(ai, code, lang):.2f}")
        rows.append((a_pct if ai else h_pct, row))
    rows.sort(key=lambda r: (-r[0], r[1][0]))
    lines += ["| " + " | ".join(r) + " |" for _, r in rows]

    def union_row(label_text, set_key):
        row = [label_text]
        h_n = a_n = h_pct = a_pct = 0
        if human:
            h_n = len(human[set_key])
            h_pct = pct(h_n, human["files"])
            row.append(f"{h_n} ({h_pct:.2f}%)")
        if ai:
            a_n = len(ai[set_key])
            a_pct = pct(a_n, ai["files"])
            row.append(f"{a_n} ({a_pct:.2f}%)")
        for label in robustness:
            c = by_label[label]
            n = len(c[set_key])
            row.append(f"{n} ({pct(n, c['files']):.2f}%)")
        if human and ai:
            row.append(fmt_ratio(precision(h_pct, a_pct)))
            row.append(fmt_ratio(precision_at_prior(a_n, h_n)))
            row.append(fmt_ratio(lift(h_pct, a_pct)))
            row += ["", ""]
        return row

    lines.append("| " + " | ".join(union_row("**any Tier A rule**", "tier_a_hits")) + " |")
    lines.append("| " + " | ".join(union_row("**any rule**", "any_hits")) + " |")
    return "\n".join(lines)


def lang_summary_matrix(lang, datasets_built, registry, only):
    """rows = APPLICABLE[lang] + the two union rows; columns = each dataset with a cell of
    this lang, H%/A% (blank when a side is absent, n/a for a dataset's na_rules). `only`
    restricts the columns to a set of dataset names, which is how the verified and unverified
    halves of the report get their own matrices."""
    cols = []  # (dataset_name, header_suffix, by_label)
    for name, by_lang in datasets_built.items():
        if lang not in by_lang or name not in only:
            continue
        ds = registry[name]
        suffix = ""
        cells_for_lang = [c for c in ds.get("cells", []) if c.get("lang") == lang]
        if any(c.get("proxy") for c in cells_for_lang):
            suffix += " (js proxy)"
        if ds.get("synthesized"):
            suffix += " (synthesized)"
        cols.append((name, suffix, by_lang[lang], ds.get("na_rules", ())))
    if not cols:
        return None
    header = ["rule"]
    for name, suffix, _by_label, _na in cols:
        header += [f"{name}{suffix} H%", f"{name}{suffix} A%"]
    lines = ["#### " + lang, "", "| " + " | ".join(header) + " |", "|" + "---|" * len(header)]
    rule_rows = list(APPLICABLE[lang]) + ["**any Tier A rule**", "**any rule**"]
    for code in rule_rows:
        row = [code]
        for _name, _suffix, by_label, na in cols:
            human, ai = by_label.get("human"), by_label.get("ai")
            if code in na:
                row += ["n/a", "n/a"]
                continue
            for cell, key in ((human, "hits"), (ai, "hits")):
                if cell is None:
                    row.append("")
                    continue
                if code == "**any Tier A rule**":
                    n = len(cell["tier_a_hits"])
                elif code == "**any rule**":
                    n = len(cell["any_hits"])
                else:
                    n = len(cell[key].get(code, ()))
                row.append(f"{pct(n, cell['files']):.2f}%")
        lines.append("| " + " | ".join(row) + " |")
    return "\n".join(lines)


def has_human(ds):
    return any(c["label"] == "human" for c in ds.get("cells", ()))


def is_verified(ds):
    """A dataset's human split counts toward the pooled human rates only when its provenance
    clears the bar; an AI-only dataset has no human split to qualify and is never pooled."""
    return has_human(ds) and ds.get("human_provenance") in VERIFIED


def split_by_provenance(names, registry):
    """(verified, unverified) dataset names: the divider the report and the HTML page draw."""
    verified = [n for n in names if is_verified(registry[n])]
    return verified, [n for n in names if n not in set(verified)]


def dataset_meta_bullets(name, ds, by_lang, revision, args):
    total_files = sum(c["files"] for by_label in by_lang.values() for c in by_label.values())
    lines = [f"### {name}", ""]
    if ds.get("url"):
        lines.append(f"- Source: {ds['url']}")
    if ds.get("paper"):
        lines.append(f"- Paper: {ds['paper']}")
    lines.append(f"- License: {ds.get('license') or 'not stated on the dataset card'}")
    lines.append(f"- Natural language: {ds.get('natlang') or 'n/a (code)'}")
    if ds.get("generator_years"):
        lines.append(f"- Generators: {ds['generator_years']}")
    if has_human(ds):
        lines.append(f"- Human split: {ds.get('human_provenance') or 'unverified'} -- "
                     f"{ds.get('human_note') or 'provenance not recorded'}")
    lines.append(f"- Revision: {revision}")
    lines.append(f"- Files fetched: {total_files}")
    for lang, by_label in sorted(by_lang.items()):
        parts = ", ".join(f"{label} {cell['files']} ({cell['empty']} blank dropped)"
                           for label, cell in sorted(by_label.items()))
        lines.append(f"  - {lang}: {parts}")
        for label, cell in sorted(by_label.items()):
            if label == "human":
                continue
            generators = {m.get("generator", "unknown") for m in cell["index"].values()}
            if len(generators) > 1:
                counts = {}
                for m in cell["index"].values():
                    g = m.get("generator", "unknown")
                    counts[g] = counts.get(g, 0) + 1
                lines.append(f"    - {label} generators: {dist(counts)}")
    proxies = {c["proxy"] for c in ds.get("cells", []) if c.get("proxy")}
    for p in sorted(proxies):
        lines.append(f"- {p}")
    for c in ds.get("caveats", []):
        lines.append(f"- {c}")
    if ds.get("na_rules"):
        lines.append(f"- `n/a` rules on this dataset: {', '.join(sorted(ds['na_rules']))}")
    return lines


def build_report(datasets_built, revisions, registry, args, not_fetched):
    version = subprocess.run([args.bin, "--version"], capture_output=True, text=True).stdout.strip()
    lines = [
        "## Per-rule hit rate across the corpus registry",
        "",
        f"- Binary: {version}, run as `stopslop <dir> --format json --stats --no-config --select ALL`",
        f"- --limit: {args.limit or 'all (0)'}" + ("  **-- a deterministic spread sample, never a reported number, when non-zero and below a dataset's full size**" if args.limit else ""),
    ]
    for name in sorted(datasets_built):
        lines.append(f"- {name} revision: {revisions.get(name, 'n/a')}")
    if not_fetched:
        lines.append(
            "- **Not fetched** (retries exhausted under `--skip-failures`; absent from every "
            f"table below, rerun to fill them from cache): {', '.join(sorted(not_fetched))}"
        )
    lines.append("")
    lines.append("### How to read this")
    lines.append("")
    lines += [
        "- `human hits` is the share of human files where the rule fired. Under \"the rule "
        "firing predicts AI-authored\" that share **is** the false-positive rate.",
        "- `precision @1:1` assumes one ai file per human file, a property of this table's "
        "construction, not of any repository; it re-expresses the two hit rates and carries "
        "no information they do not. `precision @prior` instead uses the raw counts actually "
        "fetched, so it reflects this run's `--limit` and each cell's real population. `--` "
        "means neither side fired.",
        "- `lift` is the ai hit rate divided by the human hit rate: `inf` means the rule is "
        "clean on human files here and not on ai ones; `--` means neither side fired.",
        "- Findings per KLoC/1k words only mean something read against the hit rate: a high "
        "hit rate next to a modest density is a rule weakly present everywhere, and a low hit "
        "rate next to a high density is a rule that fires hard in a few files.",
        f"- A `*` marks a precision built from fewer than {LOW_SUPPORT} hits in total. Read "
        "those off the counts, not the ratio.",
        "- The `any rule` row is the OR of every applicable rule for that lang, so its "
        "precision tracks the noisiest member rather than the best one; `any Tier A rule` is "
        "the same OR restricted to rules CI actually fails on.",
        "- A hit rate is evidence that the tells appear in that corpus. It is not a claim "
        "that any file is AI-written, and this crate ships no such claim.",
        f"- {UNSCOREABLE_NOTE}",
        "- Hugging Face cells are sampled through `/filter` when the datasets-server index "
        "answers and through a spread `/rows` scan when it does not. Both are deterministic, "
        "and a `scan_*` marker in the page cache pins a scanned cell to the scan on later "
        "runs, so reruns reproduce this file byte for byte; a cold cache under the other "
        "path draws a different sample of the same population.",
        "",
    ]
    verified, unverified = split_by_provenance(sorted(datasets_built), registry)
    lines.append("### Which human splits count as human")
    lines.append("")
    lines += [
        "A split is read as human only when its text is dated 2019 or earlier "
        "(`pinned-2019`) or was written by identified people under controlled conditions "
        "(`verified-authors`). Every other split is `unverified` and reported below a divider.",
        "",
        "| dataset | human split | source and years | generators |",
        "|---|---|---|---|",
    ]
    for name in sorted(datasets_built):
        ds = registry[name]
        prov = ds.get("human_provenance") or ("ai only" if not has_human(ds) else "unverified")
        lines.append(f"| {name} | {prov} | {ds.get('human_note') or '--'} | {ds.get('generator_years') or '--'} |")
    lines.append("")

    def summaries(names, heading):
        out = [heading, ""]
        for lang in ("python", "go", "rust", "typescript", "tsx", "prose"):
            if args.langs and lang not in args.langs:
                continue
            matrix = lang_summary_matrix(lang, datasets_built, registry, only=set(names))
            if matrix:
                out += [matrix, ""]
                if lang == "rust":
                    out += [
                        "Plain ai Rust in this table, when present, comes only from the "
                        "synthesized `synth-rust` cell; no real-world dataset registered here "
                        "ships plain machine-generated Rust (see the droid caveats below).",
                        "",
                    ]
        return out if len(out) > 2 else []

    lines += summaries(verified, "## Summary, by lang: verified human splits")
    if unverified:
        lines += ["---", "", PROVENANCE_NOTE, ""]
        lines += summaries(unverified, "## Summary, by lang: unverified human splits")
    lines.append("## Per-dataset detail")
    lines.append("")
    for name in verified + unverified:
        if unverified and name == unverified[0]:
            lines += ["---", "", PROVENANCE_NOTE, ""]
        ds = registry[name]
        by_lang = datasets_built[name]
        lines += dataset_meta_bullets(name, ds, by_lang, revisions.get(name, "n/a"), args)
        lines.append("")
        for lang, by_label in sorted(by_lang.items()):
            lines.append(f"#### {name} / {lang}")
            lines.append("")
            lines.append(detail_table(by_label, lang, ds.get("na_rules", ())))
            lines.append("")
            ai_cell = by_label.get("ai")
            if ai_cell:
                gt = generator_table(ai_cell, APPLICABLE[lang], ds.get("na_rules", ()))
                if gt:
                    lines.append(f"Per-generator (ai, >= {MIN_GENERATOR_FILES} files, rule >= {LOW_SUPPORT} hits):")
                    lines.append("")
                    lines.append(gt)
                    lines.append("")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


# --------------------------------------------------------------------------------------
# results.json: the same numbers the markdown report holds, plus flagged lines, per-message
# tallies and rule metadata, for bench/analyze_corpus.py and bench/render_report.py.
# --------------------------------------------------------------------------------------

README_RULE_ROW = re.compile(
    r"^\| (SLOP\d{3}) \| (\w+) \| (.+?) \| ([AB]), (on|off)\s*\| (.+?) \| (.+?) \| (.+?) \|$")


def parse_rules(readme_text):
    """The README rule table is the only place that records each rule's langs, natlangs and
    one-line description; `--list-rules` carries neither langs nor description."""
    rules = {}
    for line in readme_text.splitlines():
        m = README_RULE_ROW.match(line)
        if m:
            code, group, name, tier, default, langs, natlangs, desc = m.groups()
            rules[code] = {"group": group, "name": name, "tier": tier, "default": default,
                           "langs": langs, "natlangs": natlangs, "desc": desc}
    return rules


def rule_codes(binary):
    proc = subprocess.run([binary, "--list-rules"], capture_output=True, text=True)
    if proc.returncode != 0:
        raise SystemExit(f"{binary} --list-rules: exited {proc.returncode}\n{proc.stderr}")
    return {line.split()[0] for line in proc.stdout.splitlines() if line.startswith("SLOP")}


def load_rules(binary, readme_path):
    """README table vs `--list-rules`: a rule shipped but undocumented (or the reverse) would
    silently drop out of every table downstream, so the mismatch is an error here -- and so
    would an empty parse on either side, which a stale `--bin` or a reformatted table turns
    into a vacuously matching pair of empty sets."""
    with open(readme_path, encoding="utf-8") as fh:
        rules = parse_rules(fh.read())
    codes = rule_codes(binary)
    if not rules or not codes:
        raise SystemExit(f"{readme_path}: parsed {len(rules)} README row(s) and {len(codes)} "
                         f"--list-rules code(s) from {binary!r}; expected both non-empty")
    missing, extra = sorted(codes - set(rules)), sorted(set(rules) - codes)
    if missing or extra:
        raise SystemExit(f"{readme_path}: rule table and --list-rules disagree; "
                         f"undocumented {missing}, unshipped {extra}")
    return rules


def generator_counts(index):
    counts = {}
    for meta in index.values():
        g = meta.get("generator", "unknown")
        counts[g] = counts.get(g, 0) + 1
    return counts


def dataset_meta(name, ds, by_lang, revision):
    return {
        "source": ds.get("url"), "paper": ds.get("paper"),
        "license": ds.get("license") or "not stated on the dataset card",
        "natural_language": ds.get("natlang") or "n/a (code)",
        "generator_years": ds.get("generator_years"),
        "human_provenance": ds.get("human_provenance") if has_human(ds) else None,
        "human_note": ds.get("human_note") if has_human(ds) else None,
        "verified": is_verified(ds),
        "revision": revision,
        "files_fetched": sum(c["files"] for by_label in by_lang.values() for c in by_label.values()),
        "synthesized": bool(ds.get("synthesized")),
        "proxies": sorted({c["proxy"] for c in ds.get("cells", ()) if c.get("proxy")}),
        "notes": list(ds.get("caveats", ())),
    }


def results_payload(datasets_built, revisions, registry, args, rules, not_fetched):
    version = subprocess.run([args.bin, "--version"], capture_output=True, text=True).stdout.strip()
    cells = []
    for name in sorted(datasets_built):
        for lang, by_label in sorted(datasets_built[name].items()):
            for label, cell in sorted(by_label.items()):
                cells.append({
                    "dataset": name, "label": label, "lang": lang,
                    "files": cell["files"], "empty": cell["empty"],
                    "flagged_files": len(cell["any_hits"]),
                    "tier_a_files": len(cell["tier_a_hits"]),
                    "words": cell["words_total"], "lines": cell["stats"]["lines"],
                    "rules": {code: {"files": len(files), "findings": cell["totals"].get(code, 0)}
                              for code, files in sorted(cell["hits"].items())},
                    "msgs": {code: {msg: len(files) for msg, files in sorted(by_msg.items())}
                             for code, by_msg in sorted(cell["msgs"].items())},
                    "examples": cell["examples"],
                    "generators": generator_counts(cell["index"]),
                })
    return {
        "schema": RESULTS_SCHEMA, "binary": version, "limit": args.limit,
        "provenance_note": PROVENANCE_NOTE,
        "rules": rules,
        "applicable": {lang: list(codes) for lang, codes in APPLICABLE.items()},
        "datasets": {name: dataset_meta(name, registry[name], datasets_built[name], revisions.get(name, "n/a"))
                     for name in sorted(datasets_built)},
        "na_rules": {name: sorted(registry[name]["na_rules"]) for name in sorted(datasets_built)
                     if registry[name].get("na_rules")},
        "not_fetched": sorted(not_fetched),
        "cells": cells,
    }


# --------------------------------------------------------------------------------------
# Self-check + CLI.
# --------------------------------------------------------------------------------------

def self_check():
    assert precision(0.0, 0.0) is None
    assert precision(4.0, 4.0) == 0.5
    assert precision(0.0, 4.0) == 1.0
    assert precision_at_prior(0, 0) is None
    assert precision_at_prior(3, 1) == 0.75
    assert lift(0.0, 0.0) == "--"
    assert lift(0.0, 5.0) == "inf"
    assert lift(10.0, 20.0) == 2.0
    assert pct(1, 4) == 25.0 and pct(0, 0) == 0.0
    assert per_kloc(2, 1000) == 2.0
    assert per_1k_words(2, 500) == 4.0
    assert sample_indices(10, 0) == list(range(10))
    assert sample_indices(10, 20) == list(range(10))
    assert sample_indices(10, 5) == [0, 2, 4, 6, 8]
    assert len(sample_indices(1000, 7)) == 7
    assert sample_indices(1000, 7) == sorted(set(sample_indices(1000, 7)))
    sparse = spread_page_offsets(100000, 5)
    assert len(sparse) == 5 and not _windows_overlap(sparse)
    assert any(offset for offset, _ in sparse), "a spread sample must not read the literal head"
    single = spread_page_offsets(100000, 1)
    assert single[0][0] != 0, "a single page must still center, not read offset 0"
    dense = spread_page_offsets(250, 3)
    assert not _windows_overlap(dense)
    assert sum(length for _, length in dense) == 250, "full coverage must not drop rows"
    assert parse_mage_src("cmv_human") == ("cmv", "human")
    assert parse_mage_src("eli5_machine_topical_text-davinci-003") == ("eli5", "text-davinci-003")
    assert set(EXT) == set(APPLICABLE)

    # /rows scan fallback: the where grammar round-trips, and every registered where clause
    # is scannable, so a `/filter` outage can never strand a cell on a parse error.
    keep = where_predicate("\"Language\"='Go' and \"Label\"<>'HUMAN_GENERATED'")
    assert keep({"Language": "Go", "Label": "MACHINE_GENERATED"})
    assert not keep({"Language": "Go", "Label": "HUMAN_GENERATED"})
    assert not keep({"Language": "Rust", "Label": "MACHINE_GENERATED"})
    label_is_one = where_predicate('"label"=1')
    assert label_is_one({"label": 1}) and label_is_one({"label": "1"}) and not label_is_one({"label": 0})
    for spec in DATASETS.values():
        for cell in spec.get("cells", []):
            if "where" in cell:
                where_predicate(cell["where"])
    rejected = False
    try:
        where_predicate("\"a\" LIKE 'b%'")
    except SystemExit:
        rejected = True
    assert rejected, "an unsupported where clause must be rejected"
    assert sorted(scan_order(7)) == list(range(7))
    assert scan_order(7)[:2] != [0, 1], "the scan must not walk the head first"

    # _paged_rows: two pages returning overlapping row_idx values (the boundary-rounding case
    # the docstring names) must still dedupe to exactly `want` unique rows.
    calls = []

    def fake_call_page(offset, length):
        base = 0 if not calls else 50
        calls.append((offset, length))
        return {"rows": [{"row_idx": base + i, "row": base + i} for i in range(100)]}

    assert _paged_rows(fake_call_page, 300, 150, False) == list(range(150))

    with tempfile.TemporaryDirectory() as tmp:
        # write_cell: a real item, a blank item, and a whitespace-only item.
        counts = write_cell(os.path.join(tmp, "cell"), [
            ("hello world", {"generator": "human"}),
            ("", {"generator": "human"}),
            ("   \n\t  ", {"generator": "human"}),
        ], "py")
        assert counts["files"] == 1 and counts["empty"] == 2
        assert next(iter(counts["index"].values()))["words"] == 2

    with tempfile.TemporaryDirectory() as tmp:
        # fetch_local_cell: generator comes from a synth index.json when the file is listed,
        # falls back to the passed label when it is not.
        cell_dir = os.path.join(tmp, "synth", "mycell")
        ai_dir = os.path.join(cell_dir, "ai", "python")
        os.makedirs(ai_dir)
        for fname, body in (("00000.py", "print(1)\n"), ("00001.py", "print(2)\n")):
            with open(os.path.join(ai_dir, fname), "w", encoding="utf-8") as fh:
                fh.write(body)
        indexed_rel = os.path.relpath(os.path.join(ai_dir, "00000.py"), cell_dir).replace(os.sep, "/")
        with open(os.path.join(cell_dir, "index.json"), "w", encoding="utf-8") as fh:
            json.dump({indexed_rel: {"generator": "gpt-4"}}, fh)
        local_gens = sorted(meta["generator"] for _text, meta in fetch_local_cell(ai_dir, 0, "fallback"))
        assert local_gens == ["fallback", "gpt-4"]

    # tally: the same basename under two different dirs is one hit file; totals count every
    # finding, not just distinct files.
    hits, totals = tally([
        {"code": "SLOP001", "path": "/a/00001.py"},
        {"code": "SLOP001", "path": "/b/00001.py"},
        {"code": "SLOP002", "path": "/a/00002.py"},
    ])
    assert hits["SLOP001"] == {"00001.py"}
    assert totals == {"SLOP001": 2, "SLOP002": 1}

    # _looks_retryable: a transient error body, a clean body, non-JSON, a bare JSON array (no
    # `.get`), and a non-UTF-8 body (the EH-156 regression case).
    assert _looks_retryable(b'{"error": "index is loading"}') is True
    assert _looks_retryable(b'{"rows": []}') is False
    assert _looks_retryable(b"not json") is False
    assert _looks_retryable(b"[1, 2, 3]") is False
    assert _looks_retryable(b"\x80\x81\x82\x83") is False

    with tempfile.TemporaryDirectory() as tmp:
        # hf_get_cached: a pre-written cache file must short-circuit before ever touching the
        # network -- `.invalid` is a reserved TLD (RFC 2606), so a bug here hangs, not lies.
        cache_path = os.path.join(tmp, "cached.json")
        with open(cache_path, "w", encoding="utf-8") as fh:
            json.dump({"ok": True}, fh)
        cached = hf_get_cached("https://definitely-not-a-real-host.invalid/x", cache_path, "test", False)
        assert cached == {"ok": True}

    assert parse_local("name=dir:human:python") == ("name", "dir", "human", "python")
    for bad_spec in ("no-equals-sign", "name=dir:onlylabel", "name=dir:human:notalang"):
        try:
            parse_local(bad_spec)
            assert False, f"parse_local({bad_spec!r}) should have raised SystemExit"
        except SystemExit:
            continue

    with tempfile.TemporaryDirectory() as tmp:
        # carry_over: an untouched dataset carries forward, and so does a *selected* one
        # whose refresh produced nothing; a --limit that would mix sample sizes refuses.
        args = argparse.Namespace(dir=tmp, limit=500)
        root = cell_root(args, "x", "human", "python")
        os.makedirs(root)
        with open(os.path.join(root, "00000.py"), "w", encoding="utf-8") as fh:
            fh.write("pass\n")
        with open(f"{root}.index.json", "w", encoding="utf-8") as fh:
            json.dump({"00000.py": {"generator": "human", "words": 1}}, fh)
        with open(manifest_path(args), "w", encoding="utf-8") as fh:
            json.dump({
                "schema": RESULTS_SCHEMA, "limit": 500, "not_fetched": [],
                "datasets": {"x": {"revision": "r1", "cells": [
                    {"label": "human", "lang": "python", "ext": "py", "files": 1, "empty": 0},
                ]}},
            }, fh)

        datasets = {}
        carry_over(args, {"y"}, datasets, [])
        assert "x" in datasets, "a dataset outside this run's selection must be carried forward"

        datasets = {}
        not_fetched = carry_over(args, {"x"}, datasets, ["x/human/python"])
        assert "x" in datasets, "a selected dataset whose refresh produced nothing must be restored"
        assert not_fetched == [], "a restored cell must not also read as not-fetched"

        rejected = False
        try:
            carry_over(argparse.Namespace(dir=tmp, limit=300), {"x"}, {}, ["x/human/python"])
        except SystemExit:
            rejected = True
        assert rejected, "a --limit mismatched against the manifest must raise, not silently carry"

    # normalize_message: a user identifier collapses so one rule's findings group by shape,
    # a library name in a SLOP037 message stays because that span is the panel entry, and a
    # quoted panel word survives either way.
    assert normalize_message("SLOP039", "`newClient` only forwards to `client`") == "`X` only forwards to `X`"
    assert normalize_message("SLOP037", "`ioutil.ReadFile` has a direct `os`/`io` replacement") == \
        "`ioutil.ReadFile` has a direct `os`/`io` replacement"
    assert normalize_message("SLOP037", "`sha256sum` ran on 3 files") == "`sha256sum` ran on N files", \
        "a digit inside a KEEP_BACKTICKS span must survive; only the digit outside it collapses"
    assert normalize_message("SLOP033", "sentence runs 51 words; split it") == "sentence runs N words; split it"
    assert normalize_message("SLOP027", 'filler phrase repeated: "in order to" appears multiple times') == \
        'filler phrase repeated: "in order to" appears multiple times'
    msgs = message_tally([
        {"code": "SLOP033", "path": "/a/1.md", "message": "sentence runs 51 words; split it"},
        {"code": "SLOP033", "path": "/a/2.md", "message": "sentence runs 62 words; split it"},
        {"code": "SLOP033", "path": "/a/2.md", "message": "sentence runs 71 words; split it"},
    ])
    assert msgs == {"SLOP033": {"sentence runs N words; split it": {"1.md", "2.md"}}}

    assert clip("x" * 300, 1)[0].endswith("…") and len(clip("x" * 300, 1)[0]) == LINE_MAX + 1
    assert clip("short line", 1) == ("short line", 0)

    # Provenance: the bar is a property of the human split, and an AI-only dataset never
    # counts as verified (it has no human rate to pool).
    assert is_verified({"cells": [{"label": "human"}], "human_provenance": "pinned-2019"})
    assert not is_verified({"cells": [{"label": "human"}], "human_provenance": "unverified"})
    assert not is_verified({"cells": [{"label": "ai"}], "human_provenance": "pinned-2019"})
    for name, ds in DATASETS.items():
        prov = ds.get("human_provenance")
        assert (prov in PROVENANCE) if has_human(ds) else (prov is None), f"{name}: human_provenance {prov!r}"
        assert bool(ds.get("human_note")) == has_human(ds), f"{name}: human_note must match its human split"
        assert ds["kind"] in CELL_KINDS + DATASET_KINDS + ("local",), f"{name}: unknown kind"
    verified, unverified = split_by_provenance(["cpython-lib", "droid"], DATASETS)
    assert verified == ["cpython-lib"] and unverified == ["droid"]

    # FAIDSet's English/Vietnamese split has no language column, so the letter test is the
    # only thing keeping Vietnamese rows out of the English cell.
    assert VIETNAMESE.search("Ứng dụng giao đồ ăn") and not VIETNAMESE.search("Among its noteworthy features")
    assert parse_beemo_edits("[{'P1': 'first'}, {'P2': 'second'}]") == ["first", "second"]
    assert parse_beemo_edits("not a literal") == [] and parse_beemo_edits(None) == []

    assert parse_rules("| SLOP001 | artifact | Elision | A, on | Python, Go | en | drops code |\n") == {
        "SLOP001": {"group": "artifact", "name": "Elision", "tier": "A", "default": "on",
                    "langs": "Python, Go", "natlangs": "en", "desc": "drops code"}}

    with tempfile.TemporaryDirectory() as tmp:
        # read_cell counts exactly what the walk will see: a dotfile is skipped by both, and
        # any other disagreement with the index is a half-deleted cell, not a smaller sample.
        root = os.path.join(tmp, "cell")
        write_cell(root, [("a", {"generator": "human"}), ("b", {"generator": "human"})], "py")
        with open(os.path.join(root, ".DS_Store"), "w", encoding="utf-8") as fh:
            fh.write("junk")
        assert read_cell(root, 3)["files"] == 2 and read_cell(root, 3)["empty"] == 3
        os.remove(os.path.join(root, "00001.py"))
        try:
            read_cell(root, 0)
            assert False, "read_cell must reject a cell the index and directory disagree on"
        except SystemExit as refused:
            assert "disagree" in str(refused)

    # results.json must survive json.dumps: every set has to be converted at the payload
    # boundary, and a stray set here is a crash at the end of a 20-minute run otherwise.
    fake_cell = {
        "files": 2, "empty": 0, "index": {"00000.py": {"generator": "gpt-4", "words": 3}},
        "hits": {"SLOP003": {"00000.py"}}, "totals": {"SLOP003": 1},
        "stats": {"files": 2, "skipped": 0, "lines": 9}, "words_total": 3,
        "tier_a_hits": {"00000.py"}, "any_hits": {"00000.py"},
        "msgs": {"SLOP003": {"stray markdown code fence in source file": {"00000.py"}}},
        "examples": {"SLOP003": [{"file": "00000.py", "line": 1, "col": 1, "message": "m",
                                  "fix": None, "meta": {}, "more": 0, "snippet": [[1, "```py", True]]}]},
    }
    fake_args = argparse.Namespace(bin="/usr/bin/true", limit=500, dir=".")
    payload = results_payload({"aigcodeset": {"python": {"ai": fake_cell}}}, {"aigcodeset": "abc"},
                              DATASETS, fake_args, {"SLOP003": {"tier": "A"}}, ["x/y/z"])
    assert json.loads(json.dumps(payload))["cells"][0]["rules"] == {"SLOP003": {"files": 1, "findings": 1}}
    assert payload["cells"][0]["msgs"]["SLOP003"] == {"stray markdown code fence in source file": 1}

    print("ok")


def parse_local(spec):
    """NAME=DIR:LABEL:LANG -- a hard `SystemExit` naming the bad value on any malformed spec,
    so a typo'd `--local` surfaces here instead of a `KeyError` deep inside `build_dataset`."""
    name, sep, rest = spec.partition("=")
    parts = rest.rsplit(":", 2) if sep else []
    if not sep or not name or len(parts) != 3 or not all(parts):
        raise SystemExit(f"--local {spec!r}: expected NAME=DIR:LABEL:LANG")
    directory, label, lang = parts
    if lang not in EXT:
        raise SystemExit(f"--local {spec!r}: lang {lang!r} is not one of {sorted(EXT)}")
    return name, directory, label, lang


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--bin", default="target/release/stopslop", help="stopslop binary to lint each cell with")
    ap.add_argument("--dir", default="target/corpus",
                     help="corpus root: cache/<dataset>/ plus a rebuilt <dataset>/<label>/<lang>/ per cell")
    ap.add_argument("--limit", type=int, default=500,
                     help="rows per cell, spread-sampled; 0 = all (default 500)")
    ap.add_argument("--datasets", help="comma-separated dataset names (default: all registered)")
    ap.add_argument("--langs", help="comma-separated langs to restrict to")
    ap.add_argument("--natlangs", help="comma-separated natlangs to restrict datasets to")
    ap.add_argument("--report", help="write the report to this path instead of stdout")
    ap.add_argument("--local", action="append", default=[],
                     help="NAME=DIR:LABEL:LANG, repeatable")
    ap.add_argument("--skip-failures", action="store_true",
                     help="mark a cell 'not fetched' on exhausted retries instead of exiting")
    ap.add_argument("--fetch-only", action="store_true",
                     help="materialize every cell and write <dir>/fetched.json, then exit")
    ap.add_argument("--no-fetch", action="store_true",
                     help="score the cells <dir>/fetched.json lists; no network, no rewrite")
    ap.add_argument("--json", help="write results.json (rules, cells, per-message tallies, flagged lines)")
    ap.add_argument("--readme", default="README.md", help="README holding the rule table")
    ap.add_argument("--self-check", action="store_true", help="assert the metric math and exit")
    args = ap.parse_args()
    if args.self_check:
        return self_check()
    if args.fetch_only and args.no_fetch:
        raise SystemExit("--fetch-only and --no-fetch are opposites; pass at most one")
    args.langs = set(args.langs.split(",")) if args.langs else None
    args.natlangs = set(args.natlangs.split(",")) if args.natlangs else None

    os.makedirs(os.path.join(args.dir, "cache"), exist_ok=True)
    registry = dict(DATASETS)
    registry.update(discover_synth_datasets(args.dir))
    for spec in args.local:
        name, directory, label, lang = parse_local(spec)
        registry[name] = {
            "kind": "local", "natlang": None, "url": None, "paper": None, "license": None,
            "human_provenance": "unverified",
            "human_note": f"local corpus at {directory}, provenance unknown to this script",
            "caveats": [f"local corpus at {directory}"],
            "cells": [{"label": label, "lang": lang, "dir": directory}],
        }
    manifest = read_manifest(args) if args.no_fetch else None
    if manifest:
        # A manifest may name a dataset this invocation cannot re-derive (a --local or synth
        # cell fetched by an earlier run), so it carries that entry's registry record.
        for name, entry in manifest["datasets"].items():
            if name not in registry and entry.get("registry"):
                registry[name] = entry["registry"]
        args.limit = manifest["limit"]

    if args.datasets:
        wanted = set(args.datasets.split(","))
        missing = wanted - set(registry)
        if missing:
            raise SystemExit(f"unknown dataset(s): {sorted(missing)}")
        selected = {n: registry[n] for n in wanted}
    else:
        # Everything registered, including any synth/--local cell already folded into
        # `registry` above -- synth is auto-included only because `discover_synth_datasets`
        # already checked its dir exists, and --local is explicit opt-in by construction.
        selected = dict(registry)

    if args.natlangs:
        selected = {n: ds for n, ds in selected.items() if (ds.get("natlang") or "n/a") in args.natlangs}
    if manifest:
        selected = {n: ds for n, ds in selected.items() if n in manifest["datasets"]}

    fetched, datasets_built, revisions, not_fetched = {}, {}, {}, []
    for name in sorted(selected):
        ds = selected[name]
        if manifest:
            written = {(c["label"], c["lang"]): read_cell(cell_root(args, name, c["label"], c["lang"]), c["empty"])
                       | {"ext": c["ext"]}
                       for c in manifest["datasets"][name]["cells"]
                       if not args.langs or c["lang"] in args.langs}
            revisions[name] = manifest["datasets"][name]["revision"]
        else:
            written, skipped = fetch_dataset(name, ds, args)
            not_fetched += skipped
            if written:
                revisions[name] = dataset_revision(name, ds, args)
        if not written:
            continue
        fetched[name] = written
        if not args.fetch_only:
            datasets_built[name] = score_dataset(name, ds, written, args)

    if manifest:
        not_fetched = manifest["not_fetched"]
    if args.fetch_only:
        payload = write_manifest(args, fetched, revisions, selected, not_fetched, selected)
        cells = sum(len(d["cells"]) for d in payload["datasets"].values())
        sys.stderr.write(f"fetched {cells} cells across {len(payload['datasets'])} datasets "
                         f"-> {manifest_path(args)}\n")
        return
    if not manifest:
        write_manifest(args, fetched, revisions, selected, not_fetched, selected)

    out = build_report(datasets_built, revisions, selected, args, not_fetched)
    if args.report:
        with open(args.report, "w", encoding="utf-8") as fh:
            fh.write(out)
    else:
        sys.stdout.write(out)
    if args.json:
        rules = load_rules(args.bin, args.readme)
        payload = results_payload(datasets_built, revisions, selected, args, rules, not_fetched)
        with open(args.json, "w", encoding="utf-8") as fh:
            json.dump(payload, fh, ensure_ascii=False, sort_keys=True)
            fh.write("\n")
        sys.stderr.write(f"{args.json}: {len(payload['cells'])} cells, {len(rules)} rules\n")


if __name__ == "__main__":
    main()
