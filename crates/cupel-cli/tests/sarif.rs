// ABOUTME: `--format sarif` through the binary: every verb's envelope is a valid SARIF 2.1.0 log whose
// ABOUTME: exit code, results, fingerprints, root, revision, and suppressions agree with the JSON envelope.
//
// Validity is checked against the vendored OASIS schema, `crates/cupel-fixtures/schemas/
// sarif-schema-2.1.0.json`, from
// https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json
// (SARIF Version 2.1.0 Errata 01, an OASIS Standard; © OASIS Open, distributed under the OASIS IPR
// Policy, https://www.oasis-open.org/policies-guidelines/ipr/).
mod support;

use cupel_fixtures::sarif_problems;
use serde_json::{Value, json};
use support::{Outcome, Repo, cupel, run_with};

/// The Rust fixture repository, committed, so runs in it have findings and a git revision.
fn committed_fixture() -> Repo {
    let repo = Repo::new();
    cupel_fixtures::copy_repo("rust", &repo.root());
    repo.write(".gitignore", ".cupel/\ntarget/\n");
    assert!(repo.git(&["init", "--quiet", "--initial-branch", "main"]));
    assert!(repo.git(&["add", "--all"]));
    assert!(repo.git(&["commit", "--quiet", "-m", "first"]));
    repo
}

/// Stdout as a SARIF log, after checking it is valid SARIF 2.1.0 whose invocation reports the
/// process's exit code.
fn sarif(outcome: &Outcome) -> Value {
    let log: Value = serde_json::from_str(&outcome.stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{}", outcome.stdout));
    assert_eq!(sarif_problems(&log), Vec::<String>::new(), "{log:#}");
    assert_eq!(
        log["runs"][0]["invocations"][0]["exitCode"],
        json!(outcome.code),
        "{log:#}"
    );
    log
}

fn complexity(repo: &Repo, format: &str) -> Outcome {
    repo.run(&["check", "--only", "static.complexity", "--format", format])
}

#[test]
fn every_verb_renders_a_valid_sarif_log() {
    let repo = committed_fixture();
    let finding = complexity(&repo, "json").envelope()["findings"][0]["id"]
        .as_str()
        .expect("the fixture has a complexity finding")
        .to_owned();
    let invocations: [&[&str]; 13] = [
        &["check", "--only", "static.complexity"],
        &["check", "--only", "static.complexity", "--dry-run"],
        &["survey", "--only", "static.complexity"],
        &["describe"],
        &["describe", "static.complexity"],
        &["explain", &finding],
        &["baseline", "show"],
        &["oracle", "list"],
        &["tests", "--impacted", "--changed"],
        &["init"],
        &["doctor"],
        &["suppress", &finding, "--reason", "x", "--dry-run"],
        &["check", "--bogus"],
    ];
    for args in invocations {
        let mut all = args.to_vec();
        all.extend(["--format", "sarif", "--no-install"]);
        let log = sarif(&repo.run(&all));
        assert_eq!(log["version"], json!("2.1.0"), "{args:?}");
    }
}

#[test]
fn a_check_log_holds_the_envelope_findings_as_results() {
    let repo = committed_fixture();
    let envelope = complexity(&repo, "json").envelope();
    let outcome = complexity(&repo, "sarif");
    let log = sarif(&outcome);
    let run = &log["runs"][0];

    let ids = |values: &Value, pointer: &str| -> Vec<Value> {
        values
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.pointer(pointer).cloned().unwrap_or(Value::Null))
            .collect()
    };
    assert!(!envelope["findings"].as_array().unwrap().is_empty());
    assert_eq!(
        ids(&run["results"], "/partialFingerprints/cupel~1findingId"),
        ids(&envelope["findings"], "/id")
    );
    assert_eq!(
        ids(&run["results"], "/ruleId"),
        ids(&envelope["findings"], "/kind")
    );
    assert_eq!(
        ids(&run["properties"]["checks"], "/id"),
        ids(&envelope["checks"], "/id")
    );
    let root = format!("file://{}/", repo.root().display());
    assert_eq!(run["originalUriBaseIds"]["SRCROOT"]["uri"], json!(root));
    let revision = &run["versionControlProvenance"][0];
    assert_eq!(revision["repositoryUri"], json!(root));
    assert_eq!(revision["revisionId"], envelope["run"]["git"]["sha"]);
    assert_eq!(revision["branch"], json!("main"));
}

#[test]
fn a_suppressed_finding_is_an_accepted_suppressed_result() {
    let repo = committed_fixture();
    let id = complexity(&repo, "json").envelope()["findings"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        repo.run(&["suppress", &id, "--reason", "measured later"])
            .code,
        0
    );

    let log = sarif(&complexity(&repo, "sarif"));

    let results = log["runs"][0]["results"].as_array().unwrap();
    let suppressed: Vec<&Value> = results
        .iter()
        .filter(|result| result["suppressions"] != json!([]))
        .collect();
    assert_eq!(suppressed.len(), 1, "{results:#?}");
    assert_eq!(
        suppressed[0]["partialFingerprints"]["cupel/findingId"],
        json!(id)
    );
    assert_eq!(
        suppressed[0]["suppressions"],
        json!([{"kind": "external", "status": "accepted"}])
    );
    assert_eq!(suppressed[0]["level"], json!("note"));
}

#[test]
fn cupel_format_selects_sarif_too() {
    let outcome = run_with(cupel().env("CUPEL_FORMAT", "sarif"), ["describe"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stderr);
    let log = sarif(&outcome);
    assert_eq!(
        log["runs"][0]["properties"]["description"]["subject"],
        json!("cli")
    );
}
