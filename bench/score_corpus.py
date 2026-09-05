"""Per-rule hit rate for the registered rules across a labelled human-vs-AI corpus registry.

usage: python3 bench/score_corpus.py [options] > report.md
       python3 bench/score_corpus.py --self-check

Each registered dataset in `DATASETS` names a `kind` (how to fetch it), a natural language
axis, its source URL/license, known caveats, and the `cells` it contributes -- one cell per
(label, lang) pair actually lintable. Every fetch is cached under `<dir>/cache/<dataset>/` as
the raw page/file the network returned, so a rerun never touches the network and reproduces
byte-identical class directories; `<dir>/<dataset>/<label>/<lang>/` is rebuilt from that cache
every run (`rmtree` first, as the original AIGCodeSet-only version of this script did), one
`NNNNN.<ext>` file per sample plus an `index.json` mapping filename to `{generator, words,
...}`. Paste the relevant table into a PR body; this is a measurement tool, not a gate, so CI
never runs it and the report defaults to stdout.

The corpus lands under `target/` with numbered filenames on purpose. `paths::is_test_path`
skips the nine path-gated rules for any path holding a segment such as `tests`, `fixtures` or
`examples`, or a filename starting `test_`; `target/corpus/<dataset>/human/python/00001.py`
trips none of that. `target/` being gitignored costs nothing here, because the walk
ignore-filters only below an explicit root, and it keeps the corpus out of the dogfood run.

Rejected datasets, and why, so nobody re-proposes them:
  - SemEval-2026 Task 13: derived from Droid, already registered directly.
  - HumanVsAICode: function-level Python, redundant with AIGCodeSet/Droid/CodeMirage.
  - CSC15011: no dataset card, no stated license.
  - AICD-Bench: no per-row language column to split cells by lang.
  - MultiAIGCD: the archive it ships from is unpublished.
  - HybridCodeAuthorship: no public download under that name.
  - Whodunit: labels live in filenames, not a queryable column.
  - RAID: 21 GB, and its `/filter` endpoint errored on every query tried this session; its
    attack-robustness column is worth a phase-2 pass once `/filter` is stable.
  - DetectRL: no stated license.
  - OUTFOX: ships as pickles, not a stdlib-readable format.
  - MGTBench: distributed via Google Drive, no direct URL.
  - COLING multilingual GT detection: verified to carry no Portuguese config.
  - IberAuTexTification, MULTITuDE: gated behind a request-access form.
  - Carolina: distributed via a download script, not a stable direct URL.
  - python-docs-pt-br: `.po` gettext catalogs need a real parser this crate does not carry;
    phase 2.

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
    token = os.environ.get("HF_TOKEN")
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


def git_files(clone_dir, subdir, glob_pat, exclude=()):
    """Every file under `clone_dir/subdir` matching `glob_pat`, minus any path containing an
    `exclude` substring, sorted for determinism."""
    root = os.path.join(clone_dir, subdir)
    if not os.path.isdir(root):
        return []
    matches = []
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
    dir is ready -- `fetch_git_cell` and `fetch_rust_book` differ only in how they get there."""
    files = git_files(clone_dir, cell["subdir"], cell["glob"], ds.get("exclude", ()))
    if not files:
        if skip_failures:
            return None
        raise SystemExit(f"{ds['name']}/{cell['label']}/{cell['lang']}: 0 files under {cell['subdir']}")
    return read_text_files(clone_dir, files, limit)


def fetch_git_cell(ds, cell, limit, cache_dir, skip_failures):
    clone_dir = shared_clone_dir(cache_dir, ds["repo"], ds.get("tag"))
    if git_clone_ref(ds["repo"], ds.get("tag"), clone_dir, ds["name"], skip_failures) is None:
        return None
    return materialize_from_clone(ds, cell, clone_dir, limit, skip_failures)


