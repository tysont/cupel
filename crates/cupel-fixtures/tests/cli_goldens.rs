// ABOUTME: End-to-end golden tests against the real `cupel` binary: `describe`, `describe check`,
// ABOUTME: and a usage-error envelope, plus how `run_cupel` captures output that is not JSON.
use cupel_core::{ExitCode, Status};
use cupel_fixtures::run_cupel;

#[test]
fn describe_matches_its_golden() {
    let dir = tempfile::tempdir().unwrap();

    let outcome = run_cupel(&["describe", "--format", "json"], dir.path());

    assert_eq!(outcome.code, Some(0), "{}", outcome.stderr);
    outcome.assert_golden("cli/describe");
}

#[test]
fn describe_check_matches_its_golden() {
    let dir = tempfile::tempdir().unwrap();

    let outcome = run_cupel(&["describe", "check", "--format", "json"], dir.path());

    assert_eq!(outcome.code, Some(0), "{}", outcome.stderr);
    outcome.assert_golden("cli/describe-check");
}

#[test]
fn an_unknown_verb_matches_its_error_golden() {
    let dir = tempfile::tempdir().unwrap();

    let outcome = run_cupel(&["frobnicate", "--format", "json"], dir.path());

    assert_eq!(outcome.code, Some(2));
    let envelope = outcome.envelope();
    assert_eq!(envelope.status, Status::Error);
    assert_eq!(envelope.exit_code, ExitCode::Usage);
    outcome.assert_golden("cli/unknown-verb");
}

#[test]
fn stderr_is_captured_separately_from_the_envelope() {
    let dir = tempfile::tempdir().unwrap();

    let outcome = run_cupel(&["frobnicate", "--format", "json"], dir.path());

    assert!(
        outcome.stderr.contains("unrecognized subcommand"),
        "stderr: {}",
        outcome.stderr
    );
    assert!(outcome.envelope.is_some());
}

#[test]
fn text_output_is_captured_without_an_envelope() {
    let dir = tempfile::tempdir().unwrap();

    let outcome = run_cupel(&["describe", "--format", "text"], dir.path());

    assert_eq!(outcome.code, Some(0), "{}", outcome.stderr);
    assert!(outcome.envelope.is_none(), "{}", outcome.stdout);
    assert!(outcome.stdout.contains("describe"), "{}", outcome.stdout);
}
