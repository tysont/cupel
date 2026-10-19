// ABOUTME: Snapshot tests of every JSON Schema cupel-core publishes, plus their identity metadata.
// ABOUTME: A schema change shows up as a snapshot diff that must be reviewed deliberately.
use cupel_core::{FactKind, schema};
use schemars::Schema;
use serde_json::{Value, json};

const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

fn pretty(schema: &Schema) -> String {
    serde_json::to_string_pretty(schema).unwrap()
}

fn assert_identity(schema: &Schema, id: &str, title: &str) {
    let value = schema.as_value();
    assert_eq!(value["$schema"], json!(DRAFT_2020_12));
    assert_eq!(value["$id"], json!(id));
    assert_eq!(value["title"], json!(title));
}

#[test]
fn run_schema() {
    let schema = schema::run();
    assert_identity(
        &schema,
        "https://tysont.github.io/cupel/schema/v1/run.json",
        "Envelope",
    );
    insta::assert_snapshot!(pretty(&schema));
}

#[test]
fn finding_schema() {
    let schema = schema::finding();
    assert_identity(
        &schema,
        "https://tysont.github.io/cupel/schema/v1/finding.json",
        "Finding",
    );
    insta::assert_snapshot!(pretty(&schema));
}

#[test]
fn events_schema() {
    let schema = schema::events();
    assert_identity(
        &schema,
        "https://tysont.github.io/cupel/schema/v1/events.json",
        "Event",
    );
    insta::assert_snapshot!(pretty(&schema));
}

#[test]
fn plugin_schema_describes_the_manifest() {
    let schema = schema::plugin();
    assert_identity(
        &schema,
        "https://tysont.github.io/cupel/schema/v1/plugin.json",
        "PluginMessage",
    );
    let defs = schema.as_value()["$defs"].as_object().unwrap();
    for message in [
        "Manifest",
        "ProducerManifest",
        "RunRequest",
        "Plan",
        "PluginEvent",
    ] {
        assert!(defs.contains_key(message), "{message} is not described");
    }
    insta::assert_snapshot!(pretty(&schema));
}

#[test]
fn config_schema() {
    let schema = schema::config();
    assert_identity(
        &schema,
        "https://tysont.github.io/cupel/schema/v1/config.json",
        "ConfigFile",
    );
    insta::assert_snapshot!(pretty(&schema));
}

/// `cupel.toml` is input: every key is optional, and unknown keys at closed levels are errors.
#[test]
fn config_schema_keys_are_optional_and_closed() {
    let schema = schema::config();
    let value = schema.as_value();
    assert_eq!(value.get("required"), None);
    assert_eq!(value["additionalProperties"], json!(false));
    for def in ["Settings", "RootConfig", "OracleConfig", "GateConfig"] {
        assert_eq!(
            value["$defs"][def]["additionalProperties"],
            json!(false),
            "{def}"
        );
    }
    assert_eq!(value["$defs"]["Settings"].get("required"), None);
}

#[test]
fn fact_schemas_exist_exactly_for_kinds_with_payloads() {
    // Every kind of the catalog has a payload type, so producer plugins can provide each one.
    let without: Vec<FactKind> = FactKind::all()
        .iter()
        .copied()
        .filter(|kind| schema::fact(*kind).is_none())
        .collect();
    assert_eq!(without, []);
}

/// Snapshots each kind's schema as `schemas__fact_<kind>.snap`, so a new kind adds a file
/// (ADR-123), and returns each kind's draft, `$id`, and version.
fn snapshot_fact_schemas() -> Vec<(Value, Value, Value)> {
    let mut identities = Vec::new();
    for &kind in FactKind::all() {
        let schema = schema::fact(kind).unwrap();
        let value = schema.as_value();
        identities.push((
            value["$schema"].clone(),
            value["$id"].clone(),
            value["x-cupel-fact-version"].clone(),
        ));
        insta::assert_snapshot!(format!("fact_{kind}"), pretty(&schema));
    }
    identities
}

#[test]
fn fact_schemas() {
    let expected: Vec<(Value, Value, Value)> = FactKind::all()
        .iter()
        .map(|kind| {
            (
                json!(DRAFT_2020_12),
                json!(format!(
                    "https://tysont.github.io/cupel/schema/v1/fact/{kind}.json"
                )),
                json!(schema::fact_version(*kind)),
            )
        })
        .collect();
    assert_eq!(snapshot_fact_schemas(), expected);
}

#[test]
fn envelope_schema_types_exit_code_as_an_integer_0_to_6() {
    let schema = schema::run();
    let exit_code: &Value = &schema.as_value()["$defs"]["ExitCode"];
    assert_eq!(exit_code["type"], json!("integer"));
    assert_eq!(exit_code["enum"], json!([0, 1, 2, 3, 4, 5, 6]));
}

/// The `definition.field` pairs of `fields` that `schema` does not list as required.
fn not_required(schema: &Schema, fields: &[(&str, &str)]) -> Vec<String> {
    let defs = &schema.as_value()["$defs"];
    fields
        .iter()
        .filter(|(def, field)| {
            let required: Vec<String> =
                serde_json::from_value(defs[*def]["required"].clone()).unwrap();
            !required.iter().any(|name| name == field)
        })
        .map(|(def, field)| format!("{def}.{field}"))
        .collect()
}

/// ADR-013: every key is always emitted, absent values as `null`, so every property is required.
#[test]
fn optional_fields_are_required_and_nullable() {
    let run = [
        ("RunInfo", "git"),
        ("RunInfo", "budget_ms"),
        ("RunInfo", "baseline"),
        ("Finding", "symbol"),
        ("Finding", "detail"),
        ("Location", "col"),
        ("NextAction", "command"),
        ("CheckResult", "skipped"),
    ];
    assert_eq!(not_required(&schema::run(), &run), Vec::<String>::new());
    let symbols = schema::fact(FactKind::Symbols).unwrap();
    assert_eq!(
        not_required(&symbols, &[("SymbolDef", "body")]),
        Vec::<String>::new()
    );
}
