// ABOUTME: `survey`'s rankings (ADR-062): every per-target metric the checks declare, its measured targets
// ABOUTME: highest first; joins manifests with measurements, so it sits above both.
use std::cmp::Ordering;

use crate::check::Manifest;
use crate::rank::{Measured, MetricPer, RankedTarget, RankingDimension, Rankings};

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
