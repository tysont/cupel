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
    assert_identity(&schema, "https://cupel.dev/schema/v1/run.json", "Envelope");
    insta::assert_snapshot!(pretty(&schema));
}

#[test]
fn finding_schema() {
    let schema = schema::finding();
    assert_identity(
        &schema,
        "https://cupel.dev/schema/v1/finding.json",
        "Finding",
    );
    insta::assert_snapshot!(pretty(&schema));
}

#[test]
fn events_schema() {
    let schema = schema::events();
    assert_identity(&schema, "https://cupel.dev/schema/v1/events.json", "Event");
    insta::assert_snapshot!(pretty(&schema));
}

#[test]
fn plugin_schema_describes_the_manifest() {
    let schema = schema::plugin();
    assert_identity(
        &schema,
        "https://cupel.dev/schema/v1/plugin.json",
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
        "https://cupel.dev/schema/v1/config.json",
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
    let without: Vec<FactKind> = FactKind::ALL
        .into_iter()
        .filter(|kind| schema::fact(*kind).is_none())
        .collect();
    assert_eq!(without, []);
}

#[test]
fn the_execution_facts_of_wrapped_tools_are_at_version_one_under_their_kind() {
    let described: Vec<(FactKind, Option<u32>, Value)> = [
        FactKind::BenchSamples,
        FactKind::FuzzResults,
        FactKind::RaceReports,
        FactKind::Traces,
        FactKind::ModelCheckResults,
        FactKind::ProofResults,
    ]
    .into_iter()
    .map(|kind| {
        let id = schema::fact(kind).map(|schema| schema.as_value()["$id"].clone());
        (kind, schema::fact_version(kind), id.unwrap_or(Value::Null))
    })
    .collect();
    assert_eq!(
        described,
        [
            (
                FactKind::BenchSamples,
                Some(1),
                json!("https://cupel.dev/schema/v1/fact/bench-samples.json")
            ),
            (
                FactKind::FuzzResults,
                Some(1),
                json!("https://cupel.dev/schema/v1/fact/fuzz-results.json")
            ),
            (
                FactKind::RaceReports,
                Some(1),
                json!("https://cupel.dev/schema/v1/fact/race-reports.json")
            ),
            (
                FactKind::Traces,
                Some(1),
                json!("https://cupel.dev/schema/v1/fact/traces.json")
            ),
            (
                FactKind::ModelCheckResults,
                Some(1),
                json!("https://cupel.dev/schema/v1/fact/model-check-results.json")
            ),
            (
                FactKind::ProofResults,
                Some(1),
                json!("https://cupel.dev/schema/v1/fact/proof-results.json")
            ),
        ]
    );
}

#[test]
fn fact_schemas() {
    for kind in FactKind::SYNTACTIC {
        let schema = schema::fact(kind).unwrap();
        let value = schema.as_value();
        assert_eq!(value["$schema"], json!(DRAFT_2020_12));
        assert_eq!(
            value["$id"],
            json!(format!("https://cupel.dev/schema/v1/fact/{kind}.json"))
        );
        assert_eq!(
            value["x-cupel-fact-version"],
            json!(schema::fact_version(kind)),
            "{kind}"
        );
        insta::assert_snapshot!(format!("fact_{kind}"), pretty(&schema));
    }
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
