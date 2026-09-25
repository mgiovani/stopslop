"""Per-dataset fetch and score orchestration, plus the `fetched.json` manifest that lets
`--fetch-only` and `--no-fetch` run as separate `just` recipes."""
import json
import os
import sys

from .common import APPLICABLE, EXT, MANIFEST, RESULTS_SCHEMA
from .fetchers import fetch_local_cell
from .git import clone_dir_for, git_head_sha
from .hf import hf_revision
from .lint import cell_materialized, read_cell, score_cell, write_cell
from .registry import CELL_KINDS, DATASET_KINDS, DATASETS


def cell_root(args, name, label, lang):
    return os.path.join(args.dir, name, label, lang)


def _cell_lang_wanted(args, lang):
    """`args.langs` restricts which cells `fetch_dataset` touches; shared by the CELL_KINDS
    and "local" branches below so the same filter applies to hf_filter/git cells and --local
    directories alike, instead of two copies of the same `continue` guard."""
    return not args.langs or lang in args.langs


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
            if not _cell_lang_wanted(args, cell["lang"]):
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
            if not _cell_lang_wanted(args, cell["lang"]):
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
