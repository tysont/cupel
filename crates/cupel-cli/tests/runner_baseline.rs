// ABOUTME: Exit 4 through the CLI's own entry point (ADR-076 §8): a test-only check that needs `TestResults`
// ABOUTME: is submitted from this test binary, then `check --only` runs the real runner a fixture names.
use std::collections::BTreeMap;
use std::path::Path;

use cupel_core::{
    Check, CheckCtx, CheckId, CheckResult, CheckStatus, CostClass, EventSink, FactKind, Facts,
    Manifest, Plan, Result, RunId, Scope, Score, Stability, TestOutcome, TestResults, Tier, Writer,
    find_program,
};
use cupel_fixtures::copy_repo;
use cupel_store::Store;
use serde_json::{Value, json};

const ID: &str = "test.needs-test-results";

/// Scores the baseline: how many of its tests passed, out of how many ran.
struct NeedsTestResults(Manifest);

inventory::submit!(cupel_checks::NativeCheck(|| Box::new(NeedsTestResults(
    Manifest {
        id: CheckId::from(ID),
        version: 1,
        tier: Tier::try_from(2).unwrap(),
        cost: CostClass::Standard,
        needs: vec![FactKind::TestResults],
        wants: Vec::new(),
        findings: Vec::new(),
        metrics: Vec::new(),
        stability: Stability::Experimental,
        summary: "Counts the baseline's passing tests (test binary only).".to_owned(),
    }
))));

impl Check for NeedsTestResults {
    fn manifest(&self) -> &Manifest {
        &self.0
    }

    fn plan(&self, _: &CheckCtx, _: &Scope) -> Result<Plan> {
        Ok(Plan::estimate(CostClass::Standard, 1))
    }

    fn run(
        &self,
        _: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        _: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        let results = facts.get::<TestResults>()?;
        let passed = results
            .tests
            .iter()
            .filter(|test| test.outcome == TestOutcome::Passed)
            .count();
        Ok(CheckResult {
            id: self.0.id.clone(),
            status: CheckStatus::Pass,
            tier: self.0.tier,
            cost: self.0.cost,
            score: Some(Score {
                value: f64::from(u32::try_from(passed).unwrap()),
                unit: "passed".to_owned(),
                n: results.tests.len() as u64,
            }),
            metrics: BTreeMap::new(),
            delta: None,
            findings: 0,
            duration_ms: 0,
            skipped: None,
            oracle: None,
            notes: Vec::new(),
        })
    }
}

