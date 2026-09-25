"""Materialize one cell to disk, lint it with the stopslop binary, and shape its findings into
the hit sets, message tallies and flagged-line examples the report and results.json read."""
import json
import os
import re
import shutil
import subprocess

from .common import sample_indices

# Flagged lines kept per rule per cell in results.json, spread across the flagged files; the
# HTML page shows them side by side, so more than a screen's worth per split is unread.
EXAMPLES_PER_RULE = 14
# Judgment call, not calibrated: enough lines each side of a flagged line to show the
# enclosing statement in the HTML example card without approaching a whole file.
SNIPPET_CONTEXT = 2
# Judgment call, not calibrated: long enough that clip() rarely needs to cut a normal source
# line, short enough that one long line does not dominate an HTML example card.
LINE_MAX = 230
# Rules whose backticked span is a library API, not a user identifier: keep it, it is the
# panel entry. Collapsing it elsewhere is what keeps the breakdown to one row per message.
KEEP_BACKTICKS = ("SLOP037", "SLOP038")


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
    try:
        payload = json.loads(proc.stdout)
    except json.JSONDecodeError as err:
        raise SystemExit(f"{binary} {root}: printed no parseable JSON: {err}\n{proc.stderr}") from err
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
            "update APPLICABLE in bench/corpus/common.py"
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
