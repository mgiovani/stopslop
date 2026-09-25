"""Git-backed cells: clone once (by tag or by cutoff date), read files out of the clone, land
both atomically the same way `common.fetch` lands a download."""
import fnmatch
import os
import shutil
import subprocess

from .common import _slug, read_text_files


def git_clone_ref(repo, ref, dest, what, skip_failures):
    """`git clone --depth 1 [--branch ref]` once, landed atomically.

    A cache hit (`dest/.git` already present) skips the network entirely. Otherwise this
    clones into `dest + '.part'` and only `os.replace`s it onto `dest` once `git clone` exits
    0, so a clone killed halfway never leaves a `.git` directory the next run's cache check
    would mistake for a complete one; a leftover `.part` from an earlier kill is removed and
    redone rather than reused.
    """
    if os.path.isdir(os.path.join(dest, ".git")):
        return dest
    part = f"{dest}.part"
    shutil.rmtree(part, ignore_errors=True)
    os.makedirs(os.path.dirname(dest) or ".", exist_ok=True)
    cmd = ["git", "clone", "--depth", "1", "--quiet"]
    if ref:
        cmd += ["--branch", ref]
    cmd += [repo, part]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        shutil.rmtree(part, ignore_errors=True)
        if skip_failures:
            return None
        raise SystemExit(f"{what}: git clone failed:\n{proc.stderr}")
    os.replace(part, dest)
    return dest


def git_clone_before(repo, cutoff_date, dest, what, skip_failures):
    """Clone the default branch, check out its last commit before `cutoff_date`, and land the
    result atomically.

    Used for a repo with no tag near the wanted date (rust-lang/book): `--shallow-since` one
    year back keeps the clone small while still reaching a commit before `cutoff_date`. Clone
    and checkout both happen inside `dest + '.part'`, moved onto `dest` only once both exit 0,
    matching `git_clone_ref`'s atomicity; a leftover `.part` is removed and redone.
    """
    if os.path.isdir(os.path.join(dest, ".git")):
        return dest
    part = f"{dest}.part"
    shutil.rmtree(part, ignore_errors=True)
    os.makedirs(os.path.dirname(dest) or ".", exist_ok=True)
    since = f"{int(cutoff_date[:4]) - 1}-{cutoff_date[5:]}"
    proc = subprocess.run(
        ["git", "clone", "--quiet", f"--shallow-since={since}", repo, part],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        shutil.rmtree(part, ignore_errors=True)
        if skip_failures:
            return None
        raise SystemExit(f"{what}: git clone failed:\n{proc.stderr}")
    log = subprocess.run(
        ["git", "-C", part, "log", f"--until={cutoff_date}T00:00:00", "-1", "--format=%H"],
        capture_output=True, text=True,
    )
    sha = log.stdout.strip()
    if not sha:
        shutil.rmtree(part, ignore_errors=True)
        if skip_failures:
            return None
        raise SystemExit(f"{what}: no commit before {cutoff_date} in the shallow history")
    checkout = subprocess.run(["git", "-C", part, "checkout", "--quiet", sha], capture_output=True, text=True)
    if checkout.returncode != 0:
        shutil.rmtree(part, ignore_errors=True)
        if skip_failures:
            return None
        raise SystemExit(f"{what}: git checkout failed:\n{checkout.stderr}")
    os.replace(part, dest)
    return dest


def git_head_sha(clone_dir):
    return subprocess.run(
        ["git", "-C", clone_dir, "rev-parse", "--short=12", "HEAD"],
        capture_output=True, text=True,
    ).stdout.strip()


def git_files(clone_dir, subdirs, glob_pat, exclude=()):
    """Every file under `clone_dir/<subdir>` for each of `subdirs` (one string or several)
    matching `glob_pat`, minus any path containing an `exclude` substring, sorted for
    determinism. Several subdirs exist for Rust 1.40, whose std lived in three `src/lib*`
    crates before the `library/` move."""
    if isinstance(subdirs, str):
        subdirs = (subdirs,)
    matches = []
    for subdir in subdirs:
        root = os.path.join(clone_dir, subdir)
        if not os.path.isdir(root):
            continue
        for dirpath, _dirnames, filenames in os.walk(root):
            for name in fnmatch.filter(filenames, glob_pat):
                rel = os.path.relpath(os.path.join(dirpath, name), clone_dir).replace(os.sep, "/")
                if any(x in rel for x in exclude):
                    continue
                matches.append(rel)
    return sorted(matches)


def shared_clone_dir(cache_dir, repo, ref):
    """One clone per (repo, ref), shared across every dataset that names it -- cpython-lib
    and cpython-doc name the same tag of the same repo and would otherwise pay for (and
    store) the same ~120 MB clone twice."""
    git_cache = os.path.join(os.path.dirname(cache_dir), "_git")
    return os.path.join(git_cache, _slug(f"{repo}@{ref or ''}"))


def materialize_from_clone(ds, cell, clone_dir, limit, skip_failures):
    """`git_files` + `read_text_files` tail shared by every git-based fetcher once its clone
    dir is ready -- `fetch_git_cell` and `fetch_git_before_cell` differ only in how they get
    there."""
    files = git_files(clone_dir, cell["subdir"], cell["glob"], ds.get("exclude", ()))
    if not files:
        if skip_failures:
            return None
        raise SystemExit(f"{ds['name']}/{cell['label']}/{cell['lang']}: 0 files under {cell['subdir']}")
    return read_text_files(clone_dir, files, limit)


def clone_dir_for(ds, cache_dir):
    """One clone per (repo, tag-or-cutoff); a repo pinned by date gets its own key so moving
    the cutoff re-clones instead of reusing the checkout the old cutoff produced."""
    ref = ds.get("tag") or (f"before-{ds['before']}" if ds.get("before") else None)
    return shared_clone_dir(cache_dir, ds["repo"], ref)


def fetch_git_cell(ds, cell, limit, cache_dir, skip_failures):
    clone_dir = clone_dir_for(ds, cache_dir)
    if git_clone_ref(ds["repo"], ds.get("tag"), clone_dir, ds["name"], skip_failures) is None:
        return None
    return materialize_from_clone(ds, cell, clone_dir, limit, skip_failures)


def fetch_git_before_cell(ds, cell, limit, cache_dir, skip_failures):
    clone_dir = clone_dir_for(ds, cache_dir)
    if git_clone_before(ds["repo"], ds["before"], clone_dir, ds["name"], skip_failures) is None:
        return None
    return materialize_from_clone(ds, cell, clone_dir, limit, skip_failures)
