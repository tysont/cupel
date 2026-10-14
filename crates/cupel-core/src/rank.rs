// ABOUTME: Per-target measurements checks report through `EventSink::measure`, and the `rankings` that
// ABOUTME: `survey` builds from them: the top values of every per-target metric a manifest declares (ADR-062).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::finding::Location;
use crate::hash::SymbolHash;
use crate::id::{CheckId, LangId};

/// How many entries each ranking dimension keeps (ADR-062 §5).
pub const RANKED: usize = 10;

/// One value a check measured for one target, such as one function's cyclomatic complexity.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    /// A metric the manifest declares with `per` other than `scope`.
    pub metric: String,
    pub value: f64,
    pub target: Target,
}

/// What a measurement is about.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Target {
    /// How people name it: a symbol's display name, a module path, a cluster's description.
    pub display: String,
    /// The structural hash, when the target is a symbol.
    pub symbol: Option<SymbolHash>,
    pub location: Option<Location>,
}

/// A measurement with the check and language that produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    pub check: CheckId,
    pub lang: LangId,
    pub measurement: Measurement,
}

/// What `survey` adds to its envelope: the highest values of every ranking dimension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Rankings {
    /// The most entries a dimension lists.
    pub top: u64,
    pub dimensions: Vec<RankingDimension>,
}

/// One per-target metric of one check, with its worst targets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RankingDimension {
    pub check: CheckId,
    pub metric: String,
    pub per: MetricPer,
    /// The metric's summary from the manifest.
    pub summary: String,
    /// How many targets were measured.
    pub measured: u64,
    /// The highest values first, at most `top`.
    pub entries: Vec<RankedTarget>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RankedTarget {
    /// 1 is the highest value.
    pub rank: u64,
    pub value: f64,
    pub lang: LangId,
    pub target: Target,
}

/// What one value of a metric describes: the whole scope, reported in `CheckResult.metrics` and
/// usable as a gate key, or one target, reported through `EventSink::measure` and ranked by
/// `survey`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MetricPer {
    Scope,
    /// A function or method.
    Symbol,
    /// A file or module.
    Module,
    /// A set of targets, such as a clone cluster or a dependency cycle.
    Group,
}
