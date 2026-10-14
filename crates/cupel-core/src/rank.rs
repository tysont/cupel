// ABOUTME: Per-target measurements checks report through `EventSink::measure`, and the `rankings` that
// ABOUTME: `survey` builds from them: the top values of every per-target metric a manifest declares (ADR-062).
use std::cmp::Ordering;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::check::{Manifest, MetricPer};
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

/// The rankings of every per-target metric the `manifests` declare, in their order, from
/// `measured`; each dimension keeps its `top` highest values.
pub fn rankings<'a>(
    manifests: impl IntoIterator<Item = &'a Manifest>,
    measured: &[Measured],
    top: usize,
) -> Rankings {
    let dimensions = manifests
        .into_iter()
        .flat_map(|manifest| {
            manifest
                .metrics
                .iter()
                .filter(|spec| spec.per != MetricPer::Scope)
                .map(move |spec| (manifest, spec))
        })
        .map(|(manifest, spec)| {
            let mut values: Vec<&Measured> = measured
                .iter()
                .filter(|m| m.check == manifest.id && m.measurement.metric == spec.name)
                .collect();
            values.sort_by(|a, b| order(a, b));
            RankingDimension {
                check: manifest.id.clone(),
                metric: spec.name.clone(),
                per: spec.per,
                summary: spec.summary.clone(),
                measured: values.len() as u64,
                entries: values
                    .into_iter()
                    .take(top)
                    .zip(1..)
                    .map(|(m, rank)| RankedTarget {
                        rank,
                        value: m.measurement.value,
                        lang: m.lang.clone(),
                        target: m.measurement.target.clone(),
                    })
                    .collect(),
            }
        })
        .collect();
    Rankings {
        top: top as u64,
        dimensions,
    }
}

/// Highest value first; ties by display name, then file and line.
fn order(a: &Measured, b: &Measured) -> Ordering {
    let place = |m: &Measured| {
        let target = &m.measurement.target;
        (
            target.display.clone(),
            target
                .location
                .as_ref()
                .map(|location| (location.file.clone(), location.line)),
        )
    };
    b.measurement
        .value
        .total_cmp(&a.measurement.value)
        .then_with(|| place(a).cmp(&place(b)))
}
