"""Deterministic analysis over the corpus `bench/score_corpus.py` materialized and scored.

usage: python3 bench/analyze_corpus.py --report target/corpus/analysis.md --json target/corpus/analysis.json
       python3 bench/analyze_corpus.py --only candidates
       python3 bench/analyze_corpus.py --self-check

Three passes, none of which calls a model:

  candidates  compile every `[[candidate]]` in bench/candidates.toml and measure it on the
              cells, so a proposed tell is a number before it is a rule.
  messages    split each rule's hit rate by which message fired, so "SLOP030 fires on 23% of
              human prose" becomes "23%, and four fifths of it is the `this` opener".
  phrases     the n-grams that appear in a far larger share of AI files than of verified human
              files, which is where the next candidate comes from.

Everything is measured against **verified** human splits: a dataset whose human side postdates
the 2019 bar (see `score_corpus.human_provenance`) is reported separately and never pooled
into a human rate, a lift or a precision. AI splits other than plain `ai` (paraphrased,
refined, adversarial, polished, edited) are robustness variants and are likewise reported
separately, never pooled.

A hit rate here is evidence that a shape appears in a corpus. It is never a claim that a file
was AI-written, and a candidate that separates two splits is a hypothesis worth a fixture,
not a rule.
"""
import argparse
import json
import math
import os
import re
import sys
import tomllib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from score_corpus import (  # noqa: E402  (sys.path is set above)
    APPLICABLE, EXT, VERIFIED, clip, lift, pct, precision, sample_indices,
)

# A candidate needs this many AI files before its rate is worth reading; below it, one file
# moves the number by more than the difference the candidate claims to show.
MIN_AI_FILES = 15
# Phrase differential: how many n-grams to keep per lang, and how many example lines each.
TOP_PHRASES = 150
PHRASE_EXAMPLES = 3
# Longest n-gram considered. Beyond four words the surviving phrases are whole sentences from
# one document, which the per-file support floor already rejects.
MAX_N = 4
# Flagged lines quoted per split for a candidate.
CAND_EXAMPLES = 6
# A candidate "separates" when it is both frequent enough on AI files to matter and far more
# frequent there than on verified human files. Same shape as the report's verdict thresholds.
SEPARATES_LIFT = 3.0
SEPARATES_RATE = 1.0
# Per-message rows below this share of the rule's files fold into one "rarer" row.
# Percent on the pct() 0-100 scale, like SEPARATES_RATE: 2% is 2.0, and 0.02 folded nothing.
MESSAGE_FLOOR = 2.0

SCOPES = ("line", "comment", "file-first-line", "last-block")
FIRST_LINES = 3

CODE_LANGS = tuple(lang for lang in EXT if lang != "prose")
LINE_COMMENT = {"python": "#", "go": "//", "rust": "//", "typescript": "//", "tsx": "//"}

WORD = re.compile(r"[^\W\d_]+(?:'[^\W\d_]+)*", re.UNICODE)
# GPT-2 byte-pair markers for a leading space and a newline. MAGE passed part of its text
# through a tokenizer and back, so left alone these rank as their own vocabulary.
BPE_MARKERS = re.compile(r"[ĠĊ]")
# Stopword-only n-grams ("of the", "que a") carry no tell; a phrase mixing a stopword with a
# content word is kept, because that is what an idiom looks like.
STOPWORDS = frozenset("""
a an and are as at be been but by can could do does for from had has have he her his i if in
is it its me my no not of on or our she so than that the their them then there these they
this to too was we were what when which who will with would you your
a as ao aos com como da das de do dos e em na nas no nos o os ou para pela pelas pelo pelos
por que se sem seu seus sua suas um uma umas uns é são foi ser tem têm mais mas muito também
""".split())


def blank_strings(text, lang):
    """Replace string-literal bodies with spaces, keeping every newline, so a comment scan
    never reads a sentence that lives inside a quoted string.

    Comments are tracked in the same pass, one-directionally: once a line comment (`#`, `//`)
    or a block comment (`/* */`) opens, a quote character inside it is left alone instead of
    opening a string, because a comment's contents can never be code. Comment text itself
    passes through unchanged here; `comment_lines` is what reads it.

    ponytail: quote-and-comment state machine only, no escapes beyond backslash and no
    raw-string or template-literal awareness; a Rust `r#"..."#`, a Go raw string containing a
    backslash right before its closing backtick, a JS/TS regex literal holding `//`, or a
    nested Rust `/* */` can each desynchronize it. Move to tree-sitter if a candidate ever
    turns on one of those differences.
    """
    marker = LINE_COMMENT.get(lang)
    block = marker == "//"
    out, quote, i = [], None, 0
    triple = lang == "python"
    in_line_comment = in_block_comment = False
    while i < len(text):
        ch = text[i]
        if in_line_comment:
            if ch == "\n":
                in_line_comment = False
            out.append(ch)
            i += 1
            continue
        if in_block_comment:
            if text.startswith("*/", i):
                out.append("*/")
                i += 2
                in_block_comment = False
            else:
                out.append(ch)
                i += 1
            continue
        if quote:
            if ch == "\\" and i + 1 < len(text):
                out.append("  " if text[i + 1] != "\n" else " \n")
                i += 2
                continue
            if text.startswith(quote, i):
                out.append(" " * len(quote))
                i += len(quote)
                quote = None
                continue
            out.append("\n" if ch == "\n" else " ")
            i += 1
            continue
        if marker and text.startswith(marker, i):
            in_line_comment = True
            out.append(marker)
            i += len(marker)
            continue
        if block and text.startswith("/*", i):
            in_block_comment = True
            out.append("/*")
            i += 2
            continue
        if triple and (text.startswith('"""', i) or text.startswith("'''", i)):
            quote = text[i:i + 3]
            out.append("   ")
            i += 3
            continue
        if ch in "\"'" or (ch == "`" and lang in ("go", "typescript", "tsx")):
            quote = ch
            out.append(" ")
            i += 1
            continue
        out.append(ch)
        i += 1
    return "".join(out)


