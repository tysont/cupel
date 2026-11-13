// ABOUTME: `test.logic` and `test.duplicate-setup` end to end over Python tests at the default thresholds: a branch
// ABOUTME: in a test is logic and a loop holding an assertion is not; three tests sharing five setup statements fire.
use std::path::Path;
use std::process::Command;

use cupel_fixtures::{Outcome, copy_repo, run_cupel};

/// `tests.quality` over a git-initialized copy of the fixture, with the default settings.
fn quality(dir: &Path) -> Outcome {
    copy_repo("logic-and-setup/python", dir);
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir)
        .status()
        .expect("git runs");
    assert!(init.success());
    run_cupel(
        &["check", "--only", "tests.quality", "--format", "json"],
        dir,
    )
}

#[test]
fn a_branch_is_logic_and_three_tests_sharing_five_setup_statements_are_duplicated_setup() {
    let dir = tempfile::tempdir().unwrap();
    let outcome = quality(dir.path());
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let envelope = outcome.envelope();
    let found: Vec<(&str, Option<&str>, Option<&str>)> = envelope
        .findings
        .iter()
        .map(|finding| {
            (
                finding.kind.as_str(),
                finding
                    .symbol
                    .as_ref()
                    .map(|symbol| symbol.display.as_str()),
                finding.location.as_ref().map(|at| at.file.as_str()),
            )
        })
        .collect();
    assert_eq!(
        found,
        [
            (
                "test.duplicate-setup",
                Some("tests.test_cart.test_total_sums_every_line"),
                Some("tests/test_cart.py"),
            ),
            (
                "test.logic",
                Some("tests.test_grade.test_grade_picks_the_letter_by_branch"),
                Some("tests/test_grade.py"),
            ),
        ]
    );
    outcome.assert_golden("logic-and-setup/python");
}