def fetch_rust_book(ds, cell, limit, cache_dir, skip_failures):
    clone_dir = shared_clone_dir(cache_dir, ds["repo"], None)
    if os.path.isdir(os.path.join(clone_dir, ".git")):
        # Cache hit: git_clone_ref/git_clone_before would each no-op on this same check, so
        # skip the ls-remote round-trip that only exists to pick a tag for a fresh clone.
        cloned = clone_dir
    else:
        tags = subprocess.run(["git", "ls-remote", "--tags", ds["repo"]], capture_output=True, text=True).stdout
        year_tag = next(
            (line.rsplit("refs/tags/", 1)[1] for line in tags.splitlines() if "2021" in line and "^{}" not in line),
            None,
        )
        cloned = (
            git_clone_ref(ds["repo"], year_tag, clone_dir, ds["name"], skip_failures) if year_tag
            else git_clone_before(ds["repo"], "2022-01-01", clone_dir, ds["name"], skip_failures)
        )
    if cloned is None:
        return None
    return materialize_from_clone(ds, cell, clone_dir, limit, skip_failures)


def fetch_ghostbuster(ds, limit, cache_dir, skip_failures):
    clone_dir = shared_clone_dir(cache_dir, ds["repo"], None)
    if git_clone_ref(ds["repo"], None, clone_dir, ds["name"], skip_failures) is None:
        return None
    domains = ("essay", "reuter", "wp")
    human_items, ai_items = [], []
    for domain in domains:
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
    per_config = None if not limit else max(1, math.ceil(limit / len(configs)))
    human_items, ai_items = [], []
    for cfg in configs:
        rows = hf_rows_sample(ds["dataset"], cfg, "train", per_config, cache_dir, f"hc3/{cfg}", skip_failures)
        if rows is None:
            continue
        for row in rows:
            for h in row.get("human_answers") or []:
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
    for label_value, key in ((1, "human"), (0, "ai")):
        rows = hf_filter_sample(ds["dataset"], "default", "train", f'"label"={label_value}',
                                 limit, cache_dir, f"mage/label={label_value}", skip_failures)
        if rows is None:
            return None
        items = []
        for row in rows:
            domain, generator = parse_mage_src(row.get("src", ""))
            items.append((row["text"], {"generator": generator, "domain": domain}))
        out[key] = items
    return out


def fetch_wikipedia_pt(ds, limit, cache_dir, skip_failures):
    rows = hf_rows_sample(ds["dataset"], "default", "train", limit, cache_dir, "wikipedia-pt", skip_failures)
    if rows is None:
        return None
    return {"human": [(r["text"], {"generator": "human"}) for r in rows]}


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


def build_cell(root, items, ext, binary, dataset_name, label, lang, applicable):
    counts = write_cell(root, items, ext)
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
        "tier_a_hits": tier_a_hits, "any_hits": any_hits,
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


