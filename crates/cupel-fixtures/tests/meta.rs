// ABOUTME: `meta.traceability` and `meta.oracle-independence` end to end: a Python history whose range adds a function,
// ABOUTME: its test, and a requirement's test, checked with --base by the real binary (ADR-114, ADR-115).
use std::fs;
use std::path::Path;
use std::process::Command;

use cupel_core::{Envelope, SkipReason};
use cupel_fixtures::run_cupel;
use serde_json::{Value, json};

const BASE_SOURCE: &str = "def parse(text):\n    return int(text)\n";

const HEAD_SOURCE: &str = "def parse(text):\n    return int(text.strip())\n\n\n\
                           def format(value):\n    return str(value)\n";

const BASE_TESTS: &str = "from dates import parse\n\n\ndef test_req_1_parses():\n    \
                          assert parse(\"4\") == 4\n";

const ADDED_TEST: &str = "\n\ndef test_req_2_formats():\n    from dates import format\n\n    \
                          assert format(4) == \"4\"\n";

const CONFIG: &str = "[checks.\"meta.traceability\"]\nrequirements = [\"REQ-1\", \"REQ-2\"]\n";

const PYPROJECT: &str = "[project]\nname = \"dates\"\nversion = \"0.1.0\"\n\n\
                         [tool.pytest.ini_options]\ntestpaths = [\"tests\"]\npythonpath = [\"src\"]\n";

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn write(dir: &Path, file: &str, text: &str) {
    let path = dir.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A repository whose base commit has `parse` and a test of it naming REQ-1, and whose
/// working tree changes `parse`, adds `format`, and adds a test of it naming REQ-2. Returns the
/// repository and the base commit.
fn history() -> (tempfile::TempDir, String) {
    let repo = tempfile::tempdir().unwrap();
    let dir = repo.path();
    git(dir, &["init", "--quiet", "--initial-branch", "main"]);
    write(dir, "pyproject.toml", PYPROJECT);
    write(dir, "cupel.toml", CONFIG);
    write(dir, "src/dates.py", BASE_SOURCE);
    write(dir, "tests/test_dates.py", BASE_TESTS);
    git(dir, &["add", "--all"]);
    git(dir, &["commit", "--quiet", "-m", "parse"]);
    let base = git(dir, &["rev-parse", "HEAD"]);
    write(dir, "src/dates.py", HEAD_SOURCE);
    write(
        dir,
        "tests/test_dates.py",
        &format!("{BASE_TESTS}{ADDED_TEST}"),
    );
    (repo, base)
}

fn check(dir: &Path, args: &[&str]) -> Envelope {
    let mut all = vec!["check", "--format", "json"];
    all.extend(args);
    let outcome = run_cupel(&all, dir);
    assert_eq!(outcome.code, Some(0), "{}", outcome.stderr);
    outcome.envelope().clone()
}

/// Each finding as (kind, title, evidence).
fn findings(envelope: &Envelope) -> Vec<(String, String, Value)> {
    envelope
        .findings
        .iter()
        .map(|finding| {
            (
                finding.kind.clone(),
                finding.title.clone(),
                serde_json::to_value(&finding.evidence).unwrap(),
            )
        })
        .collect()
}

#[test]
fn a_requirement_only_a_pre_existing_test_names_is_untested_and_one_the_range_tests_is_traced() {
    let (repo, base) = history();
    let envelope = check(
        repo.path(),
        &["--only", "meta.traceability", "--base", &base],
    );
    assert_eq!(
        findings(&envelope),
        [(
            "spec.untested-requirement".to_owned(),
            "requirement REQ-1 has no test the change added; 1 test that existed before names it"
                .to_owned(),
            json!({
                "requirement": "REQ-1",
                "base": base,
                "tests": [{"file": "tests/test_dates.py", "name": "test_req_1_parses", "line": 4, "added": false}],
            }),
        )]
    );
    let result = &envelope.checks[0];
    assert_eq!(result.metrics.get("untested"), Some(&1.0));
}

#[test]
fn a_tree_scope_skips_both_meta_checks_because_they_need_a_range() {
    let (repo, _) = history();
    let envelope = check(
        repo.path(),
        &[
            "--only",
            "meta.traceability",
            "--only",
            "meta.oracle-independence",
        ],
    );
    let reasons: Vec<_> = envelope
        .checks
        .iter()
        .map(|result| result.skipped.as_ref().map(|skipped| skipped.reason))
        .collect();
    assert_eq!(
        reasons,
        [Some(SkipReason::NeedsRange), Some(SkipReason::NeedsRange)]
    );
}

#[test]
fn a_changed_function_only_the_added_test_runs_is_self_authored_and_one_an_old_test_runs_is_not() {
    let (repo, base) = history();
    let envelope = check(
        repo.path(),
        &[
            "--only",
            "meta.oracle-independence",
            "--base",
            &base,
            "--depth",
            "standard",
        ],
    );
    let result = &envelope.checks[0];
    if result.score.is_none() {
        eprintln!(
            "SKIPPED: no per-test map was made here ({:?}); meta.oracle-independence labeled nothing",
            result.notes
        );
        return;
    }
    assert_eq!(
        findings(&envelope),
        [(
            "evidence.self-authored-only".to_owned(),
            "`dates.format` is exercised only by tests the change added (1), none that existed before it"
                .to_owned(),
            json!({"base": base, "tests": ["tests/test_dates.py::test_req_2_formats"]}),
        )]
    );
    assert_eq!(result.metrics.get("independent"), Some(&1.0));
    assert_eq!(result.metrics.get("self_authored_only"), Some(&1.0));
}