def comment_lines(text, lang):
    """`(line_no, comment_text)` for every comment line, marker stripped.

    Docstrings fall out with the strings, which is deliberate: a docstring is documentation
    the language records, and the candidates scoped to `comment` are about the marginal notes
    a model leaves beside code.
    """
    marker = LINE_COMMENT.get(lang)
    if marker is None:
        return []
    blanked = blank_strings(text, lang).split("\n")
    out, in_block = [], False
    for n, raw in enumerate(blanked, 1):
        line = raw
        if in_block:
            end = line.find("*/")
            body = line if end < 0 else line[:end]
            out.append((n, body.lstrip().lstrip("*").strip()))
            if end >= 0:
                line, in_block = line[end + 2:], False
            else:
                continue
        while lang != "python" and "/*" in line:
            start = line.index("/*")
            end = line.find("*/", start + 2)
            body = line[start + 2:] if end < 0 else line[start + 2:end]
            out.append((n, body.lstrip().lstrip("*").strip()))
            if end < 0:
                in_block = True
                line = ""
                break
            line = line[:start] + line[end + 2:]
        at = line.find(marker)
        if at >= 0:
            out.append((n, line[at + len(marker):].strip()))
    return [(n, body) for n, body in out if body]


def paragraph_blocks(text):
    """`(line_no, block_text)` for each blank-line-separated block, with the line the block
    starts on. A prose file with no blank line is one block, which is what the AI answers
    pasted out of an API payload look like."""
    blocks, current, start = [], [], 1
    for n, line in enumerate(text.split("\n"), 1):
        if line.strip():
            if not current:
                start = n
            current.append(line)
            continue
        if current:
            blocks.append((start, "\n".join(current)))
            current = []
    if current:
        blocks.append((start, "\n".join(current)))
    return blocks


def scope_lines(text, lang, scope):
    """`(line_no, text)` pairs a candidate of this scope is measured against."""
    if scope == "comment":
        return comment_lines(text, lang)
    lines = [(n, line) for n, line in enumerate(text.split("\n"), 1) if line.strip()]
    if scope == "line":
        return lines
    if scope == "file-first-line":
        return lines[:FIRST_LINES]
    if scope == "last-block":
        blocks = paragraph_blocks(text)
        return [blocks[-1]] if blocks else []
    raise SystemExit(f"unknown scope {scope!r}; expected one of {SCOPES}")


def load_candidates(path):
    with open(path, "rb") as fh:
        data = tomllib.load(fh)
    for cand in data.get("candidate", ()):
        missing = {"name", "kind", "langs", "scope", "regex", "relation", "tier"} - set(cand)
        if missing:
            raise SystemExit(f"{path}: candidate {cand.get('name')!r} is missing {sorted(missing)}")
        if cand["scope"] not in SCOPES:
            raise SystemExit(f"{path}: candidate {cand['name']!r} has scope {cand['scope']!r}")
        if cand["kind"] not in ("code", "prose"):
            raise SystemExit(f"{path}: candidate {cand['name']!r} has kind {cand['kind']!r}")
    return data


def candidate_langs(cand):
    langs = cand["langs"].strip()
    if langs == "code":
        return set(CODE_LANGS)
    if langs == "prose":
        return {"prose"}
    unknown = {x.strip() for x in langs.split(",")} - set(EXT)
    if unknown:
        raise SystemExit(f"candidate {cand['name']!r}: unknown lang(s) {sorted(unknown)}")
    return {x.strip() for x in langs.split(",")}


def compile_candidate(cand):
    return re.compile(cand["regex"], re.IGNORECASE if cand.get("ignorecase") else 0)


# Candidates spell Brazilian Portuguese "pt-BR" (the crate's own NatLang name); datasets record
# it as "pt" (score_corpus.py's `natlang`). Everything else passes through unchanged.
NATLANG_ALIASES = {"pt-br": "pt"}


def candidate_natlangs(cand):
    """The candidate's declared natlangs, normalized to dataset spelling, or `None` when it
    declares none and is therefore unrestricted."""
    raw = cand.get("natlangs")
    if not raw:
        return None
    return {NATLANG_ALIASES.get(x.strip().lower(), x.strip().lower()) for x in raw.split(",")}


def cell_matches_natlangs(results, cell, natlangs):
    """Whether `cell` may count toward a candidate that declared `natlangs`.

    A cell whose dataset states no natural language (every code dataset: `natural_language`
    is "n/a (code)") is never excluded here -- there is nothing to compare against, and a code
    candidate's `natlangs` documents the language its regex is written for, not a per-file
    label the corpus carries. Only a cell whose dataset states "en"/"pt" and it isn't the one
    requested is dropped, which is the dilution this function exists to stop: a candidate
    declaring `natlangs = "en"` was, until this check existed, measured over the pt-BR prose
    cells too.
    """
    if natlangs is None:
        return True
    stated = natlang_of(results, cell)
    return stated is None or stated in natlangs


# --------------------------------------------------------------------------------------
# Corpus access.
# --------------------------------------------------------------------------------------

def cell_dir(root, cell):
    return os.path.join(root, cell["dataset"], cell["label"], cell["lang"])


def cell_files(root, cell):
    directory = cell_dir(root, cell)
    if not os.path.isdir(directory):
        raise SystemExit(f"{directory}: cell missing; rerun just corpus-fetch")
    for name in sorted(f for f in os.listdir(directory) if not f.startswith(".")):
        with open(os.path.join(directory, name), encoding="utf-8", errors="replace") as fh:
            yield name, fh.read()


