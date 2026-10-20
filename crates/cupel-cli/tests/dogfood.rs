// ABOUTME: Tests for how Cupel grades itself (ADR-129): `make dogfood` runs the gate at `--depth deep`, CI's dogfood
// ABOUTME: job installs what the deep checks run, and cupel.toml's gates hold the deep checks' rules.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The commands `make -n dogfood <vars>` would run, with `cargo` for `$(CARGO)` (a test runs with
/// `CARGO` set to the toolchain's own path).
fn dogfood(vars: &[&str]) -> String {
    let output = Command::new("make")
        .args(["-n", "--no-print-directory", "dogfood", "CARGO=cargo"])
        .args(vars)
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

/// On a branch, the changed gate at `deep`: everything deep selects but the flake engine, which
/// reruns the whole suite serially and cannot rerun nextest, and the race detector, which cannot
/// run Cupel's process-spawning tests under Miri.
#[test]
fn make_dogfood_runs_the_changed_gate_at_deep_depth() {
    assert_eq!(
        dogfood(&[]),
        "scripts/dogfood.sh cargo run --quiet --package cupel-cli -- check --changed --depth deep --gate --no-install \
         --skip tests.flaky --skip conc.race --format text\n"
    );
}

/// On `main`, the tree gate at `deep`, without mutating the whole tree.
#[test]
fn make_dogfood_runs_the_tree_gate_at_deep_depth_without_mutating_the_tree() {
    assert_eq!(
        dogfood(&["DOGFOOD_SCOPE=tree"]),
        "scripts/dogfood.sh cargo run --quiet --package cupel-cli -- check --depth deep --gate --no-install \
         --skip tests.flaky --skip conc.race --skip tests.mutation --format text\n"
    );
}

/// The lines of the `dogfood` job of ci.yml, trimmed.
fn dogfood_job() -> Vec<String> {
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    let (_, job) = workflow
        .split_once("\n  dogfood:\n")
        .expect("a dogfood job");
    job.lines()
        .take_while(|line| line.is_empty() || line.starts_with("    ") || line.starts_with("  #"))
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// The dogfood job installs the runner and coverage tool the suite run and the mutation engine
/// drive (cargo-nextest, cargo-llvm-cov with the LLVM tools), frees the disk the mutation slots
/// need, and builds without debug info, as the measurements in ADR-129 ran.
#[test]
fn the_dogfood_job_installs_what_the_deep_checks_run() {
    let job = dogfood_job();
    let expected = [
        "timeout-minutes: 90",
        "CARGO_PROFILE_DEV_DEBUG: \"0\"",
        "run: sudo rm -rf /usr/share/dotnet /usr/local/lib/android /opt/ghc /opt/hostedtoolcache/CodeQL",
        "tool: cargo-nextest@0.9.148",
        "tool: cargo-llvm-cov@0.9.1",
        "run: rustup component add llvm-tools-preview",
        "run: make dogfood",
        "run: make dogfood DOGFOOD_SCOPE=tree",
    ];
    let missing: Vec<&str> = expected
        .into_iter()
        .filter(|line| !job.iter().any(|have| have == line))
        .collect();
    assert_eq!(missing, Vec::<&str>::new(), "{job:#?}");
}