def dist(counts, top=8):
    ordered = sorted(counts.items(), key=lambda kv: -kv[1])
    return ", ".join(f"{k} {v}" for k, v in ordered[:top]) or "none"


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
    "codemirage": {
        "kind": "hf_filter", "natlang": None,
        "dataset": "HanxiGuo/CodeMirage", "config": "default", "split": "train",
        "text_col": "code", "generator_col": "source",
        "url": "https://huggingface.co/datasets/HanxiGuo/CodeMirage",
        "paper": None, "license": "CC-BY-NC-ND-4.0",
        "caveats": ["CC-BY-NC-ND-4.0 forbids redistributing a derivative; measurement only, "
                    "nothing from this dataset is republished here beyond aggregate counts."],
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
    "rosetta": {
        "kind": "hf_filter", "natlang": None,
        "dataset": "christopher/rosetta-code", "config": "default", "split": "train",
        "text_col": "code", "generator_col": None,
        "url": "https://huggingface.co/datasets/christopher/rosetta-code",
        "paper": None, "license": "GFDL",
        "caveats": ["Human-only: every task/language pair is a solution someone wrote for the "
                    "Rosetta Code wiki, so this dataset measures the false-positive rate alone."],
        "fetch": fetch_hf_filter_cell,
        "cells": [
            {"label": "human", "lang": "python", "generator": "human", "where": '"language_name"=\'Python\''},
            {"label": "human", "lang": "go", "generator": "human", "where": '"language_name"=\'Go\''},
            {"label": "human", "lang": "rust", "generator": "human", "where": '"language_name"=\'Rust\''},
            {"label": "human", "lang": "typescript", "generator": "human",
             "where": '"language_name"=\'TypeScript\''},
        ],
    },
    "go-std": {
        "kind": "git_tag", "natlang": None,
        "repo": "https://github.com/golang/go", "tag": "go1.17.3",
        "url": "https://github.com/golang/go", "paper": None, "license": "BSD-3-Clause",
        "caveats": ["go1.17.3 predates Copilot's June 2021 public preview cutoff era only "
                    "loosely; treat every git-tag corpus's false-positive rate as an upper "
                    "bound on how AI-assisted the tag's code could be, not a guarantee of zero."],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "go", "generator": "human",
                   "subdir": "src", "glob": "*.go", }],
        "exclude": ("testdata/", "_test.go", "/vendor/"),
    },
    "cpython-lib": {
        "kind": "git_tag", "natlang": None,
        "repo": "https://github.com/python/cpython", "tag": "v3.10.0",
        "url": "https://github.com/python/cpython", "paper": None, "license": "PSF-2.0",
        "caveats": ["Shares its clone with cpython-doc (same tag)."],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "python", "generator": "human",
                   "subdir": "Lib", "glob": "*.py"}],
        "exclude": ("/test/", "/tests/", "lib2to3/tests", "idlelib/idle_test"),
    },
    "rust-std": {
        "kind": "git_tag", "natlang": None,
        "repo": "https://github.com/rust-lang/rust", "tag": "1.57.0",
        "url": "https://github.com/rust-lang/rust", "paper": None, "license": "MIT OR Apache-2.0",
        "caveats": [],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "rust", "generator": "human",
                   "subdir": "library", "glob": "*.rs"}],
        "exclude": ("/tests/", "/benches/"),
    },
    "typescript-src": {
        "kind": "git_tag", "natlang": None,
        "repo": "https://github.com/microsoft/TypeScript", "tag": "v4.5.4",
        "url": "https://github.com/microsoft/TypeScript", "paper": None, "license": "Apache-2.0",
        "caveats": [],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "typescript", "generator": "human",
                   "subdir": "src", "glob": "*.ts"}],
        "exclude": ("/tests/", "testRunner", "harness"),
    },
    "antd-tsx": {
        "kind": "git_tag", "natlang": None,
        "repo": "https://github.com/ant-design/ant-design", "tag": "4.17.4",
        "url": "https://github.com/ant-design/ant-design", "paper": None, "license": "MIT",
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
        "caveats": ["`domain` is the HC3 config (wiki_csai, open_qa, reddit_eli5, finance, "
                    "medicine); every ai file is ChatGPT, so the per-generator table has "
                    "exactly one column here."],
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
        "caveats": ["MAGE's own license terms conflict across its source domains (it "
                    "re-publishes several licensed corpora); treat this dataset as "
                    "measurement-only, nothing republished beyond aggregate counts."],
        "fetch": fetch_mage,
        "cells": [
            {"label": "human", "lang": "prose"},
            {"label": "ai", "lang": "prose"},
        ],
    },
    "ghostbuster": {
        "kind": "git_multi", "natlang": "en",
        "repo": "https://github.com/vivek3141/ghostbuster-data",
        "url": "https://github.com/vivek3141/ghostbuster-data",
        "paper": "arXiv 2305.15047", "license": "CC-BY-3.0",
        "caveats": ["gpt_prompt*/gpt_semantic/gpt_writing variants are skipped; only the "
                    "plain gpt/ and claude/ generations are counted as `ai`."],
        "fetch": fetch_ghostbuster,
        "cells": [
            {"label": "human", "lang": "prose"},
            {"label": "ai", "lang": "prose"},
        ],
        "exclude": ("gpt_prompt", "gpt_semantic", "gpt_writing"),
    },
    "cpython-doc": {
        "kind": "git_tag", "natlang": "en",
        "repo": "https://github.com/python/cpython", "tag": "v3.10.0",
        "url": "https://github.com/python/cpython", "paper": None, "license": "PSF-2.0",
        "caveats": ["Shares its clone with cpython-lib (same tag). Written as `.rst`: the "
                    "boldface/heading-style rules (SLOP019/SLOP021) never fire on `.rst`, "
                    "only on Md/Mdx/Html, so those two rows read 0 here by construction."],
        "fetch": fetch_git_cell,
        "cells": [{"label": "human", "lang": "prose", "ext": "rst", "generator": "human",
                   "subdir": "Doc", "glob": "*.rst"}],
        "exclude": (),
    },
    "rust-book": {
        "kind": "git_tag", "natlang": "en",
        "repo": "https://github.com/rust-lang/book",
        "url": "https://github.com/rust-lang/book", "paper": None, "license": "MIT OR Apache-2.0",
        "caveats": ["No 2021 tag exists on this repo; cloned shallow-since 2020 and checked "
                    "out the last commit before 2022-01-01 on the default branch instead."],
        "fetch": fetch_rust_book,
        "cells": [{"label": "human", "lang": "prose", "generator": "human",
                   "subdir": "src", "glob": "*.md"}],
        "exclude": (),
    },
    "wetbench-pt": {
        "kind": "url_jsonl", "natlang": "pt",
        "url_tmpl": "https://huggingface.co/datasets/cs928346/WETBench/resolve/main/mgt/paragraphs_pt_{model}.jsonl",
        "url": "https://huggingface.co/datasets/cs928346/WETBench",
        "paper": None, "license": "CC-BY-NC-SA-4.0",
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
        "caveats": [],
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
        "caveats": ["Human-only: student essays for a national exam prompt."],
        "fetch": fetch_essay_br,
        "cells": [{"label": "human", "lang": "prose",
                   "url": "https://raw.githubusercontent.com/rafaelanchieta/essay/master/essay-br/essay-br.csv"}],
    },
    "wikipedia-pt": {
        "kind": "hf_rows", "natlang": "pt",
        "dataset": "TucanoBR/wikipedia-PT",
        "url": "https://huggingface.co/datasets/TucanoBR/wikipedia-PT",
        "paper": None, "license": "CC-BY-SA-3.0",
        "caveats": ["Human-only."],
        "fetch": fetch_wikipedia_pt,
        "cells": [{"label": "human", "lang": "prose"}],
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

def build_dataset(name, ds, args):
    """Fetch, materialize and lint every cell of one dataset; returns lang -> label -> cell."""
    cache_dir = os.path.join(args.dir, "cache", name)
    os.makedirs(cache_dir, exist_ok=True)
    by_lang, not_fetched = {}, []

    def store(label, lang, ext, items):
        root = os.path.join(args.dir, name, label, lang)
        if items is None:
            # A cell an earlier run did materialize would otherwise sit on disk looking like
            # part of this run's sample while the report silently omitted it.
            shutil.rmtree(root, ignore_errors=True)
            if os.path.exists(f"{root}.index.json"):
                os.remove(f"{root}.index.json")
            not_fetched.append(f"{name}/{label}/{lang}")
            sys.stderr.write(f"not fetched: {name}/{label}/{lang} (retries exhausted; rerun to fill it)\n")
            return
        cell = build_cell(root, items, ext, args.bin, name, label, lang, APPLICABLE[lang])
        by_lang.setdefault(lang, {})[label] = cell

    kind = ds["kind"]
    named_ds = ds | {"name": name}
    if kind in ("hf_filter", "url_csv", "git_tag"):
        # One fetch call per declared cell: `ds["fetch"]` takes (ds, cell, limit, cache_dir,
        # skip_failures) and returns that one cell's items (or None under --skip-failures).
        for cell in ds["cells"]:
            if args.langs and cell["lang"] not in args.langs:
                continue
            items = ds["fetch"](named_ds, cell, args.limit, cache_dir, args.skip_failures)
            store(cell["label"], cell["lang"], cell.get("ext", EXT[cell["lang"]]), items)
    elif kind in ("hf_rows", "url_jsonl", "url_json", "git_multi"):
        # One fetch call for the whole dataset: `ds["fetch"]` takes (ds, limit, cache_dir,
        # skip_failures) and returns every declared cell's items keyed by label at once,
        # because these sources split human/ai out of one shared row stream or clone.
        if args.langs and not any(c["lang"] in args.langs for c in ds["cells"]):
            return by_lang, not_fetched
        result = ds["fetch"](named_ds, args.limit, cache_dir, args.skip_failures)
        for cell in ds["cells"]:
            items = None if result is None else result.get(cell["label"])
            store(cell["label"], cell["lang"], cell.get("ext", EXT[cell["lang"]]), items)
    elif kind == "local":
        for cell in ds["cells"]:
            if args.langs and cell["lang"] not in args.langs:
                continue
            items = fetch_local_cell(cell["dir"], args.limit, cell["label"])
            store(cell["label"], cell["lang"], cell.get("ext", EXT[cell["lang"]]), items)
    else:
        raise SystemExit(f"{name}: unknown dataset kind {kind!r}")
    return by_lang, not_fetched


def dataset_revision(name, ds, args):
    if ds["kind"] == "local":
        return "local"
    if ds["kind"] in ("git_tag", "git_multi"):
        if ds.get("tag"):
            return ds["tag"]
        cache_dir = os.path.join(args.dir, "cache", name)
        clone_dir = shared_clone_dir(cache_dir, ds["repo"], None)
        return git_head_sha(clone_dir) if os.path.isdir(clone_dir) else "unknown (not cloned)"
    hf_id = ds.get("dataset")
    if not hf_id:
        return "n/a"
    cache_dir = os.path.join(args.dir, "cache", name)
    os.makedirs(cache_dir, exist_ok=True)
    return hf_revision(hf_id, cache_dir, args.skip_failures)


# --------------------------------------------------------------------------------------
# Report.
# --------------------------------------------------------------------------------------

def generator_table(cell, applicable, na_rules=()):
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


def lang_summary_matrix(lang, datasets_built, registry):
    """rows = APPLICABLE[lang] + the two union rows; columns = each dataset with a cell of
    this lang, H%/A% (blank when a side is absent, n/a for a dataset's na_rules)."""
    cols = []  # (dataset_name, header_suffix, by_label)
    for name, by_lang in datasets_built.items():
        if lang not in by_lang:
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


def dataset_meta_bullets(name, ds, by_lang, revision, args):
    total_files = sum(c["files"] for by_label in by_lang.values() for c in by_label.values())
    lines = [f"### {name}", ""]
    if ds.get("url"):
        lines.append(f"- Source: {ds['url']}")
    if ds.get("paper"):
        lines.append(f"- Paper: {ds['paper']}")
    lines.append(f"- License: {ds.get('license') or 'not stated on the dataset card'}")
    lines.append(f"- Natural language: {ds.get('natlang') or 'n/a (code)'}")
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


def build_report(datasets_built, revisions, registry, args, not_fetched=()):
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
    lines.append("## Summary, by lang")
    lines.append("")
    for lang in ("python", "go", "rust", "typescript", "tsx", "prose"):
        if args.langs and lang not in args.langs:
            continue
        matrix = lang_summary_matrix(lang, datasets_built, registry)
        if matrix:
            lines.append(matrix)
            lines.append("")
            if lang == "rust":
                lines.append(
                    "Plain ai Rust in this table, when present, comes only from the "
                    "synthesized `synth-rust` cell; no real-world dataset registered here "
                    "ships plain machine-generated Rust (see the droid caveats below)."
                )
                lines.append("")
    lines.append("## Per-dataset detail")
    lines.append("")
    for name in sorted(datasets_built):
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
    ap.add_argument("--self-check", action="store_true", help="assert the metric math and exit")
    args = ap.parse_args()
    if args.self_check:
        return self_check()
    args.langs = set(args.langs.split(",")) if args.langs else None
    args.natlangs = set(args.natlangs.split(",")) if args.natlangs else None

    os.makedirs(os.path.join(args.dir, "cache"), exist_ok=True)
    registry = dict(DATASETS)
    registry.update(discover_synth_datasets(args.dir))
    for spec in args.local:
        name, directory, label, lang = parse_local(spec)
        registry[name] = {
            "kind": "local", "natlang": None, "url": None, "paper": None, "license": None,
            "caveats": [f"local corpus at {directory}"],
            "cells": [{"label": label, "lang": lang, "dir": directory}],
        }

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

    datasets_built, revisions, not_fetched = {}, {}, []
    for name in sorted(selected):
        ds = selected[name]
        by_lang, skipped = build_dataset(name, ds, args)
        not_fetched += skipped
        if not by_lang:
            continue
        datasets_built[name] = by_lang
        revisions[name] = dataset_revision(name, ds, args)

    out = build_report(datasets_built, revisions, selected, args, not_fetched)
    if args.report:
        with open(args.report, "w", encoding="utf-8") as fh:
            fh.write(out)
    else:
        sys.stdout.write(out)


if __name__ == "__main__":
    main()
