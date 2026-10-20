// ABOUTME: Symbol matching across refs end to end (ADR-131): a fixture commit renames one function and splits
// ABOUTME: another, and the real binary's `--changed --against` run shows no spurious new or resolved findings.
use std::path::Path;
use std::process::Command;

use cupel_core::{CheckId, Envelope, SetDelta};
use cupel_fixtures::{copy_repo, run_cupel};

/// Runs git in `dir` as a fixed author.
fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The fixture's history: `main` holds the base commit, branch `work` the head commit; the base
/// commit was checked, so the store holds a run made there.
fn history() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "--quiet", "--initial-branch", "main"]);
    copy_repo("symbol-matching/base", root);
    git(root, &["add", "--all"]);
    git(root, &["commit", "--quiet", "-m", "billing"]);
    let base = run_cupel(
        &["check", "--only", "static.complexity", "--format", "json"],
        root,
    );
    assert_eq!(base.code, Some(0), "{}", base.stderr);
    git(root, &["checkout", "--quiet", "-b", "work"]);
    copy_repo("symbol-matching/head", root);
    git(root, &["add", "src", "tests"]);
    git(
        root,
        &["commit", "--quiet", "-m", "rename settle, split reconcile"],
    );
    dir
}

/// The sets of `check`'s delta, each as (base displays, head displays).
fn sets(envelope: &Envelope, check: &str) -> Vec<(Vec<String>, Vec<String>)> {
    let displays = |set: &SetDelta, head: bool| {
        let side = if head { &set.head } else { &set.base };
        side.iter().map(|symbol| symbol.display.clone()).collect()
    };
    delta_sets(envelope, check)
        .iter()
        .map(|set| (displays(set, false), displays(set, true)))
        .collect()
}

fn delta_sets<'a>(envelope: &'a Envelope, check: &str) -> &'a [SetDelta] {
    envelope
        .checks
        .iter()
        .find(|result| result.id == CheckId::from(check))
        .and_then(|result| result.delta.as_ref())
        .map_or(&[], |delta| delta.sets.as_slice())
}

#[test]
fn a_renamed_and_a_split_function_carry_their_findings_and_compare_their_metrics_as_sets() {
    let repo = history();
    let outcome = run_cupel(
        &[
            "check",
            "--changed",
            "--against",
            "main",
            "--only",
            "static.complexity",
            "--only",
            "tests.weakening",
            "--format",
            "json",
        ],
        repo.path(),
    );
    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    let envelope = outcome.envelope();

    assert_eq!((envelope.summary.new, envelope.summary.resolved), (0, 0));
    let findings: Vec<(&str, &str, bool)> = envelope
        .findings
        .iter()
        .map(|finding| {
            let symbol = finding.symbol.as_ref().map_or("", |s| s.display.as_str());
            (finding.kind.as_str(), symbol, finding.new_since_baseline)
        })
        .collect();
    assert_eq!(
        findings,
        [
            ("complexity.high", "billing.settle_invoices", false),
            ("complexity.high", "billing.reconcile", false),
        ]
    );
    let owned =
        |names: &[&str]| -> Vec<String> { names.iter().map(|name| (*name).to_owned()).collect() };
    let test = "tests.test_billing.test_";
    assert_eq!(
        sets(envelope, "static.complexity"),
        [
            (
                owned(&["billing.settle"]),
                owned(&["billing.settle_invoices"])
            ),
            (
                owned(&["billing.reconcile"]),
                owned(&["billing.match_payment", "billing.reconcile"])
            ),
            (
                owned(&["tests.test_billing.Invoice.__init__"]),
                owned(&["tests.test_billing.Invoice.__init__"])
            ),
            (
                vec![format!(
                    "{test}settle_skips_paid_invoices_and_applies_credits"
                )],
                vec![format!(
                    "{test}settled_totals_skip_paid_invoices_and_apply_credits"
                )]
            ),
        ]
    );
    // The split: reconcile's cyclomatic complexity of 15 against 11 + 5 for its two parts.
    let split = &delta_sets(envelope, "static.complexity")[1];
    let cyclomatic = split.metrics["cyclomatic"];
    assert_eq!((cyclomatic.base, cyclomatic.head), (15.0, 16.0));
    assert_eq!(split.findings.base.len(), 1);
    // The renamed test is compared with its predecessor, not reported deleted.
    let weakening = envelope
        .checks
        .iter()
        .find(|result| result.id == CheckId::from("tests.weakening"))
        .unwrap();
    assert_eq!(
        (weakening.findings, weakening.score.as_ref().map(|s| s.n)),
        (0, Some(1))
    );
    outcome.assert_golden("symbol-matching/changed");
}
