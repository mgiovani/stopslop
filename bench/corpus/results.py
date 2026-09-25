"""results.json: the same numbers the markdown report holds, plus flagged lines, per-message
tallies, rule metadata and a pooled per-rule scoreboard, for `bench/analyze_corpus.py` and
`bench/render_report.py`."""
import re
import subprocess

from .common import APPLICABLE, PROVENANCE_NOTE, RESULTS_SCHEMA
from .metrics import pct, rule_verdict
from .report import has_human, is_verified

README_RULE_ROW = re.compile(
    r"^\| (SLOP\d{3}) \| (\w+) \| (.+?) \| ([ABC]), (on|off)\s*\| (.+?) \| (.+?) \| (.+?) \|$")


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


def _cell_hits(cell, code):
    return cell["rules"].get(code, {}).get("files", 0)


def _pooled(cells, code, applicable, na_rules):
    """Files hit / files total for `code`, pooled over `cells` that apply to it -- the
    (h, a, b) building block of `scoreboard`, matching `bench/report_template.html`'s
    `pooled(code, list)`."""
    f = t = 0
    for c in cells:
        if code in applicable.get(c["lang"], ()) and code not in na_rules.get(c["dataset"], ()):
            f += _cell_hits(c, code)
            t += c["files"]
    return {"f": f, "t": t, "r": pct(f, t)}


def _dataset_order(dataset_meta_by_name, cells_by_dataset):
    """Reproduces `bench/report_template.html`'s `ORDER`: verified datasets first, then code
    over prose, then a human split over AI-only, then alphabetical -- the order `dsHits`
    lists a rule's firing datasets in."""
    def key(name):
        cs = cells_by_dataset.get(name, ())
        verified = dataset_meta_by_name[name]["verified"]
        is_code = any(c["lang"] != "prose" for c in cs)
        has_human_cell = any(c["label"] == "human" for c in cs)
        return (-int(verified), -int(is_code), -int(has_human_cell), name)
    return sorted(dataset_meta_by_name, key=key)


def scoreboard(cells, applicable, rules, na_rules, dataset_meta_by_name):
    """One row per rule code that appears in some `applicable[lang]`, pooling hit rates over
    verified-human and plain-ai cells the way `bench/report_template.html`'s `sbRows` does, so
    the page can render the scoreboard without recomputing the pooling itself.

    `lift` is `None` for both the JS's `Infinity` (human rate 0, ai rate > 0) and its `NaN`
    (both rates 0) cases -- the page already renders both as "-", and a rule with `verdict`
    "nodata" never trusts `lift` either way.
    """
    cells_by_dataset = {}
    for c in cells:
        cells_by_dataset.setdefault(c["dataset"], []).append(c)
    order = _dataset_order(dataset_meta_by_name, cells_by_dataset)

    human_cells = [c for c in cells if c["label"] == "human" and dataset_meta_by_name[c["dataset"]]["verified"]]
    ai_cells = [c for c in cells if c["label"] == "ai"]
    paired = {f"{c['dataset']}/{c['lang']}" for c in human_cells
              if any(a["dataset"] == c["dataset"] and a["lang"] == c["lang"] for a in ai_cells)}
    unpaired_human = [c for c in human_cells if f"{c['dataset']}/{c['lang']}" not in paired]

    codes = [code for code in rules if any(code in langs for langs in applicable.values())]
    rows = []
    for code in sorted(codes):
        h = _pooled(human_cells, code, applicable, na_rules)
        a = _pooled(ai_cells, code, applicable, na_rules)
        b = _pooled(unpaired_human, code, applicable, na_rules)
        lift_val = (a["r"] / h["r"]) if h["r"] else None
        ds_hits = [ds for ds in order if any(_cell_hits(c, code) > 0 for c in cells_by_dataset.get(ds, ()))]
        any_hits = sum(_cell_hits(c, code) for c in cells)
        rows.append({
            "code": code, "h": h, "a": a, "b": b, "lift": lift_val,
            "verdict": rule_verdict(h["f"], h["t"], a["f"], a["t"]),
            "ds_hits": ds_hits, "any": any_hits,
        })
    return rows


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
    dataset_meta_by_name = {name: dataset_meta(name, registry[name], datasets_built[name], revisions.get(name, "n/a"))
                             for name in sorted(datasets_built)}
    na_rules = {name: sorted(registry[name]["na_rules"]) for name in sorted(datasets_built)
                if registry[name].get("na_rules")}
    return {
        "schema": RESULTS_SCHEMA, "binary": version, "limit": args.limit,
        "provenance_note": PROVENANCE_NOTE,
        "rules": rules,
        "applicable": {lang: list(codes) for lang, codes in APPLICABLE.items()},
        "datasets": dataset_meta_by_name,
        "na_rules": na_rules,
        "not_fetched": sorted(not_fetched),
        "cells": cells,
        "scoreboard": scoreboard(cells, APPLICABLE, rules, na_rules, dataset_meta_by_name),
    }
