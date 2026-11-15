// ABOUTME: A Rust bench that needs nightly end to end: clippy on stable rejects its `#![feature(test)]` with E0554,
// ABOUTME: which static.analyze reports as info, so the check passes (Q-rs-5, ADR-232).
use std::process::Command;

use cupel_core::{Severity, Status};
use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn a_feature_gate_stable_rejects_is_an_info_finding_and_the_check_passes() {
    let clippy = Command::new("cargo").args(["clippy", "--version"]).output();
    if !clippy.is_ok_and(|output| output.status.success()) {
        eprintln!("SKIPPED: cargo clippy not installed; static.analyze was not run");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    copy_repo("nightly-benches/rust", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &["check", "--only", "static.analyze", "--format", "json"],
        dir.path(),
    );
    let envelope = outcome.envelope();
    let findings: Vec<(&str, Severity, &str)> = envelope
        .findings
        .iter()
        .map(|finding| {
            let file = finding.location.as_ref().map_or("", |l| l.file.as_str());
            (finding.kind.as_str(), finding.severity, file)
        })
        .collect();
    assert_eq!(
        findings,
        [("analyze.clippy.E0554", Severity::Info, "benches/total.rs")]
    );
    assert_eq!(envelope.status, Status::Pass, "{}", outcome.stdout);
}
