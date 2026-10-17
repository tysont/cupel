// ABOUTME: Recognizes a SARIF log in stdout and validates it against the vendored OASIS SARIF 2.1.0 JSON
// ABOUTME: schema (draft 4, formats checked), so `--format sarif` tests prove their output is SARIF.
use std::sync::OnceLock;

use jsonschema::Validator;
use serde_json::Value;

/// `schemas/sarif-schema-2.1.0.json`, vendored unmodified from
/// <https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json>
/// (SARIF Version 2.1.0 Errata 01, an OASIS Standard; © OASIS Open, under the OASIS IPR Policy).
const SCHEMA: &str = include_str!("../schemas/sarif-schema-2.1.0.json");

fn validator() -> &'static Validator {
    static VALIDATOR: OnceLock<Validator> = OnceLock::new();
    VALIDATOR.get_or_init(|| {
        let schema: Value = serde_json::from_str(SCHEMA).expect("the vendored schema is JSON");
        jsonschema::draft4::options()
            .should_validate_formats(true)
            .build(&schema)
            .expect("the vendored schema compiles")
    })
}

/// `text` as a SARIF log: a JSON object with a `version` and a `runs` array, as
/// `--format sarif` prints and no envelope is; `None` for anything else.
pub fn sarif_log(text: &str) -> Option<Value> {
    serde_json::from_str::<Value>(text)
        .ok()
        .filter(|value| value["version"].is_string() && value["runs"].is_array())
}

/// Every way `log` breaks the SARIF 2.1.0 schema, as `<JSON pointer>: <message>`, in pointer
/// order; empty when it is a valid SARIF log.
pub fn sarif_problems(log: &Value) -> Vec<String> {
    let mut problems: Vec<String> = validator()
        .iter_errors(log)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect();
    problems.sort();
    problems
}
