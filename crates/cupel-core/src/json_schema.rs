// ABOUTME: Root JSON Schemas (draft 2020-12) with Cupel's conventions: a `$id`, and every property required for
// ABOUTME: types Cupel emits (ADR-013). A leaf module, so the fact catalog can build schemas without `schema.rs`.
use schemars::generate::SchemaSettings;
use schemars::transform::transform_subschemas;
use schemars::{JsonSchema, Schema};
use serde_json::{Value, json};

/// Every published schema's `$id` starts with this: the `schema/v1/` path of this repository's GitHub Pages
/// site, where the pages workflow publishes every schema (ADR-127).
pub const BASE_URL: &str = "https://tysont.github.io/cupel/schema/v1/";

/// Schema of a type Cupel emits: every property is required (ADR-013).
pub(crate) fn root<T: JsonSchema>(id: &str) -> Schema {
    with_id(
        SchemaSettings::draft2020_12()
            .with_transform(require_every_property)
            .into_generator()
            .into_root_schema_for::<T>(),
        id,
    )
}

pub(crate) fn with_id(mut schema: Schema, id: &str) -> Schema {
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
