// ABOUTME: A root's coverage and mutation count a dependent root's tests through a per-test map, end to end (ADR-179,
// ABOUTME: ADR-194): `checkout`'s tests kill and cover `fees`' code with one; without one, `fees` has its own tests only.
use cupel_core::Envelope;
use cupel_fixtures::{copy_repo, run_cupel};
use serde_json::Value;

/// `tests.mutation` and `tests.coverage` at `depth` on a fresh copy of the fixture; `None` (reported as skipped)
/// without pytest.
fn check(depth: &str) -> Option<Envelope> {
    if cupel_core::find_program("pytest").is_none() {
        eprintln!("SKIPPED: pytest not installed; the dependent-tests fixture was not run");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo("dependent-tests", dir.path());
    let outcome = run_cupel(
        &[
            "check",
            "--only",
            "tests.mutation",
            "--only",
            "tests.coverage",
            "--depth",
            depth,
            "--no-gate",
            "--format",
            "json",
        ],
        dir.path(),
    );
    assert_eq!(
        outcome.code,
        Some(0),
        "{}{}",
        outcome.stdout,
        outcome.stderr
    );
    Some(outcome.envelope().clone())
}

/// The metric `name` of check `id`.
fn metric(envelope: &Envelope, id: &str, name: &str) -> Option<f64> {
    let result = envelope
        .checks
        .iter()
        .find(|result| result.id.as_str() == id)?;
    result.metrics.get(name).copied()
}

/// Each finding's kind, title, and the root its evidence says ran its own tests only.
fn findings(envelope: &Envelope) -> Vec<(String, String, Value)> {
    envelope
        .findings
        .iter()
        .map(|finding| {
            let own = finding.evidence.0.get("own_root").cloned();
            (
                finding.kind.clone(),
                finding.title.clone(),
                own.unwrap_or(Value::Null),
            )
        })
        .collect()
}

#[test]
fn with_a_per_test_map_the_dependent_roots_tests_kill_and_cover_the_depended_on_roots_code() {
    let Some(envelope) = check("deep") else {
        return;
    };
    assert_eq!(findings(&envelope), [], "{envelope:?}");
    assert_eq!(
        (
            metric(&envelope, "tests.mutation", "killed"),
            metric(&envelope, "tests.mutation", "survived"),
            metric(&envelope, "tests.mutation", "uncovered"),
            metric(&envelope, "tests.coverage", "line"),
        ),
        (Some(4.0), Some(0.0), Some(0.0), Some(1.0))
    );
}

#[test]
fn without_a_per_test_map_each_root_runs_its_own_tests_and_its_findings_say_so() {
    let Some(envelope) = check("standard") else {
        return;
    };
    let fees = Value::from("fees");
    assert_eq!(
        findings(&envelope),
        [
            (
                "mutant.survived".to_owned(),
                "Mutant survived: `>=` → `>` at fees/fees.py:4".to_owned(),
                fees.clone(),
            ),
            (
                "mutant.uncovered".to_owned(),
                "No test runs fees/fees.py:5; its mutant `0` → `1` was not run".to_owned(),
                fees.clone(),
            ),
            (
                "coverage.uncovered-symbol".to_owned(),
                "`fees.fees.waived` never ran under test: none of its 1 instrumented lines"
                    .to_owned(),
                fees.clone(),
            ),
            (
                "mutant.uncovered".to_owned(),
                "No test runs fees/fees.py:10; its mutant `==` → `!=` was not run".to_owned(),
                fees,
            ),
        ],
        "{envelope:?}"
    );
    let survived = &envelope.findings[0];
    assert_eq!(
        (
            survived.action.summary.as_str(),
            &survived.evidence.0["all_tests"]
        ),
        (
            "Add a test that fails when `>=` at fees/fees.py:4 becomes `>` (own tests of `fees` \
             only; no per-test map).",
            &Value::Bool(true)
        )
    );
}
