"""The hit-rate arithmetic shared by the markdown report, results.json's scoreboard, and
`bench/analyze_corpus.py`'s candidate measurement."""

# Judgment call, not calibrated against real precision/recall data: one more finding moves a
# ratio built from single-digit counts a long way, so rows/cells under this many hits are
# marked and read off their raw counts instead.
LOW_SUPPORT = 20
# Judgment call, not calibrated: per-generator table, a generator with fewer files than this
# reads as noise, not a measurement.
MIN_GENERATOR_FILES = 20
# Below this many pooled AI hits, one file moves the rate more than the verdict claims to show.
MIN_AI_FILES = 15
# A rule "separates" when it is both frequent enough on AI files to matter and far more
# frequent there than on verified human files; see `rule_verdict`.
SEPARATES_LIFT = 3
SEPARATES_RATE = 1


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
    """ai hit rate over human hit rate. `'inf'` when human is clean and ai is not; `'n/a'`
    when neither side fires."""
    if rate_h == 0:
        return "inf" if rate_ai > 0 else "n/a"
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
        return "n/a"
    if x == "inf":
        return "inf"
    if x == "n/a":
        return "n/a"
    return f"{x:.2f}"


def rule_verdict(h_files, h_total, a_files, a_total):
    """Canonical verdict key ('sep'/'weak'/'chance'/'inv'/'nodata') for a pooled human/ai
    hit-rate pair -- one rule fires in `*_files` of `*_total` files.

    Reproduces two call sites that compute the same math on different field names:
    `bench/report_template.html`'s `verdict(h, a)` (~lines 305-329, `h`/`a` = {f, t, r}) and
    `bench/analyze_corpus.py`'s `candidate_verdict(human, ai)` (~lines 388-400, `human`/`ai` =
    {"hit", "files", "rate"} from `pool()`). Once analyze_corpus.py is switched over it becomes:

        VERDICT_LABEL = {"nodata": "too few hits", "sep": "separates", "weak": "leans AI",
                          "inv": "leans human", "chance": "at chance"}

        def candidate_verdict(human, ai):
            return VERDICT_LABEL[rule_verdict(human["hit"], human["files"], ai["hit"], ai["files"])]

    `*_rate` is on the `pct()` 0-100 scale, matching `SEPARATES_RATE`'s units.
    """
    if a_files < MIN_AI_FILES:
        return "nodata"
    # a_files >= MIN_AI_FILES > 0 here, so a_rate > 0 and lift() can never return "n/a" (its
    # "neither side fires" case).
    ratio = lift(pct(h_files, h_total), pct(a_files, a_total))
    if ratio == "inf" or (isinstance(ratio, float) and ratio >= SEPARATES_LIFT and pct(a_files, a_total) >= SEPARATES_RATE):
        return "sep"
    if isinstance(ratio, float) and ratio >= 1.5:
        return "weak"
    if isinstance(ratio, float) and ratio < 0.67:
        return "inv"
    return "chance"
