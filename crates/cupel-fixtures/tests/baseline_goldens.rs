// ABOUTME: Golden envelope for `cupel baseline diff`: a baseline `doctor` run, a change to what the
// ABOUTME: repository needs, a second run, and a diff showing exactly the new and resolved findings.
use std::fs;

use cupel_core::{CheckId, FindingId};
use cupel_fixtures::run_cupel;

fn runner_finding(tool: &str) -> String {
    FindingId::for_subject(&CheckId::from("doctor"), "doctor.tool-missing", tool).to_string()
}

#[test]
fn diff_after_changing_a_needed_tool_shows_one_new_and_one_resolved_finding() {
    let dir = tempfile::tempdir().unwrap();
    let configure = |runner: &str| {
        fs::write(
            dir.path().join("cupel.toml"),
            format!("[lang.rust]\ntest = \"{runner} --all\"\n"),
        )
        .unwrap();
    };
    configure("cupel-test-missing-a");
    let first = run_cupel(&["doctor", "--format", "json"], dir.path());
    assert_eq!(first.code, Some(0), "{}", first.stdout);
    let set = run_cupel(
        &["baseline", "set", "--write", "--format", "json"],
        dir.path(),
    );
    assert_eq!(set.code, Some(0), "{}", set.stdout);
    configure("cupel-test-missing-b");
    let second = run_cupel(&["doctor", "--format", "json"], dir.path());
    assert_eq!(second.code, Some(0), "{}", second.stdout);

    let diff = run_cupel(&["baseline", "diff", "--format", "json"], dir.path());

    assert_eq!(diff.code, Some(0), "{}", diff.stdout);
    let envelope = diff.envelope();
    let findings: Vec<(String, bool)> = envelope
        .findings
        .iter()
        .map(|finding| (finding.id.to_string(), finding.new_since_baseline))
        .collect();
    assert_eq!(
        findings,
        [
            (runner_finding("cupel-test-missing-b"), true),
            (runner_finding("cupel-test-missing-a"), false),
        ]
    );
    assert_eq!((envelope.summary.new, envelope.summary.resolved), (1, 1));
    assert_eq!(
        envelope.run.baseline.as_ref().map(|baseline| baseline.id),
        Some(first.envelope().run.id)
    );
    diff.assert_golden("baseline/diff");
}
