"""Labelled real / GAN / diffusion image sets for the image-metadata and spectral rules.

Run from the repo root: `python -m bench.corpus.images` (Pillow only for the resize step,
so `uv run --with pillow python -m bench.corpus.images`).

Writes original bytes (no re-encode) to `bench/corpus/.cache/images/<class>/`. PNG or JPEG
only, at least 256x256. `real_resized/` holds real images downscaled 0.5x (PNG and JPEG q=75).

Sources (dataset id, license from the dataset card, why it is labelled):
  real       marcosv/ffhq-dataset           FFHQ (Karras et al. 2018), HF copy is JPEG, CC BY-NC-SA 4.0
  real       Oliver1515/ProGAN-Eval         label 0_real: LSUN (2015) photos, no card license
  gan        Oliver1515/ProGAN-Eval         label 1_fake: ProGAN LSUN samples (only 45 exist), no card license
  gan        gojay/StyleGAN2-Face           StyleGAN2 faces, no card license
  diffusion  Photoroom/midjourney-v6-recap  Midjourney v6, MIT
  diffusion  bitmind/GenImage_MidJourney    GenImage Midjourney subset, no card license

Budget: at most MAX_DOWNLOADS files in total, and two consecutive failures stop the run.
"""
import io
import json
import os
import sys
import urllib.request

from .hf import hf_rows_page, scan_order, spread_page_offsets

ROOT = os.path.join(os.path.dirname(__file__), ".cache", "images")
PAGES = os.path.join(ROOT, "_pages")
MIN_SIDE = 256
MAX_DOWNLOADS = 400
MAX_CONSECUTIVE_FAILURES = 2
RESIZED = 30

# (class dir, dataset, split, keep(row) predicate, wanted)
SPECS = [
    ("real", "marcosv/ffhq-dataset", "train", lambda r: True, 30),
    ("real", "Oliver1515/ProGAN-Eval", "train", lambda r: r.get("label") == 0, 30),
    ("gan", "Oliver1515/ProGAN-Eval", "train", lambda r: r.get("label") == 1, 40),
    ("gan", "gojay/StyleGAN2-Face", "train", lambda r: True, 30),
    ("diffusion", "Photoroom/midjourney-v6-recap", "train", lambda r: True, 30),
    ("diffusion", "bitmind/GenImage_MidJourney", "train", lambda r: True, 30),
]

state = {"downloads": 0, "failures": 0}


def extension(data):
    if data[:8] == b"\x89PNG\r\n\x1a\n":
        return "png"
    if data[:3] == b"\xff\xd8\xff":
        return "jpg"
    return None


def download(url):
    if state["downloads"] >= MAX_DOWNLOADS:
        raise SystemExit(f"download cap of {MAX_DOWNLOADS} reached")
    state["downloads"] += 1
    try:
        with urllib.request.urlopen(url, timeout=60) as resp:
            data = resp.read()
    except OSError as e:
        state["failures"] += 1
        sys.stderr.write(f"download failed: {e}\n")
        if state["failures"] >= MAX_CONSECUTIVE_FAILURES:
            raise SystemExit("two consecutive failures, stopping")
        return None
    state["failures"] = 0
    return data


def pages_dir(dataset):
    return os.path.join(PAGES, dataset.replace("/", "_"))


def rows_for(dataset, split):
    """Yield rows in golden-ratio page order; a page fetch failure is a hard stop."""
    probe = hf_rows_page(dataset, "default", split, 0, 1, pages_dir(dataset), dataset, False)
    total = probe["num_rows_total"]
    windows = spread_page_offsets(total, min(60, -(-total // 100)))
    for i in scan_order(len(windows)):
        offset, length = windows[i]
        page = hf_rows_page(dataset, "default", split, offset, length, pages_dir(dataset), dataset, False)
        for r in page["rows"]:
            yield r["row_idx"], r["row"]


def fetch_class(cls, dataset, split, keep, wanted):
    out = os.path.join(ROOT, cls)
    os.makedirs(out, exist_ok=True)
    total = hf_rows_page(dataset, "default", split, 0, 1, pages_dir(dataset), dataset, False)["num_rows_total"]
    per_page = wanted if total < 1000 else 3
    got, per_page_seen = 0, {}
    for idx, row in rows_for(dataset, split):
        if got >= wanted:
            break
        img = row.get("image")
        if not isinstance(img, dict) or not keep(row):
            continue
        if min(img.get("width", 0), img.get("height", 0)) < MIN_SIDE:
            continue
        page = idx // 100
        if per_page_seen.get(page, 0) >= per_page:
            continue
        name = f"{dataset.replace('/', '_')}_{idx}"
        if any(f.startswith(name + ".") for f in os.listdir(out)):
            got += 1
            per_page_seen[page] = per_page_seen.get(page, 0) + 1
            continue
        data = download(img["src"])
        ext = data and extension(data)
        if not ext:
            continue
        with open(os.path.join(out, f"{name}.{ext}"), "wb") as fh:
            fh.write(data)
        got += 1
        per_page_seen[page] = per_page_seen.get(page, 0) + 1
    print(f"{cls} <- {dataset}: {got}/{wanted}")


def make_resized():
    from PIL import Image

    src, dst = os.path.join(ROOT, "real"), os.path.join(ROOT, "real_resized")
    os.makedirs(dst, exist_ok=True)
    files = sorted(os.listdir(src))[:RESIZED]
    for f in files:
        im = Image.open(os.path.join(src, f))
        im.load()
        small = im.convert("RGB").resize((im.width // 2, im.height // 2), Image.LANCZOS)
        stem = os.path.splitext(f)[0]
        small.save(os.path.join(dst, f"{stem}.png"))
        small.save(os.path.join(dst, f"{stem}.jpg"), quality=75)


def main():
    for spec in SPECS:
        fetch_class(*spec)
    make_resized()
    for cls in ("real", "gan", "diffusion", "real_resized"):
        d = os.path.join(ROOT, cls)
        print(cls, len(os.listdir(d)) if os.path.isdir(d) else 0)
    print(f"downloads used: {state['downloads']}/{MAX_DOWNLOADS}")


if __name__ == "__main__":
    main()
