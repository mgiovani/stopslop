"""Deterministic stress inputs for bench/run.sh and tests/stress.rs.

usage: python3 bench/gen_inputs.py <outdir>
"""
import os
import pathlib
import random
import sys

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
random.seed(21)

SLOP = [
    "In today’s fast-paced world, it’s important to note that we must delve into the rich tapestry of modern development — leveraging robust, seamless, cutting-edge solutions.",
    "Furthermore, this comprehensive guide will explore how to navigate the ever-evolving landscape of best practices — ensuring that you can unlock the full potential of your workflow.",
    "It’s worth noting that many teams — arguably most of them — struggle to harness these powerful tools; experts agree that a holistic approach is a game-changer.",
    "At the end of the day, we should embark on this journey together, fostering a vibrant ecosystem of innovation — not just for the sake of it, but because it truly matters.",
    "Let’s dive in: first, we’ll underscore the key takeaways; then, we’ll meticulously break down each step; finally, we’ll wrap up with actionable insights.",
]
CLEAN = [
    "The scheduler reads the manifest once at startup and caches the parsed result for the rest of the run.",
    "Each worker owns a bounded queue, so a slow consumer applies backpressure instead of growing memory without limit.",
    "Retries use exponential backoff with jitter, capped at five attempts, and every attempt is logged with the request id.",
    "The migration adds an index on (tenant_id, created_at) because the dashboard query filters on both columns.",
    "Configuration is loaded from the environment first and falls back to the checked-in defaults file.",
    "Tests run against a temporary directory that is deleted at the end of each case.",
    "The parser rejects a header longer than 8 KB and returns 431 so the client can shorten its cookies.",
]
LINK = "See the [release notes](https://links.test/releases/{n}) and <https://links.test/docs/{n}> or https://links.test/raw/{n}.txt for details."
MB = 1 << 20


def paragraph(i):
    sents = [random.choice(SLOP) if random.random() < 0.35 else random.choice(CLEAN) for _ in range(random.randint(4, 6))]
    if i % 7 == 0:
        sents.append(LINK.format(n=i))
    return " ".join(sents)


def build(target, block):
    parts, size, i = [], 0, 0
    while size < target:
        p = block(i)
        parts.append(p)
        size += len(p.encode())
        i += 1
    return "".join(parts)


def prose_block(i):
    if i % 8 == 0:
        return f"## Section {i // 8}: notes on the {random.choice(['scheduler', 'parser', 'cache', 'walker'])}\n\n"
    if i % 11 == 0:
        return "".join(f"- {s}\n" for s in random.sample(CLEAN, 3)) + "\n"
    return paragraph(i) + "\n\n"


def heading_block(i):
    para = random.choice(CLEAN) + " " + random.choice(CLEAN)
    if i % 5 == 0:
        para += " " + random.choice(SLOP)
    return f"## Section {i}: {random.choice(['setup', 'rollout', 'rollback', 'ownership'])}\n\n{para}\n\n"


def write(name, text):
    with open(os.path.join(out, name), "w") as f:
        f.write(text)


def slice_to(text, nbytes):
    b = text.encode()[:nbytes]
    return b[: b.rfind(b"\n") + 1].decode()


def code_file():
    """One TypeScript file shaped to catch the quadratics AST rules have actually shipped.

    Every section targets a specific past bug family, so a regression shows up as wall time
    rather than as a rule that quietly got slower:
      - 5,000 methods in ONE class body: a `next_named_sibling()` step is O(index) per call.
      - 20k comments and 20k string literals next to 2,000 regex hits: rules that ask
        `in_comment_or_string` per match pay matches x spans.
      - 200-level nesting: parent-chain walks.
      - one ~200 KB line: anything that rescans to the line start, or from byte 0, per finding.
    """
    parts = ["import { readFile, writeFile } from 'node:fs/promises';\n"]
    parts += [f"import {{ helper{i} }} from './mod{i}';\n" for i in range(200)]
    parts.append("import express from 'express';\nimport lodash from 'lodash';\n\n")

    parts.append("export class Registry {\n")
    for i in range(5000):
        parts.append(f"  // resolves entry {i} from the backing store\n")
        parts.append(f"  entry{i}(key: string): string {{ return this.store[key] ?? 'default-{i}'; }}\n")
    parts.append("}\n\n")

    for i in range(2000):
        parts.append(f"// clone helper {i}: the store hands out shared references\n")
        parts.append(f"export const clone{i} = (v: unknown) => JSON.parse(JSON.stringify(v));\n")
        parts.append(f"const label{i} = 'entry-{i}-payload-string-with-some-length';\n")
        parts.append(f"const note{i} = \"another {i} string literal, kept for the span count\";\n")

    # Indentation makes each of these ~80 KB, so eight is enough to exercise the parent walks
    # without the nesting section dominating the file's size.
    for i in range(8):
        parts.append(f"export function deep{i}(n: number): number {{\n")
        for d in range(200):
            parts.append("  " * (d + 1) + f"if (n > {d}) {{\n")
        parts.append("  " * 201 + "return n;\n")
        for d in range(199, -1, -1):
            parts.append("  " * (d + 1) + "}\n")
        parts.append("  return 0;\n}\n")

    parts.append("const wide = " + " + ".join(f"'chunk{i}'" for i in range(20000)) + ";\n")
    return "".join(parts)


prose = build(8 * MB, prose_block)
write("prose_8mb_emdash.md", prose)
write("prose_8mb_ascii.md", prose.replace("—", "--").replace("’", "'").replace("“", '"').replace("”", '"').replace("…", "..."))
headings = build(20 * MB, heading_block)
write("headings_20mb.md", headings)
for n in (2, 4, 8):
    write(f"headings_{n}mb.md", slice_to(headings, n * MB))
write("three_lines.md", "# Notes\n\nThe build runs the tests and then packages the binary.\n")
write("oneline_700k.md", " — ".join(random.choice(CLEAN).split()[0] + "ish" for _ in range(100_000)) + "\n")
write("code_2mb.ts", code_file())

for name in sorted(os.listdir(out)):
    data = pathlib.Path(out, name).read_bytes()
    lines, headings, dashes = data.count(b"\n"), data.count(b"## "), data.count("—".encode())
    print(f"{name}: {len(data)} bytes, {lines} lines, {headings} headings, {dashes} em dashes")
