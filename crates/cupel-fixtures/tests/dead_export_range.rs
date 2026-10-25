// ABOUTME: `static.dead-code` under a range end to end (ADR-144): a commit edits an exported function that
// ABOUTME: only an unchanged file imports, and the real binary's `--base` run reports it neither dead nor unimported.
use std::path::Path;
use std::process::Command;

use cupel_fixtures::{copy_repo, run_cupel};

/// Runs each of `commands` as git in `dir`, as a fixed author.
fn git(dir: &Path, commands: &[&[&str]]) {
    for args in commands {
        let output = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(*args)
            .current_dir(dir)
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// `lang`'s history: `main` holds the base tree, branch `work` one commit that edits the
/// exported function and nothing else.
fn history(lang: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &[&["init", "--quiet", "--initial-branch", "main"]]);
    copy_repo(&format!("dead-export-range/{lang}/base"), root);
    git(
        root,
        &[&["add", "--all"], &["commit", "--quiet", "-m", "base"]],
    );
    git(root, &[&["checkout", "--quiet", "-b", "work"]]);
    copy_repo(&format!("dead-export-range/{lang}/head"), root);
    git(
        root,
        &[&["commit", "--quiet", "--all", "-m", "edit the price"]],
    );
    dir
}

/// Checks `lang`'s range: the edited function is in scope, and nothing is reported dead.
fn assert_the_imported_export_is_live(lang: &str, display: &str) {
    let repo = history(lang);
    let outcome = run_cupel(
        &[
            "check",
            "--base",
            "main",
            "--only",
            "static.dead-code",
            "--format",
            "json",
        ],
        repo.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let envelope = outcome.envelope();
    let scope = envelope.run.scope.as_ref().expect("a range has a scope");
    assert_eq!(scope.symbols, 1, "{display} alone is in scope");
    let findings: Vec<(&str, &str)> = envelope
        .findings
        .iter()
        .map(|finding| {
            let symbol = finding.symbol.as_ref().map_or("", |s| s.display.as_str());
            (finding.kind.as_str(), symbol)
        })
        .collect();
    assert_eq!(findings, [], "{display} is imported by an unchanged file");
    outcome.assert_golden(&format!("dead-export-range/{lang}"));
}

#[test]
fn a_python_export_only_an_unchanged_file_imports_is_not_dead_under_a_range() {
    assert_the_imported_export_is_live("python", "shop.prices.price");
}

#[test]
fn a_go_export_only_an_unchanged_package_imports_is_not_dead_under_a_range() {
    assert_the_imported_export_is_live("go", "prices.Price");
}
