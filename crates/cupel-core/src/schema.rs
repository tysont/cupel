// ABOUTME: JSON Schemas (draft 2020-12) generated from the core types, so schema and serializer agree.
// ABOUTME: Backs `cupel describe schema <name>`; each schema carries a stable `$id` URL.
use schemars::generate::SchemaSettings;
use schemars::transform::transform_subschemas;
use schemars::{JsonSchema, Schema};
use serde_json::{Value, json};

use crate::config::ConfigFile;
use crate::envelope::Envelope;
use crate::event::Event;
use crate::fact::{Fact, FactKind};
use crate::finding::Finding;
use crate::plugin::PluginMessage;

/// Every published schema's `$id` starts with this.
pub const BASE_URL: &str = "https://cupel.dev/schema/v1/";

/// Every top-level schema `describe schema <name>` serves, by name. A schema named `n` has the
/// `$id` [`url`]`(n)`. Register a new top-level schema here.
pub const PUBLISHED: &[(&str, Generate)] = &[
    ("run", run),
    ("finding", finding),
    ("events", events),
    ("config", config),
    ("plugin", plugin),
];

/// Generates one top-level schema.
pub type Generate = fn() -> Schema;

/// The `$id` of the top-level schema called `name`.
pub fn url(name: &str) -> String {
    format!("{BASE_URL}{name}.json")
}

/// Schema of the envelope every verb emits.
pub fn run() -> Schema {
    root::<Envelope>(&url("run"))
}

/// Schema of one finding.
pub fn finding() -> Schema {
    root::<Finding>(&url("finding"))
}

/// Schema of one `--events` line.
pub fn events() -> Schema {
    root::<Event>(&url("events"))
}

/// Schema of `cupel.toml` (design §6.7).
pub fn config() -> Schema {
    input_root::<ConfigFile>(&url("config"))
}

/// Schema of the plugin protocol: every message a plugin prints or reads (ADR-013 §9, ADR-087).
pub fn plugin() -> Schema {
    root::<PluginMessage>(&url("plugin"))
}

/// Schema of a fact kind's payload, from the kind's catalog entry (ADR-123); `None` for a kind
/// whose payload module submits no entry, which the catalog tests rule out.
pub fn fact(kind: FactKind) -> Option<Schema> {
    kind.find_entry().map(|entry| (entry.schema)())
}

/// The payload version of a fact kind, or `None` for a kind without a catalog entry.
pub fn fact_version(kind: FactKind) -> Option<u32> {
    kind.find_entry().map(|entry| entry.version)
}

/// A fact payload's schema, carrying the fact version it describes.
pub(crate) fn fact_schema<F: Fact>() -> Schema {
    let mut schema = root::<F>(&format!("{BASE_URL}fact/{}.json", F::KIND));
    schema.insert("x-cupel-fact-version".to_owned(), json!(F::VERSION));
    schema
}

/// Schema of a type Cupel emits: every property is required (ADR-013).
fn root<T: JsonSchema>(id: &str) -> Schema {
    with_id(
        SchemaSettings::draft2020_12()
            .with_transform(require_every_property)
            .into_generator()
            .into_root_schema_for::<T>(),
        id,
    )
}

/// Schema of a type Cupel reads, such as `cupel.toml`, where keys may be omitted.
fn input_root<T: JsonSchema>(id: &str) -> Schema {
    with_id(
        SchemaSettings::draft2020_12()
            .into_generator()
            .into_root_schema_for::<T>(),
        id,
    )
}

fn with_id(mut schema: Schema, id: &str) -> Schema {
    schema.insert("$id".to_owned(), json!(id));
    schema
}

/// Marks every property of every object schema required. Core types never skip a field when
/// serializing; absent optional values are emitted as `null` (ADR-013). The exception is a
/// property marked `x-cupel-optional`, which is omitted when empty (ADR-079).
fn require_every_property(schema: &mut Schema) {
    transform_subschemas(&mut require_every_property, schema);
    // A property marked `x-cupel-optional` is omitted when empty (ADR-079).
    let names: Option<Vec<Value>> =
        schema
            .get("properties")
            .and_then(Value::as_object)
            .map(|properties| {
                properties
                    .iter()
                    .filter(|(_, property)| property.get("x-cupel-optional").is_none())
                    .map(|(name, _)| Value::String(name.clone()))
                    .collect()
            });
    if let Some(names) = names {
        schema.insert("required".to_owned(), Value::Array(names));
    }
}
