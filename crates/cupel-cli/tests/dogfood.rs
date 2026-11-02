// ABOUTME: Tests for how Cupel grades itself (ADR-129): `make dogfood` runs the gate at `--depth deep`, CI's dogfood
// ABOUTME: job installs what the deep checks run through the shared tools action, and cupel.toml's gates hold the deep checks' rules.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The tree gate's first command: the committed baseline, imported into the store.
const IMPORT: &str = "cargo run --quiet --package cupel-cli -- baseline import \
                      .cupel/baseline.json --write --format text >/dev/null\n";

/// The commands `make -n dogfood <vars>` would run, with `cargo` for `$(CARGO)` (a test runs with
/// `CARGO` set to the toolchain's own path). The suite itself runs under make, in `make check` and
/// in the dogfood's own baseline run, whose command-line variables (`DOGFOOD_BUDGET=` in CI)
/// reach this make through `MAKEFLAGS`; it sees only `vars`.
fn dogfood(vars: &[&str]) -> String {
    let output = Command::new("make")
        .args(["-n", "--no-print-directory", "dogfood", "CARGO=cargo"])
        .args(vars)
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS")
        .env_remove("MAKELEVEL")
        .env_remove("DOGFOOD_BUDGET")
        .env_remove("DOGFOOD_SCOPE")
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

/// The half-cores cap every dogfood runs under, in any checkout (ADR-156 §7), as the variables
/// make puts before the command.
fn cap() -> String {
    let output = Command::new("getconf")
        .arg("_NPROCESSORS_ONLN")
        .output()
        .unwrap();
    let cores: usize = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let half = (cores / 2).max(1);
    format!("CARGO_BUILD_JOBS={half} NEXTEST_TEST_THREADS={half} RUST_TEST_THREADS={half}")
}

/// The gate's command: at half the cores, under the heavy lock, and under the temporary-directory
/// guard, which keeps this checkout's mutation slots across runs.
fn gated(check: &str) -> String {
    format!(
        "{} scripts/heavy.sh scripts/tmpdir-guard.sh --slots-of . cargo run --quiet --package \
         cupel-cli -- check {check} --format text\n",
        cap()
    )
}

/// On a branch, the changed gate at `deep`: everything deep selects but the flake engine, which
/// reruns the whole suite serially and cannot rerun nextest, and the race detector, which cannot
/// run Cupel's process-spawning tests under Miri. A local run is bounded by `DOGFOOD_BUDGET`, two hours by
/// default: a change to one source file took 83 minutes, mutation included (ADR-129).
#[test]
fn make_dogfood_runs_the_changed_gate_at_deep_depth_under_the_budget() {
    assert_eq!(
        dogfood(&[]),
        gated(
            "--changed --depth deep --budget 120m --gate --no-install --skip tests.flaky --skip \
             conc.race"
        )
    );
}

/// On `main`, the tree gate at `deep`, without mutating the whole tree, under a budget of 30 minutes,
/// against the committed baseline (ADR-140), imported first so a fresh store has the reference
/// the `base` rules need.
#[test]
fn make_dogfood_runs_the_tree_gate_at_deep_depth_without_mutating_the_tree() {
    assert_eq!(
        dogfood(&["DOGFOOD_SCOPE=tree"]),
        IMPORT.to_owned()
            + &gated(
                "--depth deep --budget 30m --gate --no-install --skip tests.flaky --skip \
                 conc.race --skip tests.mutation"
            )
    );
}

/// CI clears the budget, so both gates run every check deep selects.
#[test]
fn an_empty_dogfood_budget_runs_the_full_deep_set() {
    assert_eq!(
        dogfood(&["DOGFOOD_BUDGET="]),
        gated("--changed --depth deep --gate --no-install --skip tests.flaky --skip conc.race")
    );
    assert_eq!(
        dogfood(&["DOGFOOD_SCOPE=tree", "DOGFOOD_BUDGET="]),
        IMPORT.to_owned()
            + &gated(
                "--depth deep --gate --no-install --skip tests.flaky --skip conc.race --skip \
                 tests.mutation"
            )
    );
}

/// `make baseline` refreshes the committed baseline from the last tree run: the store's baseline
/// is set to it and exported to the file the tree dogfood imports.
#[test]
fn make_baseline_sets_and_exports_the_committed_baseline() {
    let output = Command::new("make")
        .args(["-n", "--no-print-directory", "baseline", "CARGO=cargo"])
        .current_dir(root())
        .output()
        .expect("make runs");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "cargo run --quiet --package cupel-cli -- baseline set --write --format text >/dev/null\n\
         cargo run --quiet --package cupel-cli -- baseline export .cupel/baseline.json --write \
         --format text >/dev/null\n"
    );
}

/// The committed baseline is a run of the whole tree on `main`, holding the metrics the tree
/// gate's `base` rules compare with.
#[test]
fn the_committed_baseline_is_a_tree_run_on_main_with_the_ratcheted_metrics() {
    let baseline: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root().join(".cupel/baseline.json")).unwrap())
            .unwrap();
    assert_eq!(
        (
            baseline["run"]["scope"]["mode"].as_str(),
            baseline["run"]["git"]["branch"].as_str(),
            baseline["run"]["git"]["dirty"].as_bool(),
        ),
        (Some("tree"), Some("main"), Some(false))
    );
    let clones = baseline["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "static.clones")
        .expect("a static.clones result");
    assert!(clones["metrics"]["pairs"].is_number() && clones["metrics"]["clusters"].is_number());
}

/// Both tree clone rules ratchet against the committed baseline rather than a literal.
#[test]
fn the_tree_gates_clone_rules_ratchet_against_base() {
    let config: toml_edit::DocumentMut = fs::read_to_string(root().join("cupel.toml"))
        .unwrap()
        .parse()
        .unwrap();
    let tree = &config["gate"]["tree"];
    assert_eq!(
        (
            tree["static.clones.pairs"].as_str(),
            tree["static.clones.clusters"].as_str()
        ),
        (Some("<= base"), Some("<= base"))
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

/// The dogfood job installs every tool the suite drives (the deep run executes the suite under
/// coverage as its baseline) through the action the `check` job uses, frees the disk the mutation
/// slots need, builds without debug info, as the measurements in ADR-129 ran, and clears the local
/// budget so both gates run in full.
#[test]
fn the_dogfood_job_installs_what_the_deep_checks_run() {
    let job = dogfood_job();
    let expected = [
        "timeout-minutes: 90",
        "CARGO_PROFILE_DEV_DEBUG: \"0\"",
        "run: sudo rm -rf /usr/share/dotnet /usr/local/lib/android /opt/ghc /opt/hostedtoolcache/CodeQL",
        "- uses: ./.github/actions/tools",
        "run: make dogfood DOGFOOD_BUDGET=",
        "run: make dogfood DOGFOOD_SCOPE=tree DOGFOOD_BUDGET=",
    ];
    let missing: Vec<&str> = expected
        .into_iter()
        .filter(|line| !job.iter().any(|have| have == line))
        .collect();
    assert_eq!(missing, Vec::<&str>::new(), "{job:#?}");
}

/// The `check` job installs its tools through the same action, so the suite and the dogfood's
/// baseline run of it see one tool set.
#[test]
fn the_check_job_installs_its_tools_through_the_same_action() {
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    let (_, job) = workflow.split_once("\n  check:\n").expect("a check job");
    let (job, _) = job
        .split_once("\n  dogfood:\n")
        .expect("the dogfood job after it");
    assert_eq!(
        job.lines()
            .filter(|line| line.trim() == "- uses: ./.github/actions/tools")
            .count(),
        1
    );
}