def is_verified_human(results, cell):
    return cell["label"] == "human" and results["datasets"][cell["dataset"]]["human_provenance"] in VERIFIED


def cell_class(results, cell):
    """Which pool a cell belongs to: verified human, unverified human, plain AI, or an AI
    robustness variant. Nothing outside `human` and `ai` is ever pooled."""
    if cell["label"] == "human":
        return "human" if is_verified_human(results, cell) else "human-unverified"
    return "ai" if cell["label"] == "ai" else "ai-variant"


def selected_cells(results, args, langs=None):
    out = []
    for cell in results["cells"]:
        if langs and cell["lang"] not in langs:
            continue
        if args.langs and cell["lang"] not in args.langs:
            continue
        if args.datasets and cell["dataset"] not in args.datasets:
            continue
        out.append(cell)
    return out


# --------------------------------------------------------------------------------------
# Pass a: candidates.
# --------------------------------------------------------------------------------------

# A dataset naming one of these in its na_rules had its comments stripped by the publisher
# (codet_m4, naples-code), so a `scope = "comment"` candidate can never match its cells.
COMMENT_RULES = ("SLOP042", "SLOP043")


def cell_has_comments(results, cell):
    na = set(results["na_rules"].get(cell["dataset"], ()))
    return not na.intersection(COMMENT_RULES)


def measure_candidates(results, root, candidates, args):
    """One walk over the corpus, evaluating every candidate whose lang scope includes the
    cell; a file is read once and its scope lines are computed once per scope."""
    compiled = [(c, compile_candidate(c), candidate_langs(c), candidate_natlangs(c)) for c in candidates]
    tallies = {c["name"]: {} for c in candidates}
    for cell in selected_cells(results, args):
        comments_ok = cell_has_comments(results, cell)
        active = [(c, rx) for c, rx, langs, nats in compiled
                  if cell["lang"] in langs and cell_matches_natlangs(results, cell, nats)
                  and (c["scope"] != "comment" or comments_ok)]
        if not active:
            continue
        key = (cell["dataset"], cell["label"], cell["lang"])
        for c, _rx in active:
            tallies[c["name"]][key] = {"files": cell["files"], "hit": 0, "occurrences": 0, "examples": []}
        for name, text in cell_files(root, cell):
            by_scope = {}
            for c, rx in active:
                lines = by_scope.get(c["scope"])
                if lines is None:
                    lines = by_scope[c["scope"]] = scope_lines(text, cell["lang"], c["scope"])
                found = [(n, line) for n, line in lines if rx.search(line)]
                if not found:
                    continue
                row = tallies[c["name"]][key]
                row["hit"] += 1
                row["occurrences"] += len(found)
                if len(row["examples"]) < CAND_EXAMPLES:
                    n, line = found[0]
                    row["examples"].append({"file": name, "line": n, "text": clip(line.strip(), 1)[0]})
    return tallies


def pool(results, tally, wanted_class):
    files = hit = 0
    for (dataset, label, lang), row in tally.items():
        cell = {"dataset": dataset, "label": label, "lang": lang}
        if cell_class(results, cell) != wanted_class:
            continue
        files += row["files"]
        hit += row["hit"]
    return {"files": files, "hit": hit, "rate": pct(hit, files)}


def candidate_verdict(human, ai):
    if ai["hit"] < MIN_AI_FILES:
        return "too few hits"
    # ai["hit"] >= MIN_AI_FILES > 0 here, so ai["rate"] > 0 and lift() can never return "--"
    # (its "neither side fires" case).
    ratio = lift(human["rate"], ai["rate"])
    if ratio == "inf" or (ratio >= SEPARATES_LIFT and ai["rate"] >= SEPARATES_RATE):
        return "separates"
    if isinstance(ratio, float) and ratio >= 1.5:
        return "leans AI"
    if isinstance(ratio, float) and ratio < 0.67:
        return "leans human"
    return "at chance"


def candidate_rows(results, root, data, args):
    tallies = measure_candidates(results, root, data.get("candidate", ()), args)
    rows = []
    for cand in data.get("candidate", ()):
        tally = tallies[cand["name"]]
        human = pool(results, tally, "human")
        ai = pool(results, tally, "ai")
        splits = [
            {"dataset": dataset, "label": label, "lang": lang, "class": cell_class(
                results, {"dataset": dataset, "label": label, "lang": lang}),
             "files": row["files"], "hit": row["hit"], "occurrences": row["occurrences"],
             "rate": pct(row["hit"], row["files"]), "examples": row["examples"]}
            for (dataset, label, lang), row in sorted(tally.items()) if row["files"]
        ]
        ratio = lift(human["rate"], ai["rate"])
        rows.append({
            "candidate": {k: v for k, v in cand.items()},
            "measured": {
                "human": human, "ai": ai,
                "human_unverified": pool(results, tally, "human-unverified"),
                "ai_variant": pool(results, tally, "ai-variant"),
                "lift": ratio if isinstance(ratio, str) else round(ratio, 2),
                "precision": precision(human["rate"], ai["rate"]),
                "verdict": candidate_verdict(human, ai),
                "splits": splits,
            },
        })
    return rows


# --------------------------------------------------------------------------------------
# Pass b: per-message breakdown.
# --------------------------------------------------------------------------------------

