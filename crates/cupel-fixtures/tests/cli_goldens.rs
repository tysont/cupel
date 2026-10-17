// ABOUTME: End-to-end golden tests against the real `cupel` binary: `describe`, `describe check`,
// ABOUTME: a usage-error envelope, a SARIF check of a fixture, and how `run_cupel` captures output that is not JSON.
use cupel_core::{ExitCode, Status};
use cupel_fixtures::{copy_repo, run_cupel, sarif_problems};

#[test]
fn a_sarif_check_of_the_rust_fixture_matches_its_golden() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("rust", dir.path());
    let check = ["check", "--only", "static.complexity"];
    let first = run_cupel(&[&check[..], &["--format", "json"]].concat(), dir.path());
    let suppressed = first.envelope().findings[0].id.to_string();
    let suppress = ["suppress", &suppressed, "--reason", "measured later"];
    assert_eq!(run_cupel(&suppress, dir.path()).code, Some(0));

    let outcome = run_cupel(&[&check[..], &["--format", "sarif"]].concat(), dir.path());

    assert_eq!(outcome.code, Some(0), "{}", outcome.stderr);
    assert!(outcome.envelope.is_none(), "a SARIF log is not an envelope");
    assert_eq!(sarif_problems(&outcome.sarif()), Vec::<String>::new());
    outcome.assert_sarif_golden("cli/sarif-check");
}

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
