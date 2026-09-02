//! Wall-clock guards against quadratic shapes (issue #21). Ignored by default; CI runs them
//! as a separate `cargo test --release -- --ignored` step. The bounds sit far above the
//! release-mode times on a laptop (3.3 s, 0.8 s, 0.5 s), so a slow runner passes while the
//! quadratic shapes fixed in #20 and #21 (18 s, 11 s, 35 s) fail.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn inputs() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/stress-inputs");
    if !dir.join("oneline_700k.md").exists() {
        let status = Command::new("python3")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/gen_inputs.py"))
            .arg(&dir)
            .stdout(Stdio::null())
            .status()
            .expect("python3 is needed to generate the stress inputs");
        assert!(status.success(), "bench/gen_inputs.py failed");
    }
    dir
}

fn assert_finishes_within(input: &str, bound: Duration) {
    let path = inputs().join(input);
    let start = Instant::now();
    let status = Command::new(env!("CARGO_BIN_EXE_stopslop"))
        .args(["--no-config", "--format", "json"])
        .arg(&path)
        .env("STOPSLOP_NO_UPDATE_CHECK", "1")
        .stdout(Stdio::null())
        .status()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(
        status.code().is_some_and(|c| c <= 1),
        "{input}: unexpected exit {status}"
    );
    assert!(
        elapsed <= bound,
        "{input}: took {elapsed:.2?}, bound is {bound:.2?}"
    );
}

#[test]
#[ignore]
fn headings_20mb_stays_linear() {
    assert_finishes_within("headings_20mb.md", Duration::from_secs(20));
}

#[test]
#[ignore]
fn prose_8mb_with_em_dashes_stays_fast() {
    assert_finishes_within("prose_8mb_emdash.md", Duration::from_secs(8));
}

#[test]
#[ignore]
fn one_line_file_stays_linear() {
    assert_finishes_within("oneline_700k.md", Duration::from_secs(8));
}
