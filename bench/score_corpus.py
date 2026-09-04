"""Per-rule hit rate for the Python rules on a labelled human-vs-machine corpus.

usage: python3 bench/score_corpus.py [--bin PATH] [--dir DIR] [--limit N] > report.md
       python3 bench/score_corpus.py --self-check

Downloads AIGCodeSet (arXiv 2412.16594, CDLA-Permissive-2.0), writes every `code`
cell as one file per class, lints each class directory once with `--format json
--stats`, and prints a markdown table: hit rate on human files, hit rate on machine
files, precision at a 1:1 prior, and findings per KLoC. Paste the table into the PR
body; issue #39 asks for a measurement tool, not a gate, so CI never runs this and
the report defaults to stdout.

Issue #39 names three other datasets. CoDET-M4 publishes only
`dataset_without_comments.parquet`, 457 MB with the comments stripped, which is most
of what the code rules read, and it would need a parquet reader this crate does not
have. The DROID resource suite and HybridCodeAuthorship have no public download under
those names.

The corpus lands in `target/` with numbered filenames on purpose. `paths::is_test_path`
skips the nine path-gated rules for any path holding a segment such as `tests`,
`fixtures` or `examples`, or a filename starting `test_`; `target/corpus/human/00001.py`
trips none of that, while `bench/fixtures/` would silently lose nine of thirteen rules.
`target/` being gitignored costs nothing here, because the walk ignore-filters only
below an explicit root, and it keeps the corpus out of the dogfood run.
"""
import argparse
import csv
import json
import os
import shutil
import subprocess
import sys
import urllib.request

BASE = "https://huggingface.co/datasets/basakdemirok/AIGCodeSet/resolve/main/data"
SOURCES = (
    ("human", f"{BASE}/human_selected_dataset.csv", "0"),
    ("machine", f"{BASE}/created_dataset_with_llms.csv", "1"),
)

# Rules whose `langs` names `Lang::Python`: `lang::CODE_LANGS` in src/lang.rs plus
# the narrower lists on SLOP006, SLOP009, SLOP040, SLOP043. SLOP010 sits in
# UNSCOREABLE below rather than here.
APPLICABLE = (
    "SLOP001",
    "SLOP002",
    "SLOP003",
    "SLOP004",
    "SLOP006",
    "SLOP008",
    "SLOP009",
    "SLOP037",
    "SLOP039",
    "SLOP040",
    "SLOP042",
    "SLOP043",
)
UNSCOREABLE = (
    "SLOP010 names `Lang::Python` too, but its check needs a dependency manifest the "
    "corpus has no reason to carry, so it can never fire and the table leaves it out."
)
# One more finding moves a ratio built from single-digit counts a long way, so rows
# under this many hits are marked and read off their raw counts instead.
LOW_SUPPORT = 20


def fetch(url, dest):
    """Download once, and land the file atomically.

    A download killed halfway leaves a truncated CSV that the next run would treat as
    cached, then report a smaller corpus with nothing to say why.
    """
    if not os.path.exists(dest):
        part = f"{dest}.part"
        with urllib.request.urlopen(url, timeout=60) as resp, open(part, "wb") as out:
            out.write(resp.read())
        os.replace(part, dest)
    return dest


def materialize(csv_path, out_dir, label, limit):
    """Write each `code` cell as one numbered file and return counts for the report.

    `newline=""` is load-bearing rather than habit: the `code` column carries embedded
    newlines inside quoted fields, and without it the csv module splits one submission
    across several rows. Filenames come from the enumerate index because
    `submission_id` repeats 1,260 times on the machine side, once per generator.
    """
    # Files a longer previous run left behind would be walked and counted.
    shutil.rmtree(out_dir, ignore_errors=True)
    os.makedirs(out_dir)
    written, empty, outcomes, models = 0, 0, {}, {}
    with open(csv_path, encoding="utf-8", newline="") as fh:
        for row in csv.DictReader(fh):
            if row["label"] != label:
                raise SystemExit(f"{csv_path}: expected label {label}, saw {row['label']!r}")
            code = row["code"]
            if not code.strip():
                empty += 1
                continue
            if limit and written >= limit:
                break
            with open(os.path.join(out_dir, f"{written:05d}.py"), "w", encoding="utf-8", newline="\n") as out:
                out.write(code)
            outcomes[row["status_in_folder"]] = outcomes.get(row["status_in_folder"], 0) + 1
            models[row["LLM"]] = models.get(row["LLM"], 0) + 1
            written += 1
    return {"files": written, "empty": empty, "outcomes": outcomes, "models": models}


