// ABOUTME: Builders every native check's manifest and evidence share: finding kinds whose evidence is a
// ABOUTME: JSON Schema, `scope`, per-target, and protection metric specs, settings, and evidence (ADR-012, ADR-062, ADR-111, ADR-132).
use cupel_core::{
    ActionKind, CheckId, CostClass, FactKind, FindingKindSpec, Manifest, MetricPer, MetricSpec,
    ProtectionMetric, SettingSpec, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

/// A finding kind mapped to `action`, whose evidence is `T`'s JSON Schema (ADR-012), with every
/// type it refers to written in place, since the evidence validator reads no `$defs` (ADR-043).
pub fn kind<T: JsonSchema>(kind: &str, action: ActionKind, summary: &str) -> FindingKindSpec {
    let mut settings = schemars::generate::SchemaSettings::default();
    settings.inline_subschemas = true;
    let schema = match settings
        .into_generator()
        .into_root_schema_for::<T>()
        .to_value()
    {
        Value::Object(schema) => schema,
        _ => Map::new(),
    };
    FindingKindSpec {
        kind: kind.to_owned(),
        action,
        summary: summary.to_owned(),
        evidence: schema,
        hints: Map::new(),
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
        protection: None,
    }
}

/// A per-symbol metric that says how well tests protect the symbol, lower weaker: a `survey`
/// ranking dimension ranked lowest first, and a value of each protect-first target (ADR-132).
pub fn protection(name: &str, protection: ProtectionMetric, summary: &str) -> MetricSpec {
    MetricSpec {
        protection: Some(protection),
        ..ranked(name, MetricPer::Symbol, summary)
    }
}

/// A setting read from `[checks."<id>"]`, with its default (ADR-111).
pub fn setting(name: &str, default: Value, summary: &str) -> SettingSpec {
    SettingSpec {
        name: name.to_owned(),
        default,
        summary: summary.to_owned(),
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

/// What a check over one execution fact, with no settings, declares (ADR-116, ADR-117,
/// ADR-118): all its manifest holds but what every such check shares (version 1, no wants, no
/// settings, experimental).
pub struct Declared {
    pub id: &'static str,
    pub tier: u8,
    pub cost: CostClass,
    pub needs: FactKind,
    pub findings: fn() -> Vec<FindingKindSpec>,
    /// Each metric, with what it counts.
    pub metrics: &'static [(&'static str, &'static str)],
    pub summary: &'static str,
}

impl Declared {
    /// The manifest.
    ///
    /// # Panics
    ///
    /// When `tier` is not a tier.
    pub fn manifest(&self) -> Manifest {
        Manifest {
            id: CheckId::from(self.id),
            version: 1,
            tier: Tier::try_from(self.tier).expect("a check declares a tier that exists"),
            cost: self.cost,
            needs: vec![self.needs],
            wants: vec![],
            findings: (self.findings)(),
            metrics: self
                .metrics
                .iter()
                .map(|(name, counts)| metric(name, counts))
                .collect(),
            settings: Vec::new(),
            stability: Stability::Experimental,
            summary: self.summary.to_owned(),
        }
    }
}
