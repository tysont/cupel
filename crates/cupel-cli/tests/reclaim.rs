// ABOUTME: Tests for `make reclaim`, which removes the build outputs a checkout regenerates on demand (the coverage
// ABOUTME: build, the incremental caches, the suite cache), so idle worktrees do not fill the disk.
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The commands `make -n reclaim` would run.
fn reclaim() -> String {
    let output = Command::new("make")
        .args(["-n", "--no-print-directory", "reclaim"])
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS")
        .env_remove("MAKELEVEL")
        .current_dir(root())
        .output()
        .expect("make runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// A coverage run is a second build of the workspace, the incremental caches grow with every edit,
/// and the suite cache holds each tree state's coverage artifacts; all three are rebuilt when needed.
/// The compiled dependencies, the fact store, and the baseline stay.
#[test]
fn make_reclaim_removes_the_coverage_build_the_incremental_caches_and_the_suite_cache() {
    let commands = reclaim();
    for path in [
        "target/cupel-llvm-cov",
        "target/debug/incremental",
        "target/release/incremental",
        ".cupel/cache",
    ] {
        assert!(
            commands
                .lines()
                .any(|line| line.starts_with("rm -rf ")
                    && line.split_whitespace().any(|w| w == path)),
            "{path} is removed:\n{commands}"
        );
    }
    for kept in [
        "target/debug/deps",
        ".cupel/cupel.db",
        ".cupel/baseline.json",
        "target/debug/cupel",
    ] {
        assert!(!commands.contains(kept), "{kept} is kept:\n{commands}");
    }
}
