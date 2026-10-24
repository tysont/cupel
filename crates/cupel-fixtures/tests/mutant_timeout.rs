// ABOUTME: A mutant's run is capped and slow tests are left out of whole-suite runs, end to end (ADR-147): the real binary
// ABOUTME: cuts two looping mutants at `max_mutant_timeout` and leaves a 61 s Go test out of every mutant with a finding.
use cupel_core::{Envelope, Severity};
use cupel_fixtures::{copy_repo, run_cupel, run_cupel_measuring};

/// Whether `program` is on `PATH`; prints the skip line when it is not.
fn installed(program: &str, what: &str) -> bool {
    let found = cupel_core::find_program(program).is_some();
    if !found {
        eprintln!("SKIPPED: {program} not installed; {what} was not run");
    }
    found
}

/// Each finding of `envelope` as `kind title`.
fn findings(envelope: &Envelope) -> Vec<String> {
    envelope
        .findings
        .iter()
        .map(|finding| format!("{} {}", finding.kind, finding.title))
        .collect()
}

#[test]
fn a_mutant_that_loops_is_cut_at_max_mutant_timeout() {
    if !installed("pytest", "the mutant-timeout fixture") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo("mutant-timeout/loop", dir.path());
    let outcome = run_cupel(
        &["check", "--only", "tests.mutation", "--format", "json"],
        dir.path(),
    );
    let envelope = outcome.envelope();
    assert_eq!(
        findings(envelope),
        [
            "mutant.survived Mutant survived: `0` → `1` at count.py:2",
            "mutant.timeout Mutant timed out: `+=` → `-=` at count.py:4",
            "mutant.timeout Mutant timed out: `1` → `0` at count.py:4",
        ],
        "{}",
        outcome.stdout
    );
    // Uncapped, each looping mutant would run a hundred thousand times its reference (hours);
    // the cap ends each at thirty seconds, so the check ends well within this guard.
    let ran_ms = envelope.checks[0].duration_ms;
    assert!(ran_ms < 600_000, "{ran_ms} ms");
    outcome.assert_golden("mutant-timeout/loop");
}

#[test]
fn a_slow_test_no_per_test_selection_narrows_is_left_out_of_every_mutant_with_a_finding() {
    if !installed("go", "the slow-test fixture") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo("mutant-timeout/slow", dir.path());
    // The finding holds the slow test's measured duration, which a second run would not repeat.
    let outcome = run_cupel_measuring(
        &["check", "--only", "tests.mutation", "--format", "json"],
        dir.path(),
    );
    let envelope = outcome.envelope();
    let excluded: Vec<_> = envelope
        .findings
        .iter()
        .filter(|finding| finding.kind == "mutant.slow-test-excluded")
        .collect();
    assert_eq!(excluded.len(), 1, "{}", outcome.stdout);
    let finding = excluded[0];
    let evidence = &finding.evidence.0;
    // `go test -json` names no test's file.
    assert_eq!(
        (
            finding.severity,
            evidence["test"].as_str(),
            evidence["file"].as_str()
        ),
        (
            Severity::Info,
            Some("example.com/slow/TestSlowIntegration"),
            None
        )
    );
    let baseline_ms = evidence["baseline_ms"].as_u64().unwrap();
    assert!(baseline_ms > 60_000, "{baseline_ms}");
    // `Half` has no test but the slow one, so its mutants survive every run that leaves it out:
    // the tests each ran against are the fast test alone.
    let survivors: Vec<(&str, &serde_json::Value)> = envelope
        .findings
        .iter()
        .filter(|finding| finding.kind == "mutant.survived")
        .map(|finding| {
            let symbol = finding.symbol.as_ref().map_or("", |s| s.display.as_str());
            (symbol, &finding.evidence.0["covering_tests"])
        })
        .collect();
    assert!(!survivors.is_empty(), "{}", outcome.stdout);
    for (symbol, tests) in survivors {
        assert_eq!(
            (symbol, tests),
            (
                "slow.Half",
                &serde_json::json!(["example.com/slow/TestDouble"])
            )
        );
    }
}
