"""Unit tests for `bench/score_corpus.py` and `bench/corpus/*`.

Runnable via `python3 bench/score_corpus.py --self-check` (which discovers this package) or
directly via `python3 -m unittest discover -s bench/tests -t bench -v`. Both invocations put
`bench/` on `sys.path`, which is what makes `import score_corpus` and `from corpus... import
...` resolve; the explicit insert below is a fallback for running this file on its own.

One `TestCase` per concern, ported from the single `self_check()` function this crate used to
carry inline: every assertion there has a home here, none dropped.
"""
import argparse
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from corpus.common import APPLICABLE, EXT, PROVENANCE, RESULTS_SCHEMA, VERIFIED, sample_indices
from corpus.fetchers import FAIDSET_LABELS, VIETNAMESE, fetch_local_cell, parse_beemo_edits, parse_mage_src
from corpus.git import clone_dir_for, git_files, shared_clone_dir
from corpus.hf import (
    _looks_retryable, _paged_rows, _windows_overlap, hf_get_cached, scan_order, spread_page_offsets,
    where_predicate,
)
from corpus.lint import clip, message_tally, normalize_message, read_cell, tally, write_cell
from corpus.metrics import (
    MIN_AI_FILES, SEPARATES_LIFT, SEPARATES_RATE, lift, per_1k_words, per_kloc, pct, precision,
    precision_at_prior, rule_verdict,
)
from corpus.orchestrate import carry_over, cell_root, manifest_path
from corpus.registry import CELL_KINDS, DATASET_KINDS, DATASETS
from corpus.report import build_report, has_human, is_verified, split_by_provenance
from corpus.results import parse_rules, results_payload, scoreboard

import score_corpus


class TestMetrics(unittest.TestCase):
    def test_precision(self):
        self.assertIsNone(precision(0.0, 0.0))
        self.assertEqual(precision(4.0, 4.0), 0.5)
        self.assertEqual(precision(0.0, 4.0), 1.0)

    def test_precision_at_prior(self):
        self.assertIsNone(precision_at_prior(0, 0))
        self.assertEqual(precision_at_prior(3, 1), 0.75)

    def test_lift(self):
        self.assertEqual(lift(0.0, 0.0), "n/a")
        self.assertEqual(lift(0.0, 5.0), "inf")
        self.assertEqual(lift(10.0, 20.0), 2.0)

    def test_pct(self):
        self.assertEqual(pct(1, 4), 25.0)
        self.assertEqual(pct(0, 0), 0.0)

    def test_density_helpers(self):
        self.assertEqual(per_kloc(2, 1000), 2.0)
        self.assertEqual(per_1k_words(2, 500), 4.0)

    def test_clip(self):
        clipped, _offset = clip("x" * 300, 1)
        self.assertTrue(clipped.endswith("…"))
        self.assertEqual(len(clipped), 231)  # LINE_MAX + 1
        self.assertEqual(clip("short line", 1), ("short line", 0))


class TestSampling(unittest.TestCase):
    def test_sample_indices(self):
        self.assertEqual(sample_indices(10, 0), list(range(10)))
        self.assertEqual(sample_indices(10, 20), list(range(10)))
        self.assertEqual(sample_indices(10, 5), [0, 2, 4, 6, 8])
        self.assertEqual(len(sample_indices(1000, 7)), 7)
        self.assertEqual(sample_indices(1000, 7), sorted(set(sample_indices(1000, 7))))

    def test_spread_page_offsets(self):
        sparse = spread_page_offsets(100000, 5)
        self.assertEqual(len(sparse), 5)
        self.assertFalse(_windows_overlap(sparse))
        self.assertTrue(any(offset for offset, _ in sparse), "a spread sample must not read the literal head")
        single = spread_page_offsets(100000, 1)
        self.assertNotEqual(single[0][0], 0, "a single page must still center, not read offset 0")
        dense = spread_page_offsets(250, 3)
        self.assertFalse(_windows_overlap(dense))
        self.assertEqual(sum(length for _, length in dense), 250, "full coverage must not drop rows")

    def test_ext_and_applicable_share_the_same_langs(self):
        self.assertEqual(set(EXT), set(APPLICABLE))


