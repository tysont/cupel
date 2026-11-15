// ABOUTME: A Rust function only its doctest calls end to end: coverage without doctests reports it as
// ABOUTME: coverage.doctest-only at info beside an ordinary uncovered function; with nightly doctests it is covered (ADR-231).
use std::process::Command;

use cupel_core::Severity;
use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_function_only_its_doctest_calls_is_doctest_only_without_nightly_doctests() {
    if cupel_core::find_program("cargo-llvm-cov").is_none() {
        eprintln!("SKIPPED: cargo-llvm-cov not installed; the coverage run was not made");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo("doctest-only/rust", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &["check", "--only", "tests.coverage", "--format", "json"],
        dir.path(),
    );
    let envelope = outcome.envelope();
    let findings: Vec<(&str, Severity, &str)> = envelope
        .findings
        .iter()
        .map(|finding| {
            let symbol = finding.symbol.as_ref().map_or("", |s| s.display.as_str());
            (finding.kind.as_str(), finding.severity, symbol)
        })
        .collect();
    let mut expected = vec![("coverage.uncovered-symbol", Severity::Warn, "halve")];
    if !cupel_adapt::coverage::nightly_coverage(dir.path()) {
        expected.insert(0, ("coverage.doctest-only", Severity::Info, "add"));
    }
    assert_eq!(findings, expected, "{}", outcome.stdout);
}
