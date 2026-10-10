// ABOUTME: JSON Schemas (draft 2020-12) generated from the core types, so schema and serializer agree.
// ABOUTME: Backs `cupel describe schema <name>`; each schema carries a stable `$id` URL.
use schemars::generate::SchemaSettings;
use schemars::transform::transform_subschemas;
use schemars::{JsonSchema, Schema};
use serde_json::{Value, json};

use crate::check::Manifest;
use crate::envelope::Envelope;
use crate::event::Event;
use crate::fact::{Fact, FactKind};
use crate::finding::Finding;
use crate::syntax::{
    ApiSurface, Decisions, ModuleGraph, MutationSites, NormTree, ProjectInfo, Roots, Symbols,
    TestConventions, TestFrames,
};

/// Every published schema's `$id` starts with this.
pub const BASE_URL: &str = "https://cupel.dev/schema/v1/";

/// Schema of the envelope every verb emits.
pub fn run() -> Schema {
    root::<Envelope>("run.json")
}

/// Schema of one finding.
pub fn finding() -> Schema {
    root::<Finding>("finding.json")
}

/// Schema of one `--events` line.
pub fn events() -> Schema {
    root::<Event>("events.json")
}

/// Schema of the plugin protocol. Today that is the `Manifest` a plugin's `describe` emits;
/// the run request and plugin events join it in P5.1 (ADR-013).
pub fn plugin() -> Schema {
    root::<Manifest>("plugin.json")
}

/// Schema of a fact kind's payload, or `None` for kinds whose payload type does not exist yet.
/// Execution-fact payloads are added in the phase that first produces them.
pub fn fact(kind: FactKind) -> Option<Schema> {
    match kind {
        FactKind::Symbols => Some(fact_schema::<Symbols>()),
        FactKind::Decisions => Some(fact_schema::<Decisions>()),
        FactKind::NormTree => Some(fact_schema::<NormTree>()),
        FactKind::MutationSites => Some(fact_schema::<MutationSites>()),
        FactKind::TestFrames => Some(fact_schema::<TestFrames>()),
        FactKind::ModuleGraph => Some(fact_schema::<ModuleGraph>()),
        FactKind::ApiSurface => Some(fact_schema::<ApiSurface>()),
        FactKind::Roots => Some(fact_schema::<Roots>()),
        FactKind::TestConventions => Some(fact_schema::<TestConventions>()),
        FactKind::ProjectInfo => Some(fact_schema::<ProjectInfo>()),
        FactKind::TestResults
        | FactKind::CoverageMap
        | FactKind::PerTestMap
        | FactKind::MutationOutcomes
        | FactKind::BenchSamples
        | FactKind::PropertyResults
        | FactKind::FuzzResults
        | FactKind::RaceReports
        | FactKind::AnalyzerReport
        | FactKind::DivergenceResults => None,
    }
}

/// A fact payload's schema, carrying the fact version it describes.
fn fact_schema<F: Fact>() -> Schema {
    let mut schema = root::<F>(&format!("fact/{}.json", F::KIND));
    schema.insert("x-cupel-fact-version".to_owned(), json!(F::VERSION));
    schema
}

fn root<T: JsonSchema>(path: &str) -> Schema {
    let mut schema = SchemaSettings::draft2020_12()
        .with_transform(require_every_property)
        .into_generator()
        .into_root_schema_for::<T>();
    schema.insert("$id".to_owned(), json!(format!("{BASE_URL}{path}")));
    schema
}

/// Marks every property of every object schema required. Core types never skip a field when
/// serializing; absent optional values are emitted as `null` (ADR-013).
fn require_every_property(schema: &mut Schema) {
    transform_subschemas(&mut require_every_property, schema);
    let names: Option<Vec<Value>> = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| properties.keys().cloned().map(Value::String).collect());
    if let Some(names) = names {
        schema.insert("required".to_owned(), Value::Array(names));
    }
}