class TestHfPaging(unittest.TestCase):
    def test_where_predicate_round_trips(self):
        keep = where_predicate("\"Language\"='Go' and \"Label\"<>'HUMAN_GENERATED'")
        self.assertTrue(keep({"Language": "Go", "Label": "MACHINE_GENERATED"}))
        self.assertFalse(keep({"Language": "Go", "Label": "HUMAN_GENERATED"}))
        self.assertFalse(keep({"Language": "Rust", "Label": "MACHINE_GENERATED"}))
        label_is_one = where_predicate('"label"=1')
        self.assertTrue(label_is_one({"label": 1}))
        self.assertTrue(label_is_one({"label": "1"}))
        self.assertFalse(label_is_one({"label": 0}))

    def test_every_registered_where_clause_is_scannable(self):
        for spec in DATASETS.values():
            for cell in spec.get("cells", []):
                if "where" in cell:
                    where_predicate(cell["where"])  # must not raise

    def test_unsupported_where_clause_is_rejected(self):
        with self.assertRaises(SystemExit):
            where_predicate("\"a\" LIKE 'b%'")

    def test_scan_order_is_a_permutation_that_avoids_the_head(self):
        self.assertEqual(sorted(scan_order(7)), list(range(7)))
        self.assertNotEqual(scan_order(7)[:2], [0, 1], "the scan must not walk the head first")

    def test_paged_rows_dedupes_overlapping_boundaries(self):
        calls = []

        def fake_call_page(offset, length):
            base = 0 if not calls else 50
            calls.append((offset, length))
            return {"rows": [{"row_idx": base + i, "row": base + i} for i in range(100)]}

        self.assertEqual(_paged_rows(fake_call_page, 300, 150, False), list(range(150)))

    def test_looks_retryable(self):
        self.assertIs(_looks_retryable(b'{"error": "index is loading"}'), True)
        self.assertIs(_looks_retryable(b'{"rows": []}'), False)
        self.assertIs(_looks_retryable(b"not json"), False)
        self.assertIs(_looks_retryable(b"[1, 2, 3]"), False)
        self.assertIs(_looks_retryable(b"\x80\x81\x82\x83"), False)

    def test_hf_get_cached_short_circuits_on_a_cache_hit(self):
        with tempfile.TemporaryDirectory() as tmp:
            # `.invalid` is a reserved TLD (RFC 2606), so a bug here hangs, not lies.
            cache_path = os.path.join(tmp, "cached.json")
            with open(cache_path, "w", encoding="utf-8") as fh:
                json.dump({"ok": True}, fh)
            cached = hf_get_cached("https://definitely-not-a-real-host.invalid/x", cache_path, "test", False)
            self.assertEqual(cached, {"ok": True})


class TestGit(unittest.TestCase):
    def test_git_files_globs_and_excludes_without_a_real_clone(self):
        with tempfile.TemporaryDirectory() as tmp:
            os.makedirs(os.path.join(tmp, "src", "sub"))
            os.makedirs(os.path.join(tmp, "src", "sub", "_test"))
            for rel in ("src/a.py", "src/sub/b.py", "src/sub/_test/c.py", "src/notes.md"):
                path = os.path.join(tmp, rel)
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(path, "w", encoding="utf-8") as fh:
                    fh.write("x")
            found = git_files(tmp, "src", "*.py", exclude=("_test/",))
            self.assertEqual(found, ["src/a.py", "src/sub/b.py"])

    def test_clone_dir_is_stable_per_repo_and_ref(self):
        cache_dir = "/tmp/does-not-need-to-exist/cache/go-std"
        a = clone_dir_for({"repo": "https://example.invalid/x", "tag": "v1"}, cache_dir)
        b = clone_dir_for({"repo": "https://example.invalid/x", "tag": "v1"}, cache_dir)
        c = clone_dir_for({"repo": "https://example.invalid/x", "tag": "v2"}, cache_dir)
        self.assertEqual(a, b)
        self.assertNotEqual(a, c)
        self.assertEqual(shared_clone_dir(cache_dir, "r", "t"), shared_clone_dir(cache_dir, "r", "t"))


