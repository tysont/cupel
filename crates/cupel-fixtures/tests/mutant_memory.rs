// ABOUTME: A mutant's run is bounded in memory, end to end (ADR-148): the real binary kills two mutants that loop
// ABOUTME: while allocating once they hold more than the slot's share of `max_mutant_memory`, and reports each.
use cupel_core::Envelope;
use cupel_fixtures::{copy_repo, run_cupel};

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
fn a_mutant_that_loops_while_allocating_is_killed_over_its_memory_budget() {
    if !installed("pytest", "the mutant-memory fixture") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo("mutant-memory/grow", dir.path());
    let outcome = run_cupel(
        &["check", "--only", "tests.mutation", "--format", "json"],
        dir.path(),
    );
    let envelope = outcome.envelope();
    assert_eq!(
        findings(envelope),
        [
            "mutant.out-of-memory Mutant ran out of memory: `+=` → `-=` at grow.py:10",
            "mutant.out-of-memory Mutant ran out of memory: `1` → `0` at grow.py:10",
        ],
        "{}",
        outcome.stdout
    );
    // Each looping mutant holds four more MiB every ten milliseconds and is killed at 128 MiB,
    // well before the ten-second cap; the check ends well within this guard.
    let ran_ms = envelope.checks[0].duration_ms;
    assert!(ran_ms < 120_000, "{ran_ms} ms");
    assert_eq!(envelope.checks[0].metrics["out_of_memory"].to_string(), "2");
    outcome.assert_golden("mutant-memory/grow");
}
