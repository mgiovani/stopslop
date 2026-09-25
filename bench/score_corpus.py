"""Per-rule hit rate for the registered rules across a labelled human-vs-AI corpus registry.

usage: python3 bench/score_corpus.py --fetch-only --skip-failures   # network: materialize cells
       python3 bench/score_corpus.py --no-fetch --report bench/corpus_report.md --json target/corpus/results.json
       python3 bench/score_corpus.py [options] > report.md            # both steps in one run
       python3 bench/score_corpus.py --self-check

Each registered dataset in `DATASETS` (see `bench/corpus/registry.py`) names a `kind` (how to
fetch it), a natural language axis, its source URL/license, known caveats, and the `cells` it
contributes -- one cell per (label, lang) pair actually lintable. Every fetch is cached under
`<dir>/cache/<dataset>/` as the raw page/file the network returned, so a rerun never touches
the network and reproduces byte-identical class directories; `<dir>/<dataset>/<label>/<lang>/`
is rebuilt from that cache every fetch (`rmtree` first, as the original AIGCodeSet-only
version of this script did), one `NNNNN.<ext>` file per sample plus an `index.json` mapping
filename to `{generator, words, ...}`. `--fetch-only` stops after writing the cells and a
`fetched.json` manifest; `--no-fetch` scores whatever that manifest lists without touching the
network, so the two halves can run as separate `just` recipes. Paste the relevant table into a
PR body; this is a measurement tool, not a gate, so CI never runs it and the report defaults to
stdout.

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
"""
import argparse
import json
import os
import sys
import unittest

from corpus.common import EXT
from corpus.lint import read_cell
from corpus.orchestrate import (
    cell_root, dataset_revision, fetch_dataset, manifest_path, read_manifest, score_dataset,
    write_manifest,
)
from corpus.registry import DATASETS, discover_synth_datasets
from corpus.report import build_report
from corpus.results import load_rules, results_payload


def self_check():
    """--self-check: run the `bench/tests/test_score_corpus.py` suite and print "ok" on
    success, keeping `just corpus-self-check`'s stdout/exit-code contract from when this
    function held the assertions itself."""
    here = os.path.dirname(os.path.abspath(__file__))
    suite = unittest.defaultTestLoader.discover(os.path.join(here, "tests"), top_level_dir=here)
    result = unittest.TextTestRunner(verbosity=0).run(suite)
    if not result.wasSuccessful():
        raise SystemExit(1)
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
