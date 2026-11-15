// ABOUTME: Rust method assertions end to end: `.debug_assert()` and `.assert_named(…)` assert, and debug
// ABOUTME: assertions count unless `[checks."tests.quality"] debug_assertions = false` (ADR-224).
use std::fs;
use std::path::Path;
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

/// The `tests.quality` findings over a git-initialized copy of the fixture with `config` as its
/// `cupel.toml`, as (kind, test).
fn quality(dir: &Path, config: Option<&str>) -> Vec<(String, String)> {
    copy_repo("method-assertions/rust", dir);
    if let Some(config) = config {
        fs::write(dir.join("cupel.toml"), config).unwrap();
    }
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir)
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &["check", "--only", "tests.quality", "--format", "json"],
        dir,
    );
    outcome
        .envelope()
        .findings
        .iter()
        .map(|finding| {
            let symbol = finding.symbol.as_ref().map_or("", |s| s.display.as_str());
            (finding.kind.clone(), symbol.to_owned())
        })
        .collect()
}

fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(kind, symbol)| ((*kind).to_owned(), (*symbol).to_owned()))
        .collect()
}

#[test]
fn methods_named_assert_and_debug_assertions_count_by_default() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        quality(dir.path(), None),
        pairs(&[
            (
                "test.weak-assertion",
                "tests::commands::the_name_is_not_empty"
            ),
            ("test.no-assertion", "tests::commands::the_command_builds"),
        ])
    );
}

#[test]
fn debug_assertions_false_leaves_debug_assertions_out() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        quality(
            dir.path(),
            Some("[checks.\"tests.quality\".lang.rust]\ndebug_assertions = false\n")
        ),
        pairs(&[
            (
                "test.no-assertion",
                "tests::commands::the_command_is_configured"
            ),
            (
                "test.no-assertion",
                "tests::commands::the_name_is_not_empty"
            ),
            ("test.no-assertion", "tests::commands::the_command_builds"),
        ])
    );
}
