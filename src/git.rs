use anyhow::Context;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

pub enum Scope {
    Staged,
    Changed,
    Since(String),
}

/// Files git reports as added, copied, modified or renamed (new name) under `dir`, relative to
/// `dir`. Deletions are never linted.
pub fn changed_files(
    dir: &Path,
    scope: &Scope,
    pathspecs: &[PathBuf],
) -> anyhow::Result<Vec<PathBuf>> {
    // `git diff --cached` outside a repo prints a usage dump instead of "not a git repository",
    // so ask for the repo first to surface git's own clear error.
    run(dir, &["rev-parse", "--show-toplevel"])?;

    let mut args: Vec<OsString> = vec![
        "diff".into(),
        "--name-only".into(),
        "-z".into(),
        "--relative".into(),
        "--diff-filter=ACMR".into(),
    ];
    match scope {
        Scope::Staged => args.push("--cached".into()),
        Scope::Changed => args.push("HEAD".into()),
        // Compares the merge base of `r` and HEAD against the working tree: `r` (e.g. main)
        // advancing after the branch forked doesn't make main's own files show up.
        Scope::Since(r) => {
            args.push("--merge-base".into());
            args.push(OsString::from(r));
        }
    }
    args.push("--".into());
    args.extend(pathspecs.iter().map(|p| p.as_os_str().to_os_string()));

    let stdout = run(dir, &args)?;
    Ok(stdout
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(String::from_utf8_lossy(s).into_owned()))
        .collect())
}

