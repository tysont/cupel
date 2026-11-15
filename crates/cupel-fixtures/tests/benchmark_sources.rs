// ABOUTME: Benchmark sources end to end: clones among JMH benchmarks, in a module of their own or Gradle's `jmh`
// ABOUTME: source set, are info test-code clones that leave static.clones' pairs at 0 (ADR-227, jackson-core #1775).
use std::process::Command;

use cupel_core::Severity;
use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn clones_among_benchmarks_are_test_code_and_count_in_no_pair() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("benchmark-sources/java", dir.path());
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git runs");
    assert!(init.success());
    let outcome = run_cupel(
        &["check", "--only", "static.clones", "--format", "json"],
        dir.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let envelope = outcome.envelope();
    let found: Vec<(&str, Severity, &serde_json::Value)> = envelope
        .findings
        .iter()
        .map(|finding| {
            (
                finding.location.as_ref().map_or("", |at| at.file.as_str()),
                finding.severity,
                &finding.evidence.0["shape"],
            )
        })
        .collect();
    let test_code = serde_json::json!("test-code");
    assert_eq!(
        found,
        [
            (
                "jmh/src/main/java/codec/jmh/EscapeBench.java",
                Severity::Info,
                &test_code
            ),
            (
                "src/jmh/java/codec/EscapeInputs.java",
                Severity::Info,
                &test_code
            ),
        ]
    );
    let pairs = envelope
        .checks
        .iter()
        .find(|check| check.id.as_str() == "static.clones")
        .and_then(|check| check.metrics.get("pairs").copied());
    assert_eq!(pairs, Some(0.0));
}