def run_lint(binary, root, expected):
    """Lint one class directory and refuse to report numbers the walk did not produce.

    An all-zero table is indistinguishable from a clean result, so a walk that reached
    fewer files than were written is a hard failure rather than a footnote.
    """
    env = dict(os.environ, STOPSLOP_NO_UPDATE_CHECK="1", CI="1")
    proc = subprocess.run(
        [binary, root, "--format", "json", "--stats", "--no-config", "--select", "ALL"],
        capture_output=True,
        text=True,
        env=env,
    )
    if proc.returncode not in (0, 1):
        raise SystemExit(f"{binary} exited {proc.returncode}\n{proc.stderr}")
    payload = json.loads(proc.stdout)
    stats = payload["stats"]
    if stats["files"] != expected or stats["skipped"]:
        raise SystemExit(f"{root}: walk saw {stats} for {expected} files")
    return payload["findings"], stats


def tally(findings):
    """Count files hit per rule code, plus raw findings per rule code."""
    hits, total = {}, {}
    for f in findings:
        hits.setdefault(f["code"], set()).add(f["path"])
        total[f["code"]] = total.get(f["code"], 0) + 1
    return hits, total


def precision(rate_h, rate_m):
    """Precision at a 1:1 class prior, exact from the two rates, `None` when both are 0.

    Balancing the classes by subsampling would compute the same number while throwing
    away human files, and the human side is the half that carries the false-positive
    evidence worth having.
    """
    if rate_h + rate_m == 0:
        return None
    return rate_m / (rate_h + rate_m)


def pct(n, total):
    return 100.0 * n / total if total else 0.0


def per_kloc(findings, lines):
    return 1000.0 * findings / lines if lines else 0.0


def dist(counts):
    return ", ".join(f"{k} {v}" for k, v in sorted(counts.items(), key=lambda kv: -kv[1]))


