"""The markdown report: per-dataset tables, per-lang summary matrices, and the provenance
divider between verified and unverified human splits."""
import subprocess

from .common import APPLICABLE, PROVENANCE_NOTE, UNSCOREABLE_NOTE, VERIFIED
from .metrics import LOW_SUPPORT, MIN_GENERATOR_FILES, density, dist, fmt_ratio, lift, pct, precision, precision_at_prior


def generator_table(cell, applicable, na_rules):
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


def lang_summary_matrix(lang, datasets_built, registry, only):
    """rows = APPLICABLE[lang] + the two union rows; columns = each dataset with a cell of
    this lang, H%/A% (blank when a side is absent, n/a for a dataset's na_rules). `only`
    restricts the columns to a set of dataset names, which is how the verified and unverified
    halves of the report get their own matrices."""
    cols = []  # (dataset_name, header_suffix, by_label)
    for name, by_lang in datasets_built.items():
        if lang not in by_lang or name not in only:
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


def has_human(ds):
    return any(c["label"] == "human" for c in ds.get("cells", ()))


def is_verified(ds):
    """A dataset's human split counts toward the pooled human rates only when its provenance
    clears the bar; an AI-only dataset has no human split to qualify and is never pooled."""
    return has_human(ds) and ds.get("human_provenance") in VERIFIED


def split_by_provenance(names, registry):
    """(verified, unverified) dataset names: the divider the report and the HTML page draw."""
    verified = [n for n in names if is_verified(registry[n])]
    return verified, [n for n in names if n not in set(verified)]


def dataset_meta_bullets(name, ds, by_lang, revision, args):
    total_files = sum(c["files"] for by_label in by_lang.values() for c in by_label.values())
    lines = [f"### {name}", ""]
    if ds.get("url"):
        lines.append(f"- Source: {ds['url']}")
    if ds.get("paper"):
        lines.append(f"- Paper: {ds['paper']}")
    lines.append(f"- License: {ds.get('license') or 'not stated on the dataset card'}")
    lines.append(f"- Natural language: {ds.get('natlang') or 'n/a (code)'}")
    if ds.get("generator_years"):
        lines.append(f"- Generators: {ds['generator_years']}")
    if has_human(ds):
        lines.append(f"- Human split: {ds.get('human_provenance') or 'unverified'} "
                     f"({ds.get('human_note') or 'provenance not recorded'})")
    lines.append(f"- Revision: {revision}")
    lines.append(f"- Files fetched: {total_files}")
    for lang, by_label in sorted(by_lang.items()):
        lines.append(f"  - {lang}:")
        for label, cell in sorted(by_label.items()):
            lines.append(f"    - {label}: {cell['files']} ({cell['empty']} blank dropped).")
        for label, cell in sorted(by_label.items()):
            if label == "human":
                continue
            generators = {m.get("generator", "unknown") for m in cell["index"].values()}
            if len(generators) > 1:
                counts = {}
                for m in cell["index"].values():
                    g = m.get("generator", "unknown")
                    counts[g] = counts.get(g, 0) + 1
                lines.append(f"    - {label} generators: {dist(counts)}.")
    proxies = {c["proxy"] for c in ds.get("cells", []) if c.get("proxy")}
    for p in sorted(proxies):
        lines.append(f"- {p}")
    for c in ds.get("caveats", []):
        lines.append(f"- {c}")
    if ds.get("na_rules"):
        lines.append(f"- `n/a` rules on this dataset: {', '.join(sorted(ds['na_rules']))}")
    return lines


def _report_header(datasets_built, revisions, args, not_fetched):
    version = subprocess.run([args.bin, "--version"], capture_output=True, text=True).stdout.strip()
    lines = [
        "## Per-rule hit rate across the corpus registry",
        "",
        f"- Binary: {version}, run as `stopslop <dir> --format json --stats --no-config --select ALL`",
        f"- `--limit`: {args.limit or 'all (0)'}" + (" (a deterministic spread sample, never a reported number, when non-zero and below a dataset's full size)" if args.limit else ""),
    ]
    for name in sorted(datasets_built):
        lines.append(f"- {name} revision: {revisions.get(name, 'n/a')}")
    if not_fetched:
        lines.append(
            "- **Not fetched** (retries exhausted under `--skip-failures`; absent from every "
            f"table below, rerun to fill them from cache): {', '.join(sorted(not_fetched))}"
        )
    return lines


def _how_to_read():
    return [
        "",
        "### How to read this",
        "",
        "- `human hits` is the share of human files where the rule fired. Under \"the rule "
        "firing predicts AI-authored\" that share **is** the false-positive rate.",
        "- `precision @1:1` assumes one ai file per human file, a property of this table's "
        "construction, not of any repository; it re-expresses the two hit rates and carries "
        "no information they do not. `precision @prior` instead uses the raw counts actually "
        "fetched, so it reflects this run's `--limit` and each cell's real population. `n/a` "
        "means neither side fired.",
        "- `lift` is the ai hit rate divided by the human hit rate: `inf` means the rule is "
        "clean on human files here and not on ai ones; `n/a` means neither side fired.",
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


def _provenance_table(datasets_built, registry):
    lines = [
        "### Which human splits count as human",
        "",
        "A split is read as human only when its text is dated 2019 or earlier "
        "(`pinned-2019`) or was written by identified people under controlled conditions "
        "(`verified-authors`). Every other split is `unverified` and reported below a divider.",
        "",
        "| dataset | human split | source and years | generators |",
        "|---|---|---|---|",
    ]
    for name in sorted(datasets_built):
        ds = registry[name]
        prov = ds.get("human_provenance") or ("ai only" if not has_human(ds) else "unverified")
        lines.append(f"| {name} | {prov} | {ds.get('human_note') or 'n/a'} | {ds.get('generator_years') or 'n/a'} |")
    lines.append("")
    return lines


def _lang_summaries(names, heading, datasets_built, registry, args):
    out = [heading, ""]
    for lang in ("python", "go", "rust", "typescript", "tsx", "prose"):
        if args.langs and lang not in args.langs:
            continue
        matrix = lang_summary_matrix(lang, datasets_built, registry, only=set(names))
        if matrix:
            out += [matrix, ""]
            if lang == "rust":
                out += [
                    "Plain ai Rust in this table, when present, comes only from the "
                    "synthesized `synth-rust` cell; no real-world dataset registered here "
                    "ships plain machine-generated Rust (see the droid caveats below).",
                    "",
                ]
    return out if len(out) > 2 else []


def _per_dataset_detail(verified, unverified, datasets_built, registry, revisions, args):
    lines = ["## Per-dataset detail", ""]
    for name in verified + unverified:
        if unverified and name == unverified[0]:
            lines += ["---", "", PROVENANCE_NOTE, ""]
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
    return lines


def build_report(datasets_built, revisions, registry, args, not_fetched):
    lines = _report_header(datasets_built, revisions, args, not_fetched)
    lines += _how_to_read()
    verified, unverified = split_by_provenance(sorted(datasets_built), registry)
    lines += _provenance_table(datasets_built, registry)
    lines += _lang_summaries(verified, "## Summary, by lang: verified human splits", datasets_built, registry, args)
    if unverified:
        lines += ["---", "", PROVENANCE_NOTE, ""]
        lines += _lang_summaries(unverified, "## Summary, by lang: unverified human splits", datasets_built, registry, args)
    lines += _per_dataset_detail(verified, unverified, datasets_built, registry, revisions, args)
    return "\n".join(lines).rstrip() + "\n"
