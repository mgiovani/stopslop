"""Render the standalone corpus scorecard from results.json and analysis.json.

usage: python3 bench/render_report.py --out target/corpus/report.html
       python3 bench/render_report.py --self-check

`bench/report_template.html` is the page: one file, one inline script, one CDN dependency
(highlight.js). This script injects the two JSON payloads into its `/*__DATA__*/` and
`/*__CANDS__*/` placeholders and writes the result. The output is never committed: several
registered datasets forbid redistributing a derivative, and the page quotes their lines.

Every non-ASCII character is escaped, as an entity in the markup and as a `\\u` escape inside
the script, so the page reads correctly however it is served: a local `http.server` sends no
charset, and a browser then decodes UTF-8 bytes as Latin-1.

ponytail: the markup region includes `<style>`, a raw-text element where character references
don't decode, so a non-ASCII character added to that CSS would render as the literal text
`&#233;`, not the glyph. None exists there today. Upgrade path: carve `<style>...</style>` out
of the entity-reference pass and give it its own escape (or ban non-ASCII in the template's CSS
outright) if one is ever added.
"""
import argparse
import json
import os

# The inline script starts after this marker; everything before it is markup, everything
# after it is JavaScript, and the two need different escapes.
SCRIPT_MARKER = '<script src="https://cdnjs'
DATA_SLOT = "/*__DATA__*/"
CANDS_SLOT = "/*__CANDS__*/"


def embed(obj):
    """JSON safe to sit inside a `<script>` element: ASCII only, and no `</` that would end
    the element early."""
    return json.dumps(obj, ensure_ascii=True, separators=(",", ":")).replace("</", "<\\/")


def ascii_safe(template):
    if template.count(SCRIPT_MARKER) != 1:
        raise SystemExit(f"template must hold exactly one {SCRIPT_MARKER!r}, "
                          f"found {template.count(SCRIPT_MARKER)}; the markup/script split is guesswork otherwise")
    marker_at = template.find(SCRIPT_MARKER)
    head, tail = template[:marker_at], template[marker_at:]
    return (head.encode("ascii", "xmlcharrefreplace").decode("ascii")
            + "".join(c if ord(c) < 128 else "\\u%04x" % ord(c) for c in tail))


def inject(template, results, analysis):
    for slot in (DATA_SLOT, CANDS_SLOT):
        if template.count(slot) != 1:
            raise SystemExit(f"template must hold exactly one {slot}, found {template.count(slot)}")
    return (ascii_safe(template)
            .replace(DATA_SLOT, embed(results))
            .replace(CANDS_SLOT, embed(analysis) if analysis else "null"))


def self_check():
    page = inject(f"<p>café</p>{SCRIPT_MARKER}x</script><script>{DATA_SLOT}{CANDS_SLOT}</script>",
                  {"t": "naïve </script>"}, None)
    assert page.isascii(), "the page must be pure ASCII however it is served"
    assert "caf&#233;" in page and "\\u00ef" in page
    assert embed({"t": "</script>"}) == '{"t":"<\\/script>"}', "a payload must not close the script element"
    assert page.count("</script>") == 2, "only the two real element ends may appear"
    assert page.endswith("null</script>")
    for bad in ("<p>no slots</p>", f"{SCRIPT_MARKER}{DATA_SLOT}{DATA_SLOT}{CANDS_SLOT}"):
        try:
            inject(bad, {}, None)
            assert False, "a template missing or duplicating a slot must be rejected"
        except SystemExit as refused:
            assert "exactly one" in str(refused)
    for bad in (f"<p>no script</p>{DATA_SLOT}",
                f"{SCRIPT_MARKER}x</script>{SCRIPT_MARKER}y</script>{DATA_SLOT}{CANDS_SLOT}"):
        try:
            ascii_safe(bad)
            assert False, "a template missing or duplicating the script marker must be rejected"
        except SystemExit as refused:
            assert "exactly one" in str(refused)
    print("ok")


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--results", default="target/corpus/results.json")
    ap.add_argument("--analysis", default="target/corpus/analysis.json",
                     help="optional; the candidate section is hidden without it")
    ap.add_argument("--template", default=os.path.join(here, "report_template.html"))
    ap.add_argument("--out", default="target/corpus/report.html")
    ap.add_argument("--self-check", action="store_true", help="assert the escaping and exit")
    args = ap.parse_args()
    if args.self_check:
        return self_check()

    if not os.path.exists(args.results):
        raise SystemExit(f"{args.results}: no results; run just corpus-score first")
    with open(args.results, encoding="utf-8") as fh:
        results = json.load(fh)
    analysis = None
    if args.analysis and os.path.exists(args.analysis):
        with open(args.analysis, encoding="utf-8") as fh:
            analysis = json.load(fh)
    with open(args.template, encoding="utf-8") as fh:
        template = fh.read()
    page = inject(template, results, analysis)
    os.makedirs(os.path.dirname(args.out) or ".", exist_ok=True)
    with open(args.out, "w", encoding="ascii") as fh:
        fh.write(page)
    print(f"{args.out}: {len(page)} bytes, {len(results['cells'])} cells, "
          f"{len(analysis.get('candidates', [])) if analysis else 0} candidates")


if __name__ == "__main__":
    main()