class TestFetchers(unittest.TestCase):
    def test_parse_mage_src(self):
        self.assertEqual(parse_mage_src("cmv_human"), ("cmv", "human"))
        self.assertEqual(parse_mage_src("eli5_machine_topical_text-davinci-003"), ("eli5", "text-davinci-003"))

    def test_vietnamese_letter_detection(self):
        self.assertTrue(VIETNAMESE.search("Ứng dụng giao đồ ăn"))
        self.assertFalse(VIETNAMESE.search("Among its noteworthy features"))
        self.assertEqual(set(FAIDSET_LABELS), {"human", "ai", "ai-collab"})

    def test_parse_beemo_edits(self):
        self.assertEqual(parse_beemo_edits("[{'P1': 'first'}, {'P2': 'second'}]"), ["first", "second"])
        self.assertEqual(parse_beemo_edits("not a literal"), [])
        self.assertEqual(parse_beemo_edits(None), [])

    def test_write_cell_drops_blank_and_whitespace_only_items(self):
        with tempfile.TemporaryDirectory() as tmp:
            counts = write_cell(os.path.join(tmp, "cell"), [
                ("hello world", {"generator": "human"}),
                ("", {"generator": "human"}),
                ("   \n\t  ", {"generator": "human"}),
            ], "py")
            self.assertEqual(counts["files"], 1)
            self.assertEqual(counts["empty"], 2)
            self.assertEqual(next(iter(counts["index"].values()))["words"], 2)

    def test_fetch_local_cell_prefers_the_synth_index_over_the_label(self):
        with tempfile.TemporaryDirectory() as tmp:
            cell_dir = os.path.join(tmp, "synth", "mycell")
            ai_dir = os.path.join(cell_dir, "ai", "python")
            os.makedirs(ai_dir)
            for fname, body in (("00000.py", "print(1)\n"), ("00001.py", "print(2)\n")):
                with open(os.path.join(ai_dir, fname), "w", encoding="utf-8") as fh:
                    fh.write(body)
            indexed_rel = os.path.relpath(os.path.join(ai_dir, "00000.py"), cell_dir).replace(os.sep, "/")
            with open(os.path.join(cell_dir, "index.json"), "w", encoding="utf-8") as fh:
                json.dump({indexed_rel: {"generator": "gpt-4"}}, fh)
            local_gens = sorted(meta["generator"] for _text, meta in fetch_local_cell(ai_dir, 0, "fallback"))
            self.assertEqual(local_gens, ["fallback", "gpt-4"])