def report(meta, binary, limit):
    human, machine = meta["human"], meta["machine"]
    n_h, n_m = human["files"], machine["files"]
    hits_h, total_h = tally(human["findings"])
    hits_m, total_m = tally(machine["findings"])
    strays = sorted((set(hits_h) | set(hits_m)) - set(APPLICABLE))
    if strays:
        raise SystemExit(f"{strays} fired but is not in APPLICABLE; update bench/score_corpus.py")
    version = subprocess.run([binary, "--version"], capture_output=True, text=True).stdout.strip()

    limited = [f"- **--limit {limit}: a biased head slice, not a reported number**"] if limit else []
    lines = [
        "## Per-rule hit rate on AIGCodeSet",
        "",
        *limited,
        f"- Corpus: AIGCodeSet, arXiv 2412.16594, CDLA-Permissive-2.0, {BASE}",
        f"- Files: {n_h} human (AtCoder submissions), {n_m} machine (LLM answers to the same problems)",
        f"- Blank `code` cells dropped: {human['empty']} human, {machine['empty']} machine",
        f"- Lines: {human['stats']['lines']} human, {machine['stats']['lines']} machine",
        f"- Human outcomes: {dist(human['outcomes'])}",
        f"- Machine outcomes: {dist(machine['outcomes'])}",
        f"- Generators: {dist(machine['models'])}",
        f"- Binary: {version}, run as `stopslop <dir> --format json --stats --no-config --select ALL`",
        "",
        "| rule | human hits (= FPR, see below) | machine hits | precision @1:1 | human /KLoC | machine /KLoC |",
        "|---|---|---|---|---|---|",
    ]
    rows = []
    for code in APPLICABLE:
        h, m = len(hits_h.get(code, ())), len(hits_m.get(code, ()))
        rate_h, rate_m = pct(h, n_h), pct(m, n_m)
        p = precision(rate_h, rate_m)
        rows.append(
            (
                rate_m,
                code,
                f"| {code} | {h} ({rate_h:.2f}%) | {m} ({rate_m:.2f}%) "
                f"| {'--' if p is None else f'{p:.2f}'}{'*' if 0 < h + m < LOW_SUPPORT else ''} "
                f"| {per_kloc(total_h.get(code, 0), human['stats']['lines']):.2f} "
                f"| {per_kloc(total_m.get(code, 0), machine['stats']['lines']):.2f} |",
            )
        )
    lines += [row for _, _, row in sorted(rows, key=lambda r: (-r[0], r[1]))]

    any_h = len(set().union(*hits_h.values()))
    any_m = len(set().union(*hits_m.values()))
    any_p = precision(pct(any_h, n_h), pct(any_m, n_m))
    lines.append(
        f"| **any rule** | {any_h} ({pct(any_h, n_h):.2f}%) | {any_m} ({pct(any_m, n_m):.2f}%) "
        f"| {'--' if any_p is None else f'{any_p:.2f}'} | | |"
    )
    lines += [
        "",
        "### How to read this",
        "",
        "- `human hits` is the share of human files where the rule fired. Under "
        "\"the rule firing predicts machine-authored\" that share **is** the false-positive rate.",
        "- `precision @1:1` assumes one machine file per human file, which is a property of "
        "this table and not of any repository. It is a re-expression of the two hit rates and "
        "carries no information they do not; the raw counts are there so a one-versus-zero "
        "result is not mistaken for a perfect rule. `--` means neither side fired.",
        "- Findings per KLoC only mean something read against the hit rate: a high hit rate "
        "next to a modest KLoC is a rule that is weakly present everywhere, and a low hit rate "
        "next to a high KLoC is a rule that fires hard in a few files. KLoC alone cannot tell "
        "those apart.",
        "- A `*` marks a precision built from fewer than "
        f"{LOW_SUPPORT} hits in total. Read those off the counts, not the ratio.",
        "- The `any rule` row is the OR of the twelve, so its precision tracks the noisiest "
        "member rather than the best one.",
        "",
        "### What this does not show",
        "",
        f"- Twelve of the registered rules can produce a number on a `.py` corpus. {UNSCOREABLE} "
        "The remaining rules declare other languages; every prose rule, the formatting-uniformity "
        "rules SLOP018-SLOP021 among them, needs a labelled prose corpus instead.",
        "- The machine side is post-extraction code: the dataset authors kept the code block and "
        "dropped the surrounding answer. SLOP001-SLOP004 exist to catch pasted assistant output, "
        "so their numbers here are a floor, measured at the hardest case.",
        "- Both sides are competitive-programming solutions: short, single-file, few abstractions. "
        "SLOP037, SLOP039 and SLOP040 have little to bite, and the false-positive rate measured "
        "here does not transfer to application code.",
        "- AtCoder is Japanese and many human files carry Japanese comments. The rules with an "
        "English lexicon cannot match those, which lowers the human hit rate and raises precision "
        "for exactly those rules.",
        "- The machine side is entirely code that failed; the human side is one third accepted. "
        "The dataset authors matched the outcome buckets deliberately, and nothing is filtered "
        "here beyond blank `code` cells, counted above.",
        "- The generators are three 2024-era models under one prompt style, and a small share of "
        "the human side is Python 2, which parses with error nodes.",
        "- A hit rate is evidence that the tells appear in that corpus. It is not a claim that any "
        "file is AI-written, and this crate ships no such claim.",
    ]
    return "\n".join(lines) + "\n"


def self_check():
    assert precision(0.0, 0.0) is None
    assert precision(4.0, 4.0) == 0.5
    assert precision(0.0, 4.0) == 1.0
    assert pct(1, 4) == 25.0 and pct(0, 0) == 0.0
    print("ok")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--bin", default="target/release/stopslop")
    ap.add_argument("--dir", default="target/corpus")
    ap.add_argument("--limit", type=int, default=0, help="first N rows per class, 0 for all; a biased smoke test, never a reported number")
    ap.add_argument("--self-check", action="store_true", help="assert the metric math and exit")
    args = ap.parse_args()
    if args.self_check:
        return self_check()

    cache = os.path.join(args.dir, "cache")
    os.makedirs(cache, exist_ok=True)
    meta = {}
    for name, url, label in SOURCES:
        csv_path = fetch(url, os.path.join(cache, os.path.basename(url)))
        root = os.path.join(args.dir, name)
        meta[name] = materialize(csv_path, root, label, args.limit)
        if not meta[name]["files"]:
            raise SystemExit(f"{csv_path}: no usable rows, so there is nothing to score")
        findings, stats = run_lint(args.bin, root, meta[name]["files"])
        meta[name].update(findings=findings, stats=stats)

    sys.stdout.write(report(meta, args.bin, args.limit))


if __name__ == "__main__":
    main()