def message_rows(results, args):
    """For each rule, which of its messages fired and how the files carrying them split
    between verified human and plain AI cells.

    Pooling does not require a dataset to hold both splits: under the 2019 human bar no code
    dataset does, so for a code rule the human side is the pinned checkouts and the AI side is
    a different corpus. That is a topic difference as well as an authorship one, and the
    report says so rather than dropping every code rule to zero.
    """
    cells = selected_cells(results, args)
    pools = {"human": {}, "ai": {}}
    # Denominators are per rule, not per pool: a prose rule never runs on the code cells, so
    # counting them would divide its rate by files it could not have fired on.
    totals = {"human": {}, "ai": {}}
    for cell in cells:
        klass = cell_class(results, cell)
        if klass not in pools:
            continue
        na = set(results["na_rules"].get(cell["dataset"], ()))
        for code in results["applicable"].get(cell["lang"], ()):
            if code not in na:
                totals[klass][code] = totals[klass].get(code, 0) + cell["files"]
        for code, by_msg in cell["msgs"].items():
            if code in na:
                continue
            for msg, files in by_msg.items():
                bucket = pools[klass].setdefault(code, {})
                bucket[msg] = bucket.get(msg, 0) + files
    rows = []
    for code in sorted(set(pools["human"]) | set(pools["ai"])):
        ht, at = totals["human"].get(code, 0), totals["ai"].get(code, 0)
        msgs = []
        for msg in sorted(set(pools["human"].get(code, {})) | set(pools["ai"].get(code, {}))):
            h = pools["human"].get(code, {}).get(msg, 0)
            a = pools["ai"].get(code, {}).get(msg, 0)
            msgs.append({"message": msg, "human": h, "ai": a,
                         "human_rate": pct(h, ht), "ai_rate": pct(a, at)})
        msgs.sort(key=lambda m: (-m["ai"], -m["human"], m["message"]))
        keep = [m for m in msgs if max(m["human_rate"], m["ai_rate"]) >= MESSAGE_FLOOR] or msgs[:1]
        rows.append({"code": code, "human_files": ht, "ai_files": at,
                     "messages": keep, "folded": len(msgs) - len(keep)})
    return rows


# --------------------------------------------------------------------------------------
# Pass c: phrase differential.
# --------------------------------------------------------------------------------------

def cell_text_lines(text, lang):
    """The lines a phrase count should read: prose as written, code through its comments,
    because an identifier is not a phrase anybody chose."""
    if lang == "prose":
        return [line for line in text.split("\n") if line.strip()]
    return [body for _n, body in comment_lines(text, lang)]


def tokenize(line):
    return [w.lower() for w in WORD.findall(BPE_MARKERS.sub(" ", line))]


def file_ngrams(lines, n):
    """The distinct n-grams in one file, as a set: a phrase repeated in a document counts
    once, so a single verbose file cannot carry a phrase into the table."""
    out = set()
    for line in lines:
        words = tokenize(line)
        for i in range(len(words) - n + 1):
            out.add(tuple(words[i:i + n]))
    return out


def natlang_of(results, cell):
    stated = results["datasets"][cell["dataset"]].get("natural_language") or ""
    return stated if stated in ("en", "pt") else None


def paired_cells(results, args, langs, natlang=None):
    """Cells from (dataset, lang) pairs that hold both a verified human split and a plain AI
    split.

    Topic control, and the reason the phrase pass is not run over every cell: a dataset with
    only one side contributes its subject matter to the differential and nothing else, so a
    corpus of agent pull requests would rank `github`, `codex` and `merge` as AI style. Within
    a pair, both splits answer the same prompts or come from the same repositories.
    """
    by_pair = {}
    for cell in selected_cells(results, args, langs=langs):
        if natlang and natlang_of(results, cell) != natlang:
            continue
        by_pair.setdefault((cell["dataset"], cell["lang"]), []).append(cell)
    out = []
    for cells in by_pair.values():
        classes = {cell_class(results, c) for c in cells}
        if "human" in classes and "ai" in classes:
            out += [c for c in cells if cell_class(results, c) in ("human", "ai")]
    return out


def lang_paired_cells(results, args, langs):
    """Verified human and plain AI cells of the same corpus lang, from any dataset.

    The fallback for code, where no dataset clears the human bar on both sides: the pinned
    2019 checkouts have no AI split and every dataset that does was collected later. Topic is
    therefore not controlled here, and the report says so.
    """
    out = [c for c in selected_cells(results, args, langs=langs)
           if cell_class(results, c) in ("human", "ai")]
    by_lang = {}
    for cell in out:
        by_lang.setdefault(cell["lang"], set()).add(cell_class(results, cell))
    return [c for c in out if by_lang[c["lang"]] == {"human", "ai"}]


def phrase_pass(results, root, cells):
    """n-gram presence counts over the verified-human and plain-AI halves of `cells`.

    Support is monotone in `n` (every occurrence of a 3-gram contains its 2-gram prefix), so
    each pass only extends the phrases that survived the previous one. That prune is what
    keeps four passes over ~20k files inside a few seconds.
    """
    docs = []  # (class, dataset, filename, lines) per file
    for cell in cells:
        klass = cell_class(results, cell)
        for name, text in cell_files(root, cell):
            docs.append((klass, cell["dataset"], name, cell_text_lines(text, cell["lang"])))
    totals = {"human": sum(1 for d in docs if d[0] == "human"), "ai": sum(1 for d in docs if d[0] == "ai")}
    if not totals["ai"]:
        return {}, totals
    # Every length that clears the floor stays: "in conclusion" and "in conclusion the" are
    # different tells. `kept` is only the prune, sound because a phrase cannot appear in more
    # files than its own prefix does.
    surviving, kept = {}, None
    for n in range(1, MAX_N + 1):
        counts = {}
        for klass, _dataset, _name, lines in docs:
            for gram in file_ngrams(lines, n):
                if kept is not None and gram[:-1] not in kept:
                    continue
                slot = counts.setdefault(gram, [0, 0])
                slot[0 if klass == "human" else 1] += 1
        kept = {gram for gram, (_h, a) in counts.items() if a >= MIN_AI_FILES}
        if not kept:
            break
        surviving.update({gram: counts[gram] for gram in kept})
    return surviving, totals