/// Content of `path` as staged in the index, so a partially staged file is linted as it will be
/// committed rather than as it sits on disk.
pub fn staged_source(dir: &Path, path: &Path) -> std::io::Result<String> {
    // One `git show` process per file is fine for a pre-commit-sized diff; if this ever needs to
    // scale to thousands of staged files, switch to a single `git cat-file --batch` pipe instead.
    let spec = format!(":./{}", path.display());
    let output = Command::new("git")
        .current_dir(dir)
        .args(["show", &spec])
        .output()
        .map_err(std::io::Error::other)?;
    if !output.status.success() {
        return Err(std::io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    String::from_utf8(output.stdout).map_err(std::io::Error::other)
}

fn run(dir: &Path, args: &[impl AsRef<OsStr>]) -> anyhow::Result<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .context("running git")?;
    if !output.status.success() {
        anyhow::bail!("git: {}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn git(dir: &Path, args: &[&str]) {
        // Hermetic setup: a contributor's global gpgsign or hooksPath must not break these tests.
        let out = Command::new("git")
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args(args)
            .output()
            .expect("spawn git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn commit(dir: &Path, msg: &str) {
        git(
            dir,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                msg,
            ],
        );
    }

    fn init_repo() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        dir
    }

    #[test]
    fn staged_reads_index_not_working_tree() {
        let repo = init_repo();
        let dir = repo.path();
        std::fs::write(dir.join("a b.md"), "old\n").unwrap();
        git(dir, &["add", "."]);
        commit(dir, "base");

        std::fs::write(dir.join("a b.md"), "staged\n").unwrap();
        git(dir, &["add", "."]);
        std::fs::write(dir.join("a b.md"), "working\n").unwrap();

        let files = changed_files(dir, &Scope::Staged, &[PathBuf::from(".")]).unwrap();
        assert_eq!(files, vec![PathBuf::from("a b.md")]);

        let source = staged_source(dir, Path::new("a b.md")).unwrap();
        assert_eq!(source, "staged\n");
    }

    #[test]
    fn rename_yields_new_path_and_deletion_is_dropped() {
        let repo = init_repo();
        let dir = repo.path();
        std::fs::write(dir.join("a.md"), "content\n").unwrap();
        std::fs::write(dir.join("c.md"), "bye\n").unwrap();
        git(dir, &["add", "."]);
        commit(dir, "base");

        git(dir, &["mv", "a.md", "b.md"]);
        git(dir, &["rm", "-q", "c.md"]);

        let files = changed_files(dir, &Scope::Staged, &[PathBuf::from(".")]).unwrap();
        assert_eq!(files, vec![PathBuf::from("b.md")]);
    }

    #[test]
    fn changed_covers_staged_and_unstaged() {
        let repo = init_repo();
        let dir = repo.path();
        std::fs::write(dir.join("a.md"), "a\n").unwrap();
        std::fs::write(dir.join("b.md"), "b\n").unwrap();
        git(dir, &["add", "."]);
        commit(dir, "base");

        std::fs::write(dir.join("a.md"), "a changed staged\n").unwrap();
        git(dir, &["add", "a.md"]);
        std::fs::write(dir.join("b.md"), "b changed unstaged\n").unwrap();

        let mut files = changed_files(dir, &Scope::Changed, &[PathBuf::from(".")]).unwrap();
        files.sort();
        assert_eq!(files, vec![PathBuf::from("a.md"), PathBuf::from("b.md")]);
    }

    #[test]
    fn since_uses_merge_base_not_tip() {
        let repo = init_repo();
        let dir = repo.path();
        std::fs::write(dir.join("b.md"), "b\n").unwrap();
        std::fs::write(dir.join("c.md"), "c\n").unwrap();
        git(dir, &["add", "."]);
        commit(dir, "base");

        git(dir, &["checkout", "-qb", "topic"]);
        std::fs::write(dir.join("b.md"), "b on topic\n").unwrap();
        git(dir, &["add", "."]);
        commit(dir, "topic changes b");

        git(dir, &["checkout", "-q", "main"]);
        std::fs::write(dir.join("c.md"), "c on main\n").unwrap();
        git(dir, &["add", "."]);
        commit(dir, "main advances");

        git(dir, &["checkout", "-q", "topic"]);

        let files = changed_files(
            dir,
            &Scope::Since("main".to_string()),
            &[PathBuf::from(".")],
        )
        .unwrap();
        assert_eq!(files, vec![PathBuf::from("b.md")]);
    }

    #[test]
    fn paths_are_relative_to_dir_and_outside_dir_is_excluded() {
        let repo = init_repo();
        let dir = repo.path();
        std::fs::create_dir(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/x.md"), "x\n").unwrap();
        std::fs::write(dir.join("top.md"), "top\n").unwrap();
        git(dir, &["add", "."]);

        let files = changed_files(&dir.join("sub"), &Scope::Staged, &[PathBuf::from(".")]).unwrap();
        assert_eq!(files, vec![PathBuf::from("x.md")]);

        let narrowed = changed_files(dir, &Scope::Staged, &[PathBuf::from("sub")]).unwrap();
        assert_eq!(narrowed, vec![PathBuf::from("sub/x.md")]);
    }

    #[test]
    fn staged_source_of_a_path_not_in_the_index_is_an_error() {
        let repo = init_repo();
        assert!(staged_source(repo.path(), Path::new("missing.md")).is_err());
    }

    #[test]
    fn since_with_an_unknown_ref_is_a_clear_error() {
        let repo = init_repo();
        let err = changed_files(
            repo.path(),
            &Scope::Since("nope".to_string()),
            &[PathBuf::from(".")],
        )
        .unwrap_err();
        assert!(err.to_string().contains("bad revision"), "{err}");
    }

    #[test]
    fn outside_a_repo_is_a_clear_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = changed_files(dir.path(), &Scope::Staged, &[PathBuf::from(".")]).unwrap_err();
        assert!(err.to_string().contains("git repository"));
    }

    #[test]
    fn empty_selection_is_ok() {
        let repo = init_repo();
        let dir = repo.path();
        std::fs::write(dir.join("a.md"), "a\n").unwrap();
        git(dir, &["add", "."]);
        commit(dir, "base");

        let files = changed_files(dir, &Scope::Staged, &[PathBuf::from(".")]).unwrap();
        assert_eq!(files, Vec::<PathBuf>::new());
    }
}
