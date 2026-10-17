// ABOUTME: Builders every native check's manifest and evidence share: finding kinds whose evidence is a
// ABOUTME: JSON Schema, `scope` and per-target metric specs, and evidence serialization (ADR-012, ADR-062).
use cupel_core::{ActionKind, FindingKindSpec, MetricPer, MetricSpec};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

/// A finding kind mapped to `action`, whose evidence is `T`'s JSON Schema (ADR-012).
pub fn kind<T: JsonSchema>(kind: &str, action: ActionKind, summary: &str) -> FindingKindSpec {
    let schema = match schemars::schema_for!(T).to_value() {
        Value::Object(schema) => schema,
        _ => Map::new(),
    };
    FindingKindSpec {
        kind: kind.to_owned(),
        action,
        summary: summary.to_owned(),
        evidence: schema,
    }
}

/// A `scope` metric: one value over the scope, reported in `CheckResult.metrics`; a gate key.
pub fn metric(name: &str, summary: &str) -> MetricSpec {
    ranked(name, MetricPer::Scope, summary)
}

/// A metric measured once per target `per` describes: a `survey` ranking dimension (ADR-062).
pub fn ranked(name: &str, per: MetricPer, summary: &str) -> MetricSpec {
    MetricSpec {
        name: name.to_owned(),
        summary: summary.to_owned(),
        per,
    }
}

/// How `value`, a unit enum variant, is spelled in JSON (`api-surface`, `missing`); empty for
/// anything else.
pub fn spelling<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(text)) => text,
        _ => String::new(),
    }
}

/// `value` as JSON evidence; a value that cannot be serialized is null.
pub fn evidence<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}
