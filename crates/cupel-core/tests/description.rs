// ABOUTME: Tests for the `describe` payload carried in the envelope's `description` field (ADR-018).
// ABOUTME: Covers every subject variant, nullable `run.verb`, exit-code meanings, and the schema list.
use cupel_core::{Description, Envelope, ExitCode, schema};
use serde_json::{Value, json};

fn envelope_with(verb: &Value, description: &Value) -> Value {
    json!({
        "cupel": {"version": "0.1.0", "schema": "https://tysont.github.io/cupel/schema/v1/run.json"},
        "run": {
            "id": "run_01JA2B3C4D5E6F7G8H9JKMNPQR", "verb": verb,
            "started": "2026-10-10T16:31:04Z", "duration_ms": 3,
            "git": null, "scope": null, "depth": null, "budget_ms": null, "seed": 0,
            "toolchain": {}, "config": {"effective": {}, "sources": []}, "baseline": null
        },
        "status": "pass", "exit_code": 0, "checks": [], "findings": [],
        "summary": {"findings": {"fail": 0, "warn": 0, "info": 0},
                    "new": 0, "resolved": 0, "suppressed": 0, "gate": null},
        "next_actions": [],
        "description": description, "plan": null, "explanation": null, "rankings": null, "tests": null,
        "oracles": null,
        "truncated": false, "total_findings": 0
    })
}

fn flag() -> Value {
    json!({
        "name": "--format", "positional": false, "value": "enum", "value_name": "FORMAT",
        "values": ["json", "jsonl", "text", "sarif"], "default": null, "env": "CUPEL_FORMAT",
        "required": false, "repeated": false, "conflicts": [],
        "summary": "Output format.", "stability": "experimental", "pending_step": null
    })
}

fn command() -> Value {
    json!({
        "name": "check", "summary": "Run checks.", "stability": "experimental",
        "pending_step": "P2.1", "arguments": [flag()], "subcommands": [],
        "examples": [{"command": "cupel check --changed", "summary": "Check changes.", "exit_code": 2}]
    })
}

fn cli() -> Value {
    json!({
        "subject": "cli",
        "name": "cupel", "version": "0.1.0", "summary": "Grades code.", "stability": "experimental",
        "verbs": [{"name": "check", "summary": "Run checks.", "stability": "experimental", "pending_step": "P2.1"}],
        "global_flags": [flag()], "flags": [],
        "exit_codes": [{"code": 2, "name": "usage", "meaning": "Bad flags or config; the envelope names the fix.", "stability": "stable"}],
        "schemas": [{"name": "run", "url": "https://tysont.github.io/cupel/schema/v1/run.json", "stability": "experimental"}],
        "config_precedence": ["flags", "env", "profile", "cupel.toml", "defaults"],
        "install_routes": [{"name": "npm", "command": "npx cupel", "platforms": ["linux", "macos"], "live": false,
                            "pending": "once the `cupel` package is published to npm", "stability": "experimental"}]
    })
}

fn schema_doc() -> Value {
    json!({"name": "run", "url": "https://tysont.github.io/cupel/schema/v1/run.json", "stability": "experimental",
           "schema": {"type": "object"}})
}

/// Asserts `example` deserializes into an envelope and serializes back unchanged.
fn assert_round_trip(example: &Value) -> Value {
    let typed: Envelope = serde_json::from_value(example.clone()).expect("example deserializes");
    let output = serde_json::to_value(&typed).unwrap();
    assert_eq!(&output, example);
    output
}

#[test]
fn every_description_subject_round_trips_inside_an_envelope() {
    let mut command_subject = command();
    command_subject["subject"] = json!("command");
    let mut schema_subject = schema_doc();
    schema_subject["subject"] = json!("schema");
    let mut cli_body = cli();
    cli_body.as_object_mut().unwrap().remove("subject");
    let all = json!({"subject": "all", "cli": cli_body, "commands": [command()], "schemas": [schema_doc()]});
    let agent = json!({"subject": "agent", "markdown": "# Cupel\n"});
    for description in [cli(), command_subject, schema_subject, all, agent] {
        assert_round_trip(&envelope_with(&json!("describe"), &description));
    }
}

#[test]
fn description_subject_is_the_tag() {
    let typed: Description = serde_json::from_value(cli()).unwrap();
    assert_eq!(serde_json::to_value(&typed).unwrap(), cli());
    assert!(matches!(typed, Description::Cli(_)));
    let error = serde_json::from_value::<Description>(json!({"subject": "galaxy"}))
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "unknown variant `galaxy`, expected one of `cli`, `command`, `schema`, `all`, \
         `language`, `languages`, `fact`, `matrix`, `check`, `finding`, `agent`"
    );
}

#[test]
fn envelopes_without_a_verb_or_description_use_null() {
    let output = assert_round_trip(&envelope_with(&Value::Null, &Value::Null));
    assert_eq!(output["run"]["verb"], Value::Null);
    assert_eq!(output["description"], Value::Null);
}

#[test]
fn exit_code_meanings_follow_the_design_table() {
    let meanings: Vec<&str> = ExitCode::ALL.iter().map(|code| code.meaning()).collect();
    assert_eq!(
        meanings,
        [
            "Ran fully; gate passed or no gate.",
            "Ran fully; gate failed.",
            "Bad flags or config; the envelope names the fix.",
            "Missing tool, oracle, or toolchain; doctor findings attached.",
            "Baseline test suite fails or build broken; scoring refused.",
            "Timeout or budget hit; the envelope has valid partial results.",
            "Cupel bug; the envelope includes a bug-report payload.",
        ]
    );
}

#[test]
fn published_schemas_are_named_by_their_url() {
    let names: Vec<&str> = schema::PUBLISHED.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, ["run", "finding", "events", "config", "plugin"]);
    for (name, build) in schema::PUBLISHED {
        assert_eq!(
            schema::url(name),
            format!("{}{name}.json", schema::BASE_URL)
        );
        assert_eq!(build().as_value()["$id"], json!(schema::url(name)));
    }
}

#[test]
fn run_schema_requires_the_nullable_verb_and_description() {
    let run = schema::run();
    let value = run.as_value();
    let required =
        |def: &Value| -> Vec<String> { serde_json::from_value(def["required"].clone()).unwrap() };
    assert_eq!(
        required(value),
        [
            "checks",
            "cupel",
            "description",
            "exit_code",
            "explanation",
            "findings",
            "next_actions",
            "oracles",
            "plan",
            "rankings",
            "run",
            "status",
            "summary",
            "tests",
            "total_findings",
            "truncated"
        ]
    );
    assert_eq!(
        required(&value["$defs"]["RunInfo"]),
        [
            "baseline",
            "budget_ms",
            "config",
            "depth",
            "duration_ms",
            "git",
            "id",
            "scope",
            "seed",
            "started",
            "toolchain",
            "verb"
        ]
    );
}
