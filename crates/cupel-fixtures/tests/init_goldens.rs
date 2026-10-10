// ABOUTME: Golden envelopes for `cupel init --dry-run` on every detection fixture: the planned
// ABOUTME: `cupel.toml` and `.cupel/.gitignore` diffs, run in a scratch copy outside any git repository.
use cupel_core::Status;
use cupel_fixtures::{copy_repo, run_cupel};

const CASES: [&str; 10] = [
    "cargo-single",
    "cargo-workspace",
    "go-module",
    "npm-js",
    "pnpm-ts-workspace",
    "python-pyproject",
    "python-requirements",
    "monorepo",
    "maven",
    "empty",
];

#[test]
fn init_dry_run_matches_its_golden_for_every_detect_fixture() {
    for case in CASES {
        let dir = tempfile::tempdir().unwrap();
        copy_repo(&format!("detect/{case}"), dir.path());

        let outcome = run_cupel(&["init", "--dry-run", "--format", "json"], dir.path());

        assert_eq!(outcome.code, Some(0), "{case}: {}", outcome.stdout);
        assert_eq!(outcome.envelope().status, Status::Plan, "{case}");
        outcome.assert_golden(&format!("init/{case}"));
    }
}
