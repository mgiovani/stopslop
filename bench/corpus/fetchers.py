"""One-off per-dataset fetchers -- everything `registry.DATASETS` points its `fetch` key at,
other than the git-based ones in `git.py`. Each returns either a flat list of (text, meta) for
a single-cell dataset, or a dict label -> list[(text, meta)] for a multi-cell one; `None` (or a
`None` value for one label) means "skipped under --skip-failures", never a crash.
"""
import ast
import csv
import json
import math
import os
import re
import sys
import urllib.error
import urllib.request
import zipfile

from .common import fetch, sample_indices, spread_labeled, spread_pair
from .git import clone_dir_for, git_clone_ref, git_files
from .hf import hf_filter_sample, hf_headers, hf_rows_sample


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
    return spread_labeled(by_label, limit)


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
    return spread_labeled(out, limit)


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
    return spread_labeled(out, limit)


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
