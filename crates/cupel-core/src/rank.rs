// ABOUTME: Per-target measurements checks report through `EventSink::measure`, and the `rankings` that
// ABOUTME: `survey` builds from them: the worst values of every per-target metric, and protect-first targets (ADR-062, ADR-132).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::finding::Location;
use crate::hash::{FindingId, SymbolHash};
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

/// What `survey` adds to its envelope: the worst values of every ranking dimension, and the
/// symbols to act on joined across them, protect-first (ADR-132).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Rankings {
    /// The most entries a dimension, and `targets`, lists.
    pub top: u64,
    pub dimensions: Vec<RankingDimension>,
    /// How many symbols have a finding whose action protects or simplifies them.
    pub recommended: u64,
    /// Those symbols, the ones to protect first, at most `top`.
    pub targets: Vec<RecommendedTarget>,
}

/// One per-target metric of one check, with its worst targets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RankingDimension {
    pub check: CheckId,
    pub metric: String,
    pub per: MetricPer,
    /// The metric's summary from the manifest.
    pub summary: String,
    /// Which end of the metric is worse, and so ranks first.
    pub worst: Worst,
    /// How many targets were measured.
    pub measured: u64,
    /// The worst values first, at most `top`.
    pub entries: Vec<RankedTarget>,
}

/// Which end of a ranking dimension is worse: `lowest` for a protection metric, `highest` for
/// every other (ADR-132).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Worst {
    Highest,
    Lowest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RankedTarget {
    /// 1 is the worst value.
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

/// How well tests protect a target, as a per-target metric reports it; lower is weaker
/// (ADR-132).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProtectionMetric {
    /// The killed ratio of the target's scored mutants.
    MutationScore,
    /// The ratio of the target's instrumented lines a test ran.
    Coverage,
    /// How many tests ran against the target.
    Tests,
}

/// What to do about a target, protect-first: write or strengthen the tests that protect it
/// before refactoring it (ADR-132).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Recommendation {
    /// A finding asks for a test (`write_test`, `strengthen_assertion`).
    Protect,
    /// A finding asks for a refactoring (`reduce_complexity`, `refactor_extract`).
    Simplify,
    /// Both: protect it first, then simplify it.
    Both,
}

impl Recommendation {
    /// Whether the target is to be protected before anything else is done to it.
    pub fn protects(self) -> bool {
        matches!(self, Self::Protect | Self::Both)
    }
}

/// One symbol joined across the ranking dimensions, with what protects it and what to do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RecommendedTarget {
    /// 1 is the first to act on.
    pub rank: u64,
    pub recommend: Recommendation,
    pub lang: LangId,
    pub target: Target,
    pub protection: Protection,
    /// Every per-target value measured for the symbol, in dimension order.
    pub values: Vec<TargetValue>,
    /// The findings that recommend it, in envelope order.
    pub findings: Vec<FindingId>,
}

/// The protection values measured for a target; null where nothing measured one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Protection {
    pub mutation_score: Option<f64>,
    pub coverage: Option<f64>,
    pub tests: Option<u64>,
}

/// One value of a target in one dimension, with the target's rank there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TargetValue {
    pub check: CheckId,
    pub metric: String,
    pub value: f64,
    /// Its rank among every target the dimension measured; 1 is the worst.
    pub rank: u64,
}