class TestLintTallyMessages(unittest.TestCase):
    def test_tally_keys_on_the_bare_filename(self):
        hits, totals = tally([
            {"code": "SLOP001", "path": "/a/00001.py"},
            {"code": "SLOP001", "path": "/b/00001.py"},
            {"code": "SLOP002", "path": "/a/00002.py"},
        ])
        self.assertEqual(hits["SLOP001"], {"00001.py"})
        self.assertEqual(totals, {"SLOP001": 2, "SLOP002": 1})

    def test_normalize_message(self):
        self.assertEqual(
            normalize_message("SLOP039", "`newClient` only forwards to `client`"),
            "`X` only forwards to `X`",
        )
        self.assertEqual(
            normalize_message("SLOP037", "`ioutil.ReadFile` has a direct `os`/`io` replacement"),
            "`ioutil.ReadFile` has a direct `os`/`io` replacement",
        )
        self.assertEqual(
            normalize_message("SLOP037", "`sha256sum` ran on 3 files"), "`sha256sum` ran on N files",
            "a digit inside a KEEP_BACKTICKS span must survive; only the digit outside it collapses",
        )
        self.assertEqual(
            normalize_message("SLOP033", "sentence runs 51 words; split it"), "sentence runs N words; split it",
        )
        self.assertEqual(
            normalize_message("SLOP027", 'filler phrase repeated: "in order to" appears multiple times'),
            'filler phrase repeated: "in order to" appears multiple times',
        )

    def test_message_tally_groups_by_normalized_shape(self):
        msgs = message_tally([
            {"code": "SLOP033", "path": "/a/1.md", "message": "sentence runs 51 words; split it"},
            {"code": "SLOP033", "path": "/a/2.md", "message": "sentence runs 62 words; split it"},
            {"code": "SLOP033", "path": "/a/2.md", "message": "sentence runs 71 words; split it"},
        ])
        self.assertEqual(msgs, {"SLOP033": {"sentence runs N words; split it": {"1.md", "2.md"}}})

    def test_read_cell_matches_the_walk_exactly(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = os.path.join(tmp, "cell")
            write_cell(root, [("a", {"generator": "human"}), ("b", {"generator": "human"})], "py")
            with open(os.path.join(root, ".DS_Store"), "w", encoding="utf-8") as fh:
                fh.write("junk")
            self.assertEqual(read_cell(root, 3)["files"], 2)
            self.assertEqual(read_cell(root, 3)["empty"], 3)
            os.remove(os.path.join(root, "00001.py"))
            with self.assertRaises(SystemExit) as ctx:
                read_cell(root, 0)
            self.assertIn("disagree", str(ctx.exception))


class TestManifest(unittest.TestCase):
    def test_carry_over(self):
        with tempfile.TemporaryDirectory() as tmp:
            args = argparse.Namespace(dir=tmp, limit=500)
            root = cell_root(args, "x", "human", "python")
            os.makedirs(root)
            with open(os.path.join(root, "00000.py"), "w", encoding="utf-8") as fh:
                fh.write("pass\n")
            with open(f"{root}.index.json", "w", encoding="utf-8") as fh:
                json.dump({"00000.py": {"generator": "human", "words": 1}}, fh)
            with open(manifest_path(args), "w", encoding="utf-8") as fh:
                json.dump({
                    "schema": RESULTS_SCHEMA, "limit": 500, "not_fetched": [],
                    "datasets": {"x": {"revision": "r1", "cells": [
                        {"label": "human", "lang": "python", "ext": "py", "files": 1, "empty": 0},
                    ]}},
                }, fh)

            datasets = {}
            carry_over(args, {"y"}, datasets, [])
            self.assertIn("x", datasets, "a dataset outside this run's selection must be carried forward")

            datasets = {}
            not_fetched = carry_over(args, {"x"}, datasets, ["x/human/python"])
            self.assertIn("x", datasets, "a selected dataset whose refresh produced nothing must be restored")
            self.assertEqual(not_fetched, [], "a restored cell must not also read as not-fetched")

            with self.assertRaises(SystemExit):
                carry_over(argparse.Namespace(dir=tmp, limit=300), {"x"}, {}, ["x/human/python"])


class TestRegistry(unittest.TestCase):
    def test_every_dataset_declares_a_consistent_human_split(self):
        for name, ds in DATASETS.items():
            prov = ds.get("human_provenance")
            with self.subTest(dataset=name):
                if has_human(ds):
                    self.assertIn(prov, PROVENANCE)
                    self.assertTrue(ds.get("human_note"))
                else:
                    self.assertIsNone(prov)
                    self.assertFalse(ds.get("human_note"))
                self.assertIn(ds["kind"], CELL_KINDS + DATASET_KINDS + ("local",))

    def test_is_verified_needs_both_a_human_split_and_a_cleared_bar(self):
        self.assertTrue(is_verified({"cells": [{"label": "human"}], "human_provenance": "pinned-2019"}))
        self.assertFalse(is_verified({"cells": [{"label": "human"}], "human_provenance": "unverified"}))
        self.assertFalse(is_verified({"cells": [{"label": "ai"}], "human_provenance": "pinned-2019"}))

    def test_split_by_provenance(self):
        verified, unverified = split_by_provenance(["cpython-lib", "droid"], DATASETS)
        self.assertEqual(verified, ["cpython-lib"])
        self.assertEqual(unverified, ["droid"])


class TestCli(unittest.TestCase):
    def test_parse_local_accepts_the_documented_spec(self):
        self.assertEqual(score_corpus.parse_local("name=dir:human:python"), ("name", "dir", "human", "python"))

    def test_parse_local_rejects_malformed_specs(self):
        for bad_spec in ("no-equals-sign", "name=dir:onlylabel", "name=dir:human:notalang"):
            with self.subTest(spec=bad_spec):
                with self.assertRaises(SystemExit):
                    score_corpus.parse_local(bad_spec)


class TestRuleVerdict(unittest.TestCase):
    """Each branch of `rule_verdict`, which backs both the results.json scoreboard and (once
    switched over) `bench/analyze_corpus.py`'s `candidate_verdict`."""

    def test_nodata_below_min_ai_files(self):
        self.assertEqual(rule_verdict(0, 100, MIN_AI_FILES - 1, 100), "nodata")

    def test_separates_on_infinite_lift(self):
        self.assertEqual(rule_verdict(0, 100, MIN_AI_FILES, 100), "sep")

    def test_separates_on_a_high_finite_lift(self):
        self.assertEqual(rule_verdict(10, 100, 40, 100), "sep")  # lift 4.0 >= SEPARATES_LIFT

    def test_weak_below_the_separates_threshold(self):
        self.assertEqual(rule_verdict(10, 100, 20, 100), "weak")  # lift 2.0

    def test_leans_human_below_point_67(self):
        self.assertEqual(rule_verdict(30, 100, MIN_AI_FILES, 100), "inv")  # lift 0.5

    def test_at_chance_around_lift_one(self):
        self.assertEqual(rule_verdict(20, 100, 20, 100), "chance")  # lift 1.0

    def test_constants_match_the_html_template(self):
        # bench/report_template.html hardcodes MIN_AI_FILES = 15, SEPARATES_LIFT = 3,
        # SEPARATES_RATE = 1; a drift here would silently desync the scoreboard from the page.
        self.assertEqual((MIN_AI_FILES, SEPARATES_LIFT, SEPARATES_RATE), (15, 3, 1))


class TestScoreboard(unittest.TestCase):
    def test_pools_human_and_ai_and_ranks_the_firing_datasets(self):
        applicable = {"python": ("SLOP003",)}
        dataset_meta_by_name = {"human-ds": {"verified": True}, "ai-ds": {"verified": False}}
        cells = [
            {"dataset": "human-ds", "label": "human", "lang": "python", "files": 100, "rules": {}},
            {"dataset": "ai-ds", "label": "ai", "lang": "python", "files": 100,
             "rules": {"SLOP003": {"files": 20, "findings": 25}}},
        ]
        rows = scoreboard(cells, applicable, {"SLOP003": {}}, {}, dataset_meta_by_name)
        self.assertEqual(len(rows), 1)
        row = rows[0]
        self.assertEqual(row["code"], "SLOP003")
        self.assertEqual(row["h"], {"f": 0, "t": 100, "r": 0.0})
        self.assertEqual(row["a"], {"f": 20, "t": 100, "r": 20.0})
        self.assertEqual(row["b"], {"f": 0, "t": 100, "r": 0.0}, "the lone human cell has no ai counterpart, so it is unpaired")
        self.assertIsNone(row["lift"], "a zero human rate is Infinity in JS, collapsed to None here")
        self.assertEqual(row["verdict"], "sep")
        self.assertEqual(row["ds_hits"], ["ai-ds"])
        self.assertEqual(row["any"], 20)

    def test_na_rules_exclude_a_cell_from_pooling(self):
        applicable = {"python": ("SLOP003",)}
        dataset_meta_by_name = {"ai-ds": {"verified": False}}
        cells = [{"dataset": "ai-ds", "label": "ai", "lang": "python", "files": 100,
                  "rules": {"SLOP003": {"files": 20, "findings": 25}}}]
        rows = scoreboard(cells, applicable, {"SLOP003": {}}, {"ai-ds": ["SLOP003"]}, dataset_meta_by_name)
        self.assertEqual(rows[0]["a"], {"f": 0, "t": 0, "r": 0.0}, "na_rules excludes the cell from pooling")
        # dsHits and "any" are raw per-cell hit counts, independent of na_rules -- matching
        # bench/report_template.html's `hits(c, code)`, which never consults na_rules either.
        self.assertEqual(rows[0]["ds_hits"], ["ai-ds"])
        self.assertEqual(rows[0]["any"], 20)


class TestReportResults(unittest.TestCase):
    def test_parse_rules(self):
        self.assertEqual(
            parse_rules("| SLOP001 | artifact | Elision | A, on | Python, Go | en | drops code |\n"),
            {"SLOP001": {"group": "artifact", "name": "Elision", "tier": "A", "default": "on",
                         "langs": "Python, Go", "natlangs": "en", "desc": "drops code"}},
        )
        self.assertEqual(
            parse_rules("| SLOP045 | format | Uniform | C, off | Rust | en | flat |\n")["SLOP045"]["tier"], "C",
        )

    def test_results_payload_survives_json_dumps(self):
        # Every set has to be converted at the payload boundary, and a stray set here is a
        # crash at the end of a 20-minute run otherwise.
        fake_cell = {
            "files": 2, "empty": 0, "index": {"00000.py": {"generator": "gpt-4", "words": 3}},
            "hits": {"SLOP003": {"00000.py"}}, "totals": {"SLOP003": 1},
            "stats": {"files": 2, "skipped": 0, "lines": 9}, "words_total": 3,
            "tier_a_hits": {"00000.py"}, "any_hits": {"00000.py"},
            "msgs": {"SLOP003": {"stray markdown code fence in source file": {"00000.py"}}},
            "examples": {"SLOP003": [{"file": "00000.py", "line": 1, "col": 1, "message": "m",
                                      "fix": None, "meta": {}, "more": 0, "snippet": [[1, "```py", True]]}]},
        }
        fake_args = argparse.Namespace(bin="/usr/bin/true", limit=500, dir=".")
        payload = results_payload({"aigcodeset": {"python": {"ai": fake_cell}}}, {"aigcodeset": "abc"},
                                  DATASETS, fake_args, {"SLOP003": {"tier": "A"}}, ["x/y/z"])
        dumped = json.loads(json.dumps(payload))
        self.assertEqual(dumped["cells"][0]["rules"], {"SLOP003": {"files": 1, "findings": 1}})
        self.assertEqual(payload["cells"][0]["msgs"]["SLOP003"], {"stray markdown code fence in source file": 1})
        self.assertIn("scoreboard", dumped, "the scoreboard must round-trip through JSON too")

    def test_build_report_smoke(self):
        fake_registry = {
            "fake-ds": {
                "url": "https://example.invalid/fake-ds", "paper": None, "license": "MIT",
                "natlang": None, "generator_years": None,
                "human_provenance": "pinned-2019", "human_note": "fabricated for the smoke test",
                "caveats": [], "cells": [{"label": "human", "lang": "python"}, {"label": "ai", "lang": "python"}],
            },
        }
        fake_cell = {
            "files": 2, "empty": 0, "index": {"00000.py": {"generator": "human", "words": 3}},
            "hits": {}, "totals": {}, "stats": {"files": 2, "skipped": 0, "lines": 9},
            "words_total": 3, "tier_a_hits": set(), "any_hits": set(), "msgs": {}, "examples": {},
        }
        datasets_built = {"fake-ds": {"python": {"human": fake_cell, "ai": fake_cell}}}
        args = argparse.Namespace(bin="/usr/bin/true", limit=500, langs=None)
        report = build_report(datasets_built, {"fake-ds": "abc123"}, fake_registry, args, [])
        self.assertIn("## Per-rule hit rate across the corpus registry", report)
        self.assertIn("### fake-ds", report)
        self.assertIn("## Per-dataset detail", report)


if __name__ == "__main__":
    unittest.main()
