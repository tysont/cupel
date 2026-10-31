// ABOUTME: `tests.flaky` on a Rust project whose runner is nextest, end to end (ADR-158): the baseline runs, the
// ABOUTME: flake engine does not rerun it, and the check is skipped `unsupported`, naming the fact and nextest.
use std::process::Command;

use cupel_core::{CheckStatus, FactKind, SkipReason};
use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn tests_flaky_on_nextest_is_skipped_naming_the_runner_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("flaky-nextest/rust", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &["check", "--only", "tests.flaky", "--format", "json"],
        dir.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let result = &outcome.envelope().checks[0];
    let skipped = result.skipped.as_ref().expect("the check is skipped");
    assert_eq!(
        (
            result.status,
            skipped.reason,
            skipped.fact,
            skipped.tool.as_ref().map(ToString::to_string),
            result.notes.clone(),
        ),
        (
            CheckStatus::Skipped,
            SkipReason::Unsupported,
            Some(FactKind::TestReruns),
            Some("cargo-nextest".to_owned()),
            vec![
                "engine.flake does not rerun tool.cargo-nextest: it runs each test in its own \
                 process, so the order of the tests cannot change an outcome (rust)"
                    .to_owned()
            ],
        )
    );
    outcome.assert_golden("flaky-nextest/rust");
}