def phrase_rows(results, root, cells, label, topic_matched):
    counts, totals = phrase_pass(results, root, cells)
    rows = []
    for gram, (h, a) in counts.items():
        if a < MIN_AI_FILES or all(w in STOPWORDS for w in gram):
            continue
        h_rate, a_rate = pct(h, totals["human"]), pct(a, totals["ai"])
        # Laplace smoothing on the human side: a phrase absent from every human file would
        # otherwise sort by nothing but its AI count.
        ratio = a_rate / (h_rate + pct(1, max(1, totals["human"])))
        rows.append({"phrase": " ".join(gram), "human": h, "ai": a,
                     "human_rate": h_rate, "ai_rate": a_rate, "ratio": round(ratio, 2)})
    rows.sort(key=lambda r: (-r["ratio"], -r["ai"], r["phrase"]))
    return {"group": label, "topic_matched": topic_matched, "cells": cells,
            "human_files": totals["human"], "ai_files": totals["ai"], "phrases": rows[:TOP_PHRASES]}


def phrase_examples(results, root, groups):
    """Attach up to `PHRASE_EXAMPLES` real lines to each kept phrase; a second walk, because
    the counting pass keeps no text."""
    for g in groups:
        phrases = {p["phrase"]: [] for p in g["phrases"]}
        if not phrases:
            continue
        patterns = {p: re.compile(r"\b" + r"\s+".join(re.escape(w) for w in p.split()) + r"\b", re.IGNORECASE)
                    for p in phrases}
        for cell in g["cells"]:
            if cell_class(results, cell) != "ai":
                continue
            pending = [p for p, seen in phrases.items() if len(seen) < PHRASE_EXAMPLES]
            if not pending:
                break
            for _name, text in cell_files(root, cell):
                for line in cell_text_lines(text, cell["lang"]):
                    # tokenize() (and so the counting pass) reads a phrase through this same
                    # substitution; searching the raw line here could never match a phrase
                    # whose count came from a BPE-marked MAGE line.
                    clean = BPE_MARKERS.sub(" ", line)
                    for p in list(pending):
                        if len(phrases[p]) >= PHRASE_EXAMPLES:
                            pending.remove(p)
                            continue
                        if patterns[p].search(clean):
                            phrases[p].append(clip(clean.strip(), 1)[0])
        for p in g["phrases"]:
            p["examples"] = phrases[p["phrase"]]
    for g in groups:
        del g["cells"]
    return groups


def phrase_groups(results, root, args):
    """One differential per natural language for prose, plus one for code comments.

    Prose pairs within a dataset, so both sides answer the same prompts. Code cannot: no
    dataset clears the human bar on both sides, so its human side is the pinned 2019
    checkouts and its AI side is somebody else's corpus, and the ranking carries their topic
    difference as well as their style difference.
    """
    groups = []
    for nat, label in (("en", "prose, English"), ("pt", "prose, Portuguese")):
        cells = paired_cells(results, args, {"prose"}, natlang=nat)
        if cells:
            groups.append(phrase_rows(results, root, cells, label, True))
    code = lang_paired_cells(results, args, set(CODE_LANGS))
    if code:
        groups.append(phrase_rows(results, root, code, "code comments", False))
    return [g for g in groups if g["phrases"]]


# --------------------------------------------------------------------------------------
# Report.
# --------------------------------------------------------------------------------------

def fmt_lift(value):
    return value if isinstance(value, str) else f"{value:.1f}"


def fmt_rate(hit, files, rate):
    return f"{rate:.2f}% ({hit}/{files})"


# Verdict category first, lift only breaking ties inside it. A data-starved candidate with a
# clean human side reads "inf", so sorting on lift first put it above real "leans AI" rows.
VERDICT_RANK = {"separates": 0, "leans AI": 1, "at chance": 2, "leans human": 3, "too few hits": 4}


def verdict_sort_key(row):
    lift_value = row["measured"]["lift"]
    if lift_value == "inf":
        as_float = math.inf
    elif isinstance(lift_value, float):
        as_float = lift_value
    else:
        as_float = -math.inf
    return VERDICT_RANK[row["measured"]["verdict"]], -as_float


def candidates_section(rows, data):
    separating = [r for r in rows if r["measured"]["verdict"] == "separates"]
    lines = ["## Candidate tells", "",
             f"{len(separating)} of {len(rows)} candidates separate the splits at lift >= "
             f"{SEPARATES_LIFT:.0f} and an AI rate of at least {SEPARATES_RATE:.0f}%. "
             "Rates pool verified human splits against plain AI splits only.", "",
             "| candidate | scope | langs | verified human | plain AI | lift | verdict |",
             "|---|---|---|---|---|---|---|"]
    for row in sorted(rows, key=verdict_sort_key):
        c, m = row["candidate"], row["measured"]
        lines.append(
            f"| {c['name']} | {c['scope']} | {c['langs']} | "
            f"{fmt_rate(m['human']['hit'], m['human']['files'], m['human']['rate'])} | "
            f"{fmt_rate(m['ai']['hit'], m['ai']['files'], m['ai']['rate'])} | "
            f"{fmt_lift(m['lift'])} | {m['verdict']} |")
    lines.append("")
    for row in rows:
        c, m = row["candidate"], row["measured"]
        lines += [f"### {c['name']}", "",
                  f"- Verdict: **{m['verdict']}**, lift {fmt_lift(m['lift'])}"
                  + (f", precision at a 1:1 mix {m['precision']:.2f}" if m["precision"] is not None else ""),
                  f"- Scope: {c['scope']} on {c['langs']}"
                  + (f", natlangs {c['natlangs']}" if c.get("natlangs") else ""),
                  f"- Relation: {c['relation']} (would open at Tier {c['tier']})",
                  f"- Regex: `{c['regex']}`",
                  f"- Unverified human splits: "
                  f"{fmt_rate(m['human_unverified']['hit'], m['human_unverified']['files'], m['human_unverified']['rate'])}"
                  f"; AI robustness variants: "
                  f"{fmt_rate(m['ai_variant']['hit'], m['ai_variant']['files'], m['ai_variant']['rate'])}",
                  ""]
        if c.get("notes"):
            lines += [c["notes"].strip(), ""]
        lines += ["| split | class | files | hit | rate |", "|---|---|---|---|---|"]
        for s in sorted(m["splits"], key=lambda s: -s["rate"]):
            if not s["hit"]:
                continue
            lines.append(f"| {s['dataset']}/{s['label']}/{s['lang']} | {s['class']} | "
                         f"{s['files']} | {s['hit']} | {s['rate']:.2f}% |")
        quiet = [s for s in m["splits"] if not s["hit"]]
        if quiet:
            lines.append(f"| {len(quiet)} other splits | | | 0 | 0.00% |")
        lines.append("")
        for s in sorted(m["splits"], key=lambda s: -s["rate"])[:4]:
            if not s["examples"]:
                continue
            lines.append(f"Lines from {s['dataset']}/{s['label']}/{s['lang']}:")
            lines.append("")
            for ex in s["examples"]:
                lines.append(f"- `{ex['text']}`")
            lines.append("")
    if data.get("miss"):
        lines += ["## Rules that missed", "",
                  "| rule | what it missed | fix idea |", "|---|---|---|"]
        for m in data["miss"]:
            lines.append(f"| {m['rule']} | {m['what_it_missed']} | {m['fix_idea']} |")
        lines.append("")
    return lines


