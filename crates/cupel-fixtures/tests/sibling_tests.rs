// ABOUTME: A sibling crate's or package's test kills and covers a library's code through a per-test map, end to end
// ABOUTME: (ADR-202): a cargo workspace, a Go module, and a Go workspace whose library has no test of its own.
use cupel_core::Envelope;
use cupel_fixtures::{copy_repo, run_cupel};

/// `tests.mutation` and `tests.coverage` at `deep` on a fresh copy of `sibling-tests/<variant>`; `None` (reported
/// as skipped) without one of `programs`.
fn check(variant: &str, programs: &[&str]) -> Option<Envelope> {
    if let Some(missing) = programs
        .iter()
        .find(|program| cupel_core::find_program(program).is_none())
    {
        eprintln!("SKIPPED: {missing} not installed; sibling-tests/{variant} was not run");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo(&format!("sibling-tests/{variant}"), dir.path());
    let outcome = run_cupel(
        &[
            "check",
            "--only",
            "tests.mutation",
            "--only",
            "tests.coverage",
            "--depth",
            "deep",
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

/// Each finding's kind and title, then the mutants killed, survived and uncovered, and the line coverage.
type Scored = (Vec<(String, String)>, [Option<f64>; 4]);

fn scored(envelope: &Envelope) -> Scored {
    let findings = envelope
        .findings
        .iter()
        .map(|finding| (finding.kind.clone(), finding.title.clone()))
        .collect();
    let mutation = |name| metric(envelope, "tests.mutation", name);
    (
        findings,
        [
            mutation("killed"),
            mutation("survived"),
            mutation("uncovered"),
            metric(envelope, "tests.coverage", "line"),
        ],
    )
}

#[test]
fn a_sibling_crates_integration_test_kills_and_covers_the_library_crates_code() {
    let Some(envelope) = check("rust", &["cargo-llvm-cov", "cargo-nextest"]) else {
        return;
    };
    assert_eq!(
        scored(&envelope),
        (vec![], [Some(3.0), Some(0.0), Some(0.0), Some(1.0)]),
        "{envelope:?}"
    );
}

#[test]
fn a_sibling_packages_test_kills_and_covers_the_package_of_its_module() {
    let Some(envelope) = check("go", &["go"]) else {
        return;
    };
    assert_eq!(
        scored(&envelope),
        (vec![], [Some(2.0), Some(0.0), Some(0.0), Some(1.0)]),
        "{envelope:?}"
    );
}

#[test]
fn a_sibling_modules_test_kills_and_covers_the_module_its_go_workspace_uses() {
    let Some(envelope) = check("go-work", &["go"]) else {
        return;
    };
    assert_eq!(
        scored(&envelope),
        (vec![], [Some(2.0), Some(0.0), Some(0.0), Some(1.0)]),
        "{envelope:?}"
    );
}
