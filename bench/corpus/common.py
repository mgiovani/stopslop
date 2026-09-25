"""Shared constants and the dependency-free helpers every other `bench/corpus` module builds
on: deterministic sampling, an atomic download primitive, and the applicability/provenance
vocabulary the scorer, the report and `bench/analyze_corpus.py` all read.
"""
import hashlib
import os
import shutil
import time
import urllib.error
import urllib.request

# datasets-server builds its DuckDB index lazily and answers 500 meanwhile; a six-step ladder
# (~3 minutes) lost nine Droid and CodeMirage cells in one 500-per-cell run on 2026-09-05.
RETRY_SLEEPS = (5, 10, 20, 40, 60, 60, 90, 120, 120, 120)
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

EXT = {"python": "py", "go": "go", "rust": "rs", "typescript": "ts", "tsx": "tsx", "prose": "md"}
JS_PROXY = "javascript written as .ts; SLOP007 cannot fire"

# Applicable rules per lang, hand-transcribed from each RuleDef.langs in src/rules/<group>/*.rs.
# SLOP010 needs a manifest no cell carries, so it never fires; left out on purpose.
# ponytail: add a langs column to `--list-rules`, read it here once that exists.
APPLICABLE = {
    "python": (
        "SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP006", "SLOP008",
        "SLOP009", "SLOP037", "SLOP039", "SLOP040", "SLOP042", "SLOP043", "SLOP045",
    ),
    "go": (
        "SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP005", "SLOP008",
        "SLOP009", "SLOP037", "SLOP039", "SLOP042", "SLOP043",
    ),
    "rust": (
        "SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP005", "SLOP008",
        "SLOP009", "SLOP037", "SLOP039", "SLOP042", "SLOP043", "SLOP045",
    ),
    "typescript": (
        "SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP005", "SLOP007", "SLOP008",
        "SLOP009", "SLOP037", "SLOP038", "SLOP039", "SLOP040", "SLOP042", "SLOP043",
        "SLOP045",
    ),
    "tsx": (
        "SLOP001", "SLOP002", "SLOP003", "SLOP004", "SLOP005", "SLOP007", "SLOP008",
        "SLOP009", "SLOP037", "SLOP038", "SLOP039", "SLOP040", "SLOP042", "SLOP043",
        "SLOP045",
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
    "tables leave it out rather than show a rule that structurally cannot score. SLOP044 "
    "(HTML) and SLOP046-SLOP048 (images) are left out for the plainer reason that no "
    "registered dataset carries an HTML or image cell. SLOP045 reads only files of 60+ "
    "non-blank lines, which excludes nearly every file in the snippet and contest corpora "
    "(2.6% of AIGCodeSet's human `.py` files clear it): its 0% there means it barely ran, "
    "not that it found nothing. The pinned standard libraries clear it (67% of rust-std)."
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


def spread_labeled(by_label, limit):
    """{label: ...}, each list independently spread-sampled to `limit` -- the shared tail of
    every fetcher that splits one source into more than two labeled lists (`fetch_beemo`,
    `fetch_pan25`, `fetch_faidset`); `spread_pair` is the two-label case of this."""
    return {label: [items[i] for i in sample_indices(len(items), limit)] for label, items in by_label.items()}


def spread_pair(human_items, ai_items, limit):
    """{"human": ..., "ai": ...}, each independently spread-sampled to `limit` -- the shared
    tail of every fetcher that splits one source into a human list and an ai list."""
    return spread_labeled({"human": human_items, "ai": ai_items}, limit)


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


def _slug(text):
    return hashlib.sha1(text.encode("utf-8")).hexdigest()[:16]


def read_text_files(clone_dir, files, limit):
    idx = sample_indices(len(files), limit)
    items = []
    for i in idx:
        rel = files[i]
        with open(os.path.join(clone_dir, rel), encoding="utf-8", errors="replace") as fh:
            text = fh.read()
        items.append((text, {"generator": "human", "domain": rel}))
    return items