def messages_section(rows):
    lines = ["## What each rule's message actually was", "",
             "A rule's hit rate hides which of its messages did the firing. Rates are files "
             "carrying that message over all files in the pool.", ""]
    for row in rows:
        head = f"### {row['code']}"
        lines += [head, "", "| message | verified human | plain AI |", "|---|---|---|"]
        for m in row["messages"]:
            lines.append(f"| {m['message']} | {m['human_rate']:.2f}% ({m['human']}) | "
                         f"{m['ai_rate']:.2f}% ({m['ai']}) |")
        if row["folded"]:
            plural = "message" if row["folded"] == 1 else "messages"
            lines.append(f"| {row['folded']} rarer {plural} | | |")
        lines.append("")
    return lines


def phrases_section(groups):
    lines = ["## Phrases far more common in AI files", "",
             f"Every n-gram up to {MAX_N} words present in at least {MIN_AI_FILES} plain-AI "
             "files, ranked by how much more of the AI pool carries it than of the verified "
             "human pool. Prose is counted as written, one table per natural language; code "
             "is counted through its comments. This is where the next candidate comes from, "
             "and a high ratio here is a hypothesis, not a rule.", ""]
    for g in groups:
        if not g["phrases"]:
            continue
        lines += [f"### {g['group']} ({g['ai_files']} AI files, {g['human_files']} verified human)", ""]
        lines += ["Both sides come from the same datasets, so the two halves answer the same "
                  "prompts and a phrase that ranks here is not just this corpus's subject "
                  "matter.", ""] if g["topic_matched"] else [
                  "No dataset clears the human bar on both sides for code, so the human half "
                  "is the pinned 2019 checkouts and the AI half is a different corpus. Topic "
                  "differs as well as authorship: read a domain word here as evidence about "
                  "the corpora, not about models.", ""]
        lines += ["| phrase | AI | verified human | ratio |", "|---|---|---|---|"]
        for p in g["phrases"]:
            lines.append(f"| {p['phrase']} | {p['ai_rate']:.2f}% ({p['ai']}) | "
                         f"{p['human_rate']:.2f}% ({p['human']}) | {p['ratio']:.1f} |")
        lines.append("")
    return lines


def build_analysis_report(payload):
    lines = ["# Corpus analysis", "",
             f"- Corpus: {payload['cells']} cells, {payload['limit'] or 'all'} files per cell",
             f"- Verified human splits: {', '.join(payload['verified']) or 'none'}",
             f"- Unverified human splits (never pooled): {', '.join(payload['unverified']) or 'none'}",
             "", payload["provenance_note"], ""]
    if payload.get("candidates"):
        lines += candidates_section(payload["candidates"], payload["candidates_data"])
    if payload.get("messages"):
        lines += messages_section(payload["messages"])
    if payload.get("phrases"):
        lines += phrases_section(payload["phrases"])
    if payload["candidates_data"].get("paper_caveats"):
        lines += ["## Paper caveats", ""]
        lines += [f"- {c}" for c in payload["candidates_data"]["paper_caveats"]] + [""]
    if payload["candidates_data"].get("dropped"):
        lines += ["## Measured and dropped", ""]
        lines += [f"- {d}" for d in payload["candidates_data"]["dropped"]] + [""]
    return "\n".join(lines).rstrip() + "\n"


# --------------------------------------------------------------------------------------
# Self-check + CLI.
# --------------------------------------------------------------------------------------

