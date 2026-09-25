"""Hugging Face datasets-server client: `/filter` and `/rows` sampling, with the retry and
scan-fallback ladders that keep a rerun offline and byte-identical."""
import json
import math
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

from .common import RETRY_SLEEPS, _slug

HF_FILTER_URL = "https://datasets-server.huggingface.co/filter"
HF_ROWS_URL = "https://datasets-server.huggingface.co/rows"
HF_API_URL = "https://huggingface.co/api/datasets"
# `/filter` gets a short ladder because `/rows` (no server-side index) is the fallback; on
# 2026-09-05 the Droid and CodeMirage indexes stayed "loading" for over an hour.
FILTER_RETRY_SLEEPS = (5, 10, 20)
# Pages a `/rows` scan may read before giving up on a cell: 600 pages = 60k rows, enough
# for 500 rows of a 2% cell on a shuffled dataset; a dataset sorted by language yields less.
SCAN_MAX_PAGES = 600
# Windows a scan must spread a common cell over before it may stop (see `hf_rows_scan`).
SCAN_MIN_PAGES = 50


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