/// A copy of the python test-results fixture (red when `red`) whose `cupel.toml` holds `toml`;
/// `None` (reported as skipped) when `program` is not installed.
fn python_repo(program: &str, red: bool, toml: &str) -> Option<tempfile::TempDir> {
    if find_program(program).is_none() {
        eprintln!("SKIPPED: {program} not installed");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo("test-results/python", dir.path());
    if red {
        copy_repo("test-results/red/python", dir.path());
    }
    std::fs::write(dir.path().join("cupel.toml"), toml).unwrap();
    Some(dir)
}

const UNITTEST: &str = "[lang.python]\nrunner = \"unittest\"\n";

/// `cupel <args> --format json --cwd <repo>` through `cupel_cli::run`: its exit code and envelope.
fn cupel(repo: &Path, args: &[&str]) -> (u8, Value) {
    let mut words: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    words.extend(["--format".to_owned(), "json".to_owned(), "--cwd".to_owned()]);
    words.push(repo.display().to_string());
    let output = cupel_cli::run(&cupel_cli::Environment::new(
        words.into_iter().map(Into::into).collect(),
        Some(repo.to_path_buf()),
    ));
    let envelope = serde_json::from_str(&output.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}{}", output.stdout, output.stderr));
    (output.code, envelope)
}

/// The test rows the store indexed for the run of `envelope`: `(test, outcome, runner)`.
fn stored(repo: &Path, envelope: &Value) -> Vec<(String, String, String)> {
    let run: RunId = envelope["run"]["id"].as_str().unwrap().parse().unwrap();
    let store = Store::open(&Writer::new(repo, false).unwrap()).unwrap();
    store
        .test_results(&run)
        .unwrap()
        .into_iter()
        .map(|row| (row.test, row.outcome, row.runner.to_string()))
        .collect()
}

fn row(test: &str, outcome: &str, runner: &str) -> (String, String, String) {
    (test.to_owned(), outcome.to_owned(), runner.to_owned())
}

#[test]
fn a_green_baseline_is_scored_and_indexed() {
    let Some(repo) = python_repo("python3", false, UNITTEST) else {
        return;
    };
    let (code, envelope) = cupel(repo.path(), &["check", "--only", ID]);
    assert_eq!(
        (code, &envelope["status"], &envelope["checks"][0]["score"]),
        (
            0,
            &json!("pass"),
            &json!({"value": 1.0, "unit": "passed", "n": 2})
        )
    );
    assert_eq!(
        stored(repo.path(), &envelope),
        [
            row(
                "tests.test_tally.TotalTest.test_totals_a_huge_list",
                "skipped",
                "tool.unittest"
            ),
            row(
                "tests.test_tally.TotalTest.test_totals_a_list",
                "passed",
                "tool.unittest"
            ),
        ]
    );
}

#[test]
fn a_red_baseline_is_exit_4_with_no_scores_and_a_finding_per_failing_test() {
    let Some(repo) = python_repo("python3", true, UNITTEST) else {
        return;
    };
    let (code, envelope) = cupel(repo.path(), &["check", "--only", ID]);
    assert_eq!(
        (
            code,
            &envelope["status"],
            &envelope["exit_code"],
            &envelope["checks"]
        ),
        (4, &json!("error"), &json!(4), &json!([]))
    );
    let findings: Vec<(&Value, &Value, &Value, &Value, &Value)> = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            let evidence = &finding["evidence"];
            (
                &finding["kind"],
                &evidence["test"],
                &evidence["file"],
                &evidence["runner"],
                &finding["action"]["kind"],
            )
        })
        .collect();
    let failing = json!("tests.test_red.RedTest.test_reports_a_wrong_total");
    assert_eq!(
        findings,
        [(
            &json!("baseline.test-failed"),
            &failing,
            &json!("tests/test_red.py"),
            &json!("tool.unittest"),
            &json!("fix_bug")
        )]
    );
    assert_eq!(
        stored(repo.path(), &envelope)[0],
        row(
            "tests.test_red.RedTest.test_reports_a_wrong_total",
            "failed",
            "tool.unittest"
        )
    );
}

#[test]
fn without_a_pin_the_detected_runner_runs_the_suite() {
    let Some(repo) = python_repo("pytest", true, "") else {
        return;
    };
    let (code, envelope) = cupel(repo.path(), &["check", "--only", ID]);
    assert_eq!(
        (
            code,
            &envelope["findings"][0]["evidence"]["runner"],
            &envelope["findings"][0]["evidence"]["test"]
        ),
        (
            4,
            &json!("tool.pytest"),
            &json!("tests/test_red.py::RedTest::test_reports_a_wrong_total")
        )
    );
}

#[test]
fn doctor_reports_the_version_of_the_runner_the_checks_need() {
    let Some(repo) = python_repo("python3", false, UNITTEST) else {
        return;
    };
    let (code, envelope) = cupel(repo.path(), &["doctor"]);
    let tools: Vec<&str> = envelope["run"]["toolchain"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!((code, tools), (0, vec!["python3"]));
    assert_eq!(
        envelope["run"]["toolchain"]["python3"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .next(),
        Some("Python")
    );
}

#[test]
fn a_pin_naming_no_runner_skips_the_check_and_doctor_names_the_key() {
    let Some(repo) = python_repo("python3", false, "[lang.python]\nrunner = \"nose\"\n") else {
        return;
    };
    let (_, checked) = cupel(repo.path(), &["check", "--only", ID]);
    let (code, doctor) = cupel(repo.path(), &["doctor"]);
    let problem = doctor["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["kind"] == json!("doctor.producer-unknown"))
        .map(|finding| &finding["evidence"]["key"]);
    assert_eq!(
        (&checked["checks"][0]["skipped"]["reason"], code, problem),
        (
            &json!("unconfigured"),
            3,
            Some(&json!("lang.python.runner"))
        )
    );
}