def self_check():
    blanked = blank_strings('x = "a # b"  # real', "python")
    assert blanked == "x =          # real" and len(blanked) == len('x = "a # b"  # real')
    assert blank_strings('s = """\nline\n"""\n', "python").count("\n") == 3
    assert comment_lines("# lead\nx = 1  # trail\n", "python") == [(1, "lead"), (2, "trail")]
    assert comment_lines('x = "// not a comment"\n// yes\n', "typescript") == [(2, "yes")]
    assert comment_lines("/* one\n * two */\nlet x = 1\n", "typescript") == [(1, "one"), (2, "two")]
    assert comment_lines("code\n", "prose") == []

    # An apostrophe inside a comment must not open a string and swallow every comment after
    # it: the defect that lost 30%+ of the comments in 230 of 499 cpython-lib files.
    assert comment_lines("x = 1  # Django's default apps\nx = 2\n# real comment\n", "python") == [
        (1, "Django's default apps"), (3, "real comment")]
    assert comment_lines("let s = 1  // it's fine\nlet t = 2\n// real comment\n", "typescript") == [
        (1, "it's fine"), (3, "real comment")]
    assert comment_lines("/* it's a note */\nlet x = 1\n// after\n", "typescript") == [
        (1, "it's a note"), (3, "after")]
    # Go raw strings use backticks; a `//` inside one is not a comment.
    assert comment_lines('const usage = `see https://x.io for // details`\n// real\n', "go") == [(2, "real")]

    assert paragraph_blocks("a\n\nb\nc\n") == [(1, "a"), (3, "b\nc")]
    assert scope_lines("a\nb\nc\nd\n", "prose", "file-first-line") == [(1, "a"), (2, "b"), (3, "c")]
    assert scope_lines("first\n\nlast one\n", "prose", "last-block") == [(3, "last one")]
    assert scope_lines("only\n", "prose", "line") == [(1, "only")]

    assert candidate_langs({"langs": "code", "name": "x"}) == set(CODE_LANGS)
    assert candidate_langs({"langs": "prose", "name": "x"}) == {"prose"}
    assert candidate_langs({"langs": "python, go", "name": "x"}) == {"python", "go"}

    assert candidate_natlangs({"natlangs": "en"}) == {"en"}
    assert candidate_natlangs({"natlangs": "pt-BR"}) == {"pt"}
    assert candidate_natlangs({"natlangs": "en, pt-BR"}) == {"en", "pt"}
    assert candidate_natlangs({}) is None
    nat_results = {"datasets": {"en-ds": {"natural_language": "en"}, "pt-ds": {"natural_language": "pt"},
                                "code-ds": {"natural_language": "n/a (code)"}}}
    assert cell_matches_natlangs(nat_results, {"dataset": "en-ds"}, {"en"}) is True
    assert cell_matches_natlangs(nat_results, {"dataset": "pt-ds"}, {"en"}) is False, \
        "a prose candidate scoped to natlangs=en must not count a pt-BR dataset's cells"
    assert cell_matches_natlangs(nat_results, {"dataset": "code-ds"}, {"en"}) is True, \
        "a code dataset states no natural language, so a natlangs restriction cannot exclude it"
    assert cell_matches_natlangs(nat_results, {"dataset": "pt-ds"}, None) is True

    assert tokenize("It's a test, isn't it?") == ["it's", "a", "test", "isn't", "it"]
    assert tokenize("ĠconclusionĊthe end") == ["conclusion", "the", "end"]
    assert file_ngrams(["a b a b"], 2) == {("a", "b"), ("b", "a")}
    # phrase_examples searches lines through this same substitution, or a phrase counted from
    # a BPE-marked MAGE line could never find an example to quote.
    assert re.compile(r"\bthe end\b").search(BPE_MARKERS.sub(" ", "ĠconclusionĊthe end"))

    assert candidate_verdict({"rate": 0.0, "hit": 0, "files": 10}, {"rate": 0.0, "hit": 0, "files": 10}) == "too few hits"
    assert candidate_verdict({"rate": 0.0, "hit": 0, "files": 100}, {"rate": 5.0, "hit": 50, "files": 1000}) == "separates"
    assert candidate_verdict({"rate": 2.0, "hit": 20, "files": 1000}, {"rate": 8.0, "hit": 80, "files": 1000}) == "separates"
    assert candidate_verdict({"rate": 4.0, "hit": 40, "files": 1000}, {"rate": 4.0, "hit": 40, "files": 1000}) == "at chance"
    assert candidate_verdict({"rate": 9.0, "hit": 90, "files": 1000}, {"rate": 2.0, "hit": 20, "files": 1000}) == "leans human"

    # A "too few hits" row can still carry lift "inf" (clean human side, a handful of AI
    # hits); it must not outrank a real "leans AI" row in the report.
    fake_rows = [
        {"measured": {"verdict": "too few hits", "lift": "inf"}},
        {"measured": {"verdict": "leans AI", "lift": 2.5}},
        {"measured": {"verdict": "separates", "lift": 5.0}},
    ]
    assert [r["measured"]["verdict"] for r in sorted(fake_rows, key=verdict_sort_key)] == [
        "separates", "leans AI", "too few hits"]

    results = {
        "datasets": {"pinned": {"human_provenance": "pinned-2019"}, "recent": {"human_provenance": "unverified"}},
        "na_rules": {"codet_m4": ["SLOP042", "SLOP043"]},
    }
    assert cell_class(results, {"dataset": "pinned", "label": "human"}) == "human"
    assert cell_class(results, {"dataset": "recent", "label": "human"}) == "human-unverified"
    assert cell_class(results, {"dataset": "recent", "label": "ai"}) == "ai"
    assert cell_class(results, {"dataset": "recent", "label": "ai-paraphrased"}) == "ai-variant"
    assert not cell_has_comments(results, {"dataset": "codet_m4"}), \
        "codet_m4 strips every comment; a comment-scope candidate must skip its cells"
    assert cell_has_comments(results, {"dataset": "pinned"})

    # The shipped regexes must still match the shape they were written for. The backslash-n
    # one is two literal backslashes; a TOML edit turning it into a newline measures nothing.
    here = os.path.dirname(os.path.abspath(__file__))
    data = load_candidates(os.path.join(here, "candidates.toml"))
    by_name = {c["name"]: c for c in data["candidate"]}
    for cand in data["candidate"]:
        compile_candidate(cand)
    backslash_n = next(c for c in data["candidate"] if "backslash-n" in c["name"])
    assert backslash_n["regex"] == "\\\\n\\\\n", backslash_n["regex"]
    assert compile_candidate(backslash_n).search("ends here.\\n\\nAnd then")
    assert not compile_candidate(backslash_n).search("ends here.\n\nAnd then")
    narrated = compile_candidate(by_name[next(n for n in by_name if n.startswith("Narrated"))])
    assert narrated.search("Implementation of the parser would go here")
    assert not narrated.search("thereby avoiding the copy"), "bare `here` must keep its word boundary"
    fence = compile_candidate(by_name[next(n for n in by_name if n.startswith("Bare fence"))])
    assert fence.search("python  ") and fence.search(" rust,") and not fence.search("go func main() {")

    msg = "robotic rhythm: three or more sentences open with `X`"
    rows = message_rows({
        "cells": [
            {"dataset": "pinned", "label": "human", "lang": "prose", "files": 100, "msgs": {"SLOP030": {msg: 4}}},
            {"dataset": "recent", "label": "ai", "lang": "prose", "files": 100, "msgs": {"SLOP030": {msg: 40}}},
            {"dataset": "recent", "label": "human", "lang": "prose", "files": 100, "msgs": {"SLOP030": {msg: 90}}},
            {"dataset": "pinned", "label": "human", "lang": "python", "files": 500, "msgs": {}},
        ],
        "datasets": {"pinned": {"human_provenance": "pinned-2019"}, "recent": {"human_provenance": "unverified"}},
        "applicable": {"prose": ["SLOP030"], "python": ["SLOP006"]},
        "na_rules": {},
    }, argparse.Namespace(langs=None, datasets=None))
    assert rows[0]["messages"][0]["human"] == 4 and rows[0]["messages"][0]["ai"] == 40, \
        "an unverified human split must not enter the pooled human count"
    assert rows[0]["human_files"] == 100, "a prose rule's denominator must exclude the code cells"

    # MESSAGE_FLOOR is on the pct() 0-100 scale: a message at 0.3% must fold, one at 5%/6%
    # must not. At the old 0.02 value nothing ever folded.
    floor_rows = message_rows({
        "cells": [
            {"dataset": "pinned", "label": "human", "lang": "prose", "files": 1000,
             "msgs": {"SLOP030": {"common": 50, "rare": 3}}},
            {"dataset": "recent", "label": "ai", "lang": "prose", "files": 1000,
             "msgs": {"SLOP030": {"common": 60, "rare": 2}}},
        ],
        "datasets": {"pinned": {"human_provenance": "pinned-2019"}, "recent": {"human_provenance": "unverified"}},
        "applicable": {"prose": ["SLOP030"]},
        "na_rules": {},
    }, argparse.Namespace(langs=None, datasets=None))
    assert [m["message"] for m in floor_rows[0]["messages"]] == ["common"], floor_rows[0]["messages"]
    assert floor_rows[0]["folded"] == 1

    print("ok")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--dir", default="target/corpus", help="corpus root written by score_corpus.py")
    ap.add_argument("--results", default="target/corpus/results.json", help="results.json from score_corpus.py")
    ap.add_argument("--candidates", default="bench/candidates.toml", help="candidate table to measure")
    ap.add_argument("--json", help="write analysis.json to this path")
    ap.add_argument("--report", help="write the markdown report here instead of stdout")
    ap.add_argument("--only", help="comma-separated passes: candidates, messages, phrases")
    ap.add_argument("--langs", help="comma-separated langs to restrict to")
    ap.add_argument("--datasets", help="comma-separated dataset names to restrict to")
    ap.add_argument("--self-check", action="store_true", help="assert the scanners and exit")
    args = ap.parse_args()
    if args.self_check:
        return self_check()
    args.langs = set(args.langs.split(",")) if args.langs else None
    args.datasets = set(args.datasets.split(",")) if args.datasets else None
    passes = set((args.only or "candidates,messages,phrases").split(","))
    unknown = passes - {"candidates", "messages", "phrases"}
    if unknown:
        raise SystemExit(f"--only: unknown pass(es) {sorted(unknown)}")

    if not os.path.exists(args.results):
        raise SystemExit(f"{args.results}: no results; run just corpus-score first")
    with open(args.results, encoding="utf-8") as fh:
        results = json.load(fh)
    data = load_candidates(args.candidates)
    cells = selected_cells(results, args)
    verified = sorted({c["dataset"] for c in cells if cell_class(results, c) == "human"})
    unverified = sorted({c["dataset"] for c in cells if cell_class(results, c) == "human-unverified"})

    payload = {"schema": results["schema"], "limit": results["limit"], "cells": len(cells),
               "verified": verified, "unverified": unverified,
               "provenance_note": results["provenance_note"],
               "candidates_data": {k: v for k, v in data.items() if k != "candidate"}}
    if "candidates" in passes:
        payload["candidates"] = candidate_rows(results, args.dir, data, args)
        sys.stderr.write(f"candidates: {len(payload['candidates'])} measured\n")
    if "messages" in passes:
        payload["messages"] = message_rows(results, args)
        sys.stderr.write(f"messages: {len(payload['messages'])} rules\n")
    if "phrases" in passes:
        payload["phrases"] = phrase_examples(results, args.dir, phrase_groups(results, args.dir, args))
        sys.stderr.write(f"phrases: {sum(len(g['phrases']) for g in payload['phrases'])} kept\n")

    out = build_analysis_report(payload)
    if args.report:
        with open(args.report, "w", encoding="utf-8") as fh:
            fh.write(out)
    else:
        sys.stdout.write(out)
    if args.json:
        with open(args.json, "w", encoding="utf-8") as fh:
            json.dump(payload, fh, ensure_ascii=False, sort_keys=True)
            fh.write("\n")


if __name__ == "__main__":
    main()
