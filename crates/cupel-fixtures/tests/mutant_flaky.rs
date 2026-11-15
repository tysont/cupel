// ABOUTME: A test that fails with no mutant applied is left out of every mutant's run, end to end (ADR-219): the real
// ABOUTME: binary reports one `mutant.flaky-test-excluded` at info and scores the mutants by the other tests.
use cupel_core::{CheckStatus, Envelope, Severity};
use cupel_fixtures::{copy_repo, run_cupel};

const FLAKY: &str = "tests/test_overlay.py::test_outside_mutation_slots";

/// Each finding of `envelope` of `kind`.
fn of_kind<'a>(envelope: &'a Envelope, kind: &str) -> Vec<&'a cupel_core::Finding> {
    envelope
        .findings
        .iter()
        .filter(|finding| finding.kind == kind)
        .collect()
}

#[test]
fn a_test_failing_with_no_mutant_is_left_out_of_every_mutant_with_a_finding() {
    if cupel_core::find_program("pytest").is_none() {
        eprintln!("SKIPPED: pytest not installed; the mutant-flaky fixture was not run");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo("mutant-flaky/python", dir.path());
    let outcome = run_cupel(
        &["check", "--only", "tests.mutation", "--format", "json"],
        dir.path(),
    );
    let envelope = outcome.envelope();
    let mutation = &envelope.checks[0];
    assert!(
        matches!(mutation.status, CheckStatus::Pass | CheckStatus::Fail),
        "{}",
        outcome.stdout
    );

    let excluded = of_kind(envelope, "mutant.flaky-test-excluded");
    assert_eq!(excluded.len(), 1, "{}", outcome.stdout);
    let finding = excluded[0];
    assert_eq!(
        (
            finding.severity,
            finding.evidence.0["test"].as_str(),
            finding.evidence.0["file"].as_str(),
        ),
        (Severity::Info, Some(FLAKY), Some("tests/test_overlay.py"))
    );

    // The left-out test kills every mutant, since it fails whatever is mutated. Without it,
    // `test_five_is_positive` checks `is_positive` only far from its boundary, so `>` → `>=`
    // and `0` → `1` survive, while `test_adds` kills the mutants of `add`.
    let survivors: Vec<String> = of_kind(envelope, "mutant.survived")
        .iter()
        .map(|finding| finding.title.clone())
        .collect();
    assert_eq!(
        survivors,
        [
            "Mutant survived: `>` → `>=` at calc.py:6",
            "Mutant survived: `0` → `1` at calc.py:6",
        ],
        "{}",
        outcome.stdout
    );
    let killed = mutation.metrics["killed"];
    assert!(killed >= 1.0, "{}", outcome.stdout);
    let score = mutation.score.as_ref().unwrap().value;
    assert!(score < 1.0, "{score}");
}
