// ABOUTME: `perf.bench` end to end (ADR-106): on git histories whose branch makes a benchmark allocate (Go) or sleep
// ABOUTME: (Python), --changed measures the merge base in an export, reports the regression, and the gate fails on it.
use std::fs;
use std::path::Path;
use std::process::Command;

use cupel_core::{Envelope, find_program};
use cupel_fixtures::{copy_repo, run_cupel_measuring};

const CHECK: &str = "perf.bench";

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// The fixture of `lang` on `main` with `config` as its `cupel.toml`, and branch `work` where
/// `file` has `from` replaced by `to`; the base commit.
fn history(
    lang: &str,
    config: &str,
    file: &str,
    from: &str,
    to: &str,
) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    copy_repo(&format!("{CHECK}/{lang}"), dir.path());
    fs::write(dir.path().join("cupel.toml"), config).unwrap();
    git(dir.path(), &["init", "--quiet", "--initial-branch", "main"]);
    git(dir.path(), &["add", "--all"]);
    git(dir.path(), &["commit", "--quiet", "-m", "base"]);
    let base = git(dir.path(), &["rev-parse", "HEAD"]);
    git(dir.path(), &["checkout", "--quiet", "-b", "work"]);
    let code = dir.path().join(file);
    let text = fs::read_to_string(&code).unwrap();
    assert!(text.contains(from), "{file} has `{from}`");
    fs::write(&code, text.replace(from, to)).unwrap();
    git(dir.path(), &["commit", "--quiet", "--all", "-m", "slower"]);
    (dir, base)
}

/// `check --only perf.bench --changed` in `dir`: its exit code and envelope. It measures wall
/// time, so it runs cold only, not through the parity daemon (`run_cupel_measuring`).
fn changed(dir: &Path) -> (Option<i32>, Envelope) {
    let outcome = run_cupel_measuring(
        &["check", "--only", CHECK, "--changed", "--format", "json"],
        dir,
    );
    (outcome.code, outcome.envelope().clone())
}

/// (kind, benchmark, unit) of every finding, sorted.
fn reported(envelope: &Envelope) -> Vec<(String, String, String)> {
    let field = |finding: &cupel_core::Finding, key: &str| {
        finding.evidence.0[key].as_str().unwrap_or("-").to_owned()
    };
    let mut reported: Vec<(String, String, String)> = envelope
        .findings
        .iter()
        .map(|f| (f.kind.clone(), field(f, "benchmark"), field(f, "unit")))
        .collect();
    reported.sort();
    reported
}

#[test]
fn an_allocation_the_branch_adds_is_a_regression_and_fails_the_gate() {
    if find_program("go").is_none() {
        eprintln!("SKIPPED: go not installed; perf.bench did not compare Go benchmarks");
        return;
    }
    let (dir, base) = history(
        "go",
        "[gate.changed]\n\"perf.bench.regression\" = \"< 5%\"\n",
        "sum.go",
        "\ttotal := 0\n",
        "\tvalues = append([]int(nil), values...)\n\ttotal := 0\n",
    );
    let (code, envelope) = changed(dir.path());
    let name = "example.com/units.BenchmarkSum".to_owned();
    let found = reported(&envelope);
    for unit in ["allocs", "bytes"] {
        let expected = ("perf.regression".to_owned(), name.clone(), unit.to_owned());
        assert!(found.contains(&expected), "{found:?}");
    }
    let regression = &envelope.findings[0];
    assert_eq!(
        regression.repro,
        format!("cupel check --only {CHECK} --base {base}")
    );
    let location = regression.location.as_ref().unwrap();
    assert_eq!(location.file, "sum_test.go");
    let result = &envelope.checks[0];
    assert!(result.metrics["regression"] > 0.05, "{:?}", result.metrics);
    assert!(result.notes.is_empty(), "{:?}", result.notes);
    assert_eq!(code, Some(1), "{:?}", envelope.summary.gate);
    let gate = envelope.summary.gate.as_ref().unwrap();
    assert_eq!(gate.rules[0].rule, "perf.bench.regression < 5%");
}

#[test]
fn a_benchmark_twenty_times_slower_at_head_is_a_regression() {
    let installed = Command::new("python3")
        .args(["-c", "import pytest_benchmark"])
        .status()
        .is_ok_and(|status| status.success());
    if !installed {
        eprintln!("SKIPPED: pytest-benchmark not installed; perf.bench did not compare Python");
        return;
    }
    let (dir, _) = history(
        "python",
        "",
        "units.py",
        "    return sum(values)\n",
        "    time.sleep(0.002)\n    return sum(values)\n",
    );
    let code = dir.path().join("units.py");
    let text = fs::read_to_string(&code).unwrap();
    fs::write(&code, format!("import time\n\n\n{text}")).unwrap();
    git(
        dir.path(),
        &["commit", "--quiet", "--all", "-m", "import time"],
    );
    let (code, envelope) = changed(dir.path());
    assert_eq!(code, Some(0));
    assert_eq!(
        reported(&envelope),
        [(
            "perf.regression".to_owned(),
            "tests/test_bench.py::test_total".to_owned(),
            "ns".to_owned()
        )]
    );
    let change = envelope.findings[0].evidence.0["change"].as_f64().unwrap();
    assert!(change > 19.0, "{change}");
}
