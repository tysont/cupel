// ABOUTME: The SARIF 2.1.0 schema check the `--format sarif` tests rely on: a valid log has no problems,
// ABOUTME: and an invalid one is reported at the JSON pointer of each violation, formats included.
//
// The schema is `schemas/sarif-schema-2.1.0.json`, vendored unmodified from
// https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json
// (SARIF Version 2.1.0 Errata 01, an OASIS Standard). Copyright © OASIS Open; it is distributed
// under the OASIS IPR Policy (https://www.oasis-open.org/policies-guidelines/ipr/), which permits
// copying the specification's schemas in full.
use cupel_fixtures::sarif_problems;
use serde_json::{Value, json};

fn minimal() -> Value {
    json!({
        "version": "2.1.0",
        "runs": [{
            "tool": {"driver": {"name": "cupel"}},
            "results": [{"ruleId": "complexity.high", "level": "error", "message": {"text": "Too complex"}}]
        }]
    })
}

#[test]
fn a_minimal_log_has_no_problems() {
    assert_eq!(sarif_problems(&minimal()), Vec::<String>::new());
}

#[test]
fn each_violation_is_reported_at_its_pointer() {
    let mut log = minimal();
    log["runs"][0]["results"][0]["level"] = json!("fatal");
    log["runs"][0]["tool"]["driver"]
        .as_object_mut()
        .unwrap()
        .remove("name");

    let problems = sarif_problems(&log);

    let pointers: Vec<&str> = problems
        .iter()
        .map(|problem| problem.split(": ").next().unwrap())
        .collect();
    assert_eq!(
        pointers,
        ["/runs/0/results/0/level", "/runs/0/tool/driver"],
        "{problems:#?}"
    );
}

#[test]
fn formats_are_validated() {
    let mut log = minimal();
    log["runs"][0]["invocations"] =
        json!([{"executionSuccessful": true, "startTimeUtc": "yesterday"}]);

    let problems = sarif_problems(&log);

    assert_eq!(problems.len(), 1, "{problems:#?}");
    assert!(
        problems[0].starts_with("/runs/0/invocations/0/startTimeUtc: "),
        "{problems:#?}"
    );
}
