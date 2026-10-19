// ABOUTME: Integration tests for `required_version` outside `doctor`: the verbs that read or grade the
// ABOUTME: repository refuse with exit 3 and doctor's finding when this binary does not satisfy it.
mod support;

use cupel_core::{CheckId, FindingId};
use serde_json::{Value, json};
use support::{Outcome, Repo};

/// This binary's version.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The verbs that refuse, each as a command line that runs in an empty repository.
const REFUSING: [&[&str]; 7] = [
    &["check"],
    &["check", "--dry-run"],
    &["survey"],
    &["tests", "--impacted"],
    &["explain", "static.complexity"],
    &["baseline", "show"],
    &["oracle", "list"],
];

/// A repository whose `cupel.toml` holds only `required_version = "<requirement>"`.
fn requiring(requirement: &str) -> Repo {
    let repo = Repo::new();
    repo.write(
        "cupel.toml",
        &format!("required_version = \"{requirement}\"\n"),
    );
    repo
}

fn version_id(kind: &str) -> String {
    FindingId::for_subject(&CheckId::from("doctor"), kind, "required_version").to_string()
}

/// The envelope's only finding, after asserting the run was refused with `code`.
fn refused(outcome: &Outcome, code: i32) -> Value {
    assert_eq!(outcome.code, code, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("error"));
    assert_eq!(envelope["checks"], json!([]));
    outcome.finding()
}

#[test]
fn an_unsatisfied_required_version_refuses_every_verb_that_reads_the_repository() {
    let repo = requiring(">=99.0");
    let kinds: Vec<(String, Value, i32)> = REFUSING
        .iter()
        .map(|args| {
            let outcome = repo.run(args);
            let kind = outcome.finding()["kind"].clone();
            (args.join(" "), kind, outcome.code)
        })
        .collect();
    let expected: Vec<(String, Value, i32)> = REFUSING
        .iter()
        .map(|args| (args.join(" "), json!("doctor.version-unsatisfied"), 3))
        .collect();
    assert_eq!(kinds, expected);
}

#[test]
fn the_refusal_is_doctors_finding_with_the_required_and_actual_version() {
    let finding = refused(&requiring(">=99.0").run(&["check"]), 3);
    assert_eq!(
        finding["id"],
        json!(version_id("doctor.version-unsatisfied"))
    );
    assert_eq!(finding["check"], json!("doctor"));
    assert_eq!(finding["severity"], json!("fail"));
    assert_eq!(
        finding["evidence"],
        json!({"required": ">=99.0", "actual": VERSION})
    );
    assert_eq!(finding["action"]["kind"], json!("install_tool"));
    assert_eq!(
        finding["action"]["summary"],
        json!("Install a cupel matching `>=99.0`, or change required_version.")
    );
}

#[test]
fn doctor_reports_the_actual_version_too() {
    let outcome = requiring(">=99.0").run(&["doctor"]);
    assert_eq!(outcome.code, 3, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    let finding = support::finding(&envelope, &version_id("doctor.version-unsatisfied"));
    assert_eq!(
        finding["evidence"],
        json!({"required": ">=99.0", "actual": VERSION})
    );
}

#[test]
fn a_satisfied_required_version_lets_every_verb_run() {
    let repo = requiring(&format!("={VERSION}"));
    let refusals: Vec<(String, Vec<String>)> = REFUSING
        .iter()
        .map(|args| {
            let envelope = repo.run(args).envelope();
            let kinds = support::finding_fields(&envelope, "kind")
                .into_iter()
                .filter(|kind| kind.starts_with("doctor.version"))
                .collect();
            (args.join(" "), kinds)
        })
        .collect();
    let expected: Vec<(String, Vec<String>)> = REFUSING
        .iter()
        .map(|args| (args.join(" "), Vec::new()))
        .collect();
    assert_eq!(refusals, expected);
    assert_eq!(repo.run(&["check"]).code, 0);
}

#[test]
fn describe_init_suppress_and_doctor_run_whatever_the_binary_version() {
    let repo = requiring(">=99.0");
    let describe = repo.run(&["describe", "--agent"]);
    assert_eq!(describe.code, 0, "{}", describe.stdout);
    let init = repo.run(&["init"]);
    assert_ne!(
        support::finding_fields(&init.envelope(), "kind"),
        vec!["doctor.version-unsatisfied".to_owned()]
    );
    let doctor = repo.run(&["doctor"]);
    assert_eq!(doctor.code, 3, "doctor reports it: {}", doctor.stdout);
    // `suppress` needs a stored finding, which the doctor run above recorded.
    let suppress = repo.run(&[
        "suppress",
        &version_id("doctor.version-unsatisfied"),
        "--reason",
        "upgrading next sprint",
    ]);
    assert_eq!(suppress.code, 0, "{}", suppress.stdout);
}

#[test]
fn a_suppressed_version_finding_no_longer_refuses() {
    let repo = requiring(">=99.0");
    let id = version_id("doctor.version-unsatisfied");
    assert_eq!(repo.run(&["check"]).code, 3, "the refusal is recorded");
    let suppressed = repo.run(&["suppress", &id, "--reason", "upgrading next sprint"]);
    assert_eq!(suppressed.code, 0, "{}", suppressed.stdout);
    let outcome = repo.run(&["check"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
}

#[test]
fn a_malformed_required_version_is_a_usage_error_naming_the_key() {
    let finding = refused(&requiring("soon").run(&["check"]), 2);
    assert_eq!(finding["id"], json!(version_id("doctor.version-invalid")));
    assert_eq!(finding["action"]["kind"], json!("configure"));
}
