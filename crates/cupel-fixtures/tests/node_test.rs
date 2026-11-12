// ABOUTME: A configured root tested with `node --test` beside a repository root that depends on Vitest, end to end:
// ABOUTME: `tests.coverage` reads c8's committed report and `tests.flaky` reruns the tests `tool.node-test` reports.
use std::process::Command;

use cupel_core::CheckStatus;
use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_node_test_root_gets_coverage_and_test_results() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("node-test", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &[
            "check",
            "--only",
            "tests.coverage",
            "--only",
            "tests.flaky",
            "--format",
            "json",
        ],
        dir.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let envelope = outcome.envelope();
    let checks: Vec<(&str, CheckStatus, Option<u64>)> = envelope
        .checks
        .iter()
        .map(|check| {
            (
                check.id.as_str(),
                check.status,
                check.score.as_ref().map(|score| score.n),
            )
        })
        .collect();
    assert_eq!(
        checks,
        [
            ("tests.coverage", CheckStatus::Fail, Some(7)),
            ("tests.flaky", CheckStatus::Pass, Some(2)),
        ],
        "c8's report under .cupel/cache/coverage/ must be fresh (see the fixture's cupel.toml), \
         and node --test must name each test's file (Node 24.11.1 and later)"
    );
    let found: Vec<(&str, Option<&str>)> = envelope
        .findings
        .iter()
        .map(|finding| {
            let symbol = finding.symbol.as_ref();
            (
                finding.kind.as_str(),
                symbol.map(|symbol| symbol.display.as_str()),
            )
        })
        .collect();
    assert_eq!(
        found,
        [("coverage.uncovered-symbol", Some("quote.joinCommand"))]
    );
    outcome.assert_golden("node-test");
}
