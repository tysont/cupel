// ABOUTME: `survey`'s rankings (ADR-062, ADR-132): every per-target metric the checks declare, worst first, and
// ABOUTME: the symbols findings ask to protect or simplify, joined across them, protect-first.
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::check::Manifest;
use crate::finding::{Finding, Symbol};
use crate::hash::SymbolHash;
use crate::rank::{
    Measured, MetricPer, Protection, ProtectionMetric, RankedTarget, RankingDimension, Rankings,
    Recommendation, RecommendedTarget, TargetValue, Worst,
};

/// The rankings of every per-target metric the `manifests` declare, in their order, from
/// `measured`; each dimension keeps its `top` worst values. The targets are the symbols an
/// unsuppressed finding of `findings` asks to protect or simplify, the first `top` of them.
pub fn rankings<'a>(
    manifests: impl IntoIterator<Item = &'a Manifest>,
    measured: &[Measured],
    findings: &[Finding],
    top: usize,
) -> Rankings {
    let ranked: Vec<Ranked<'_>> = manifests
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
            let worst = if spec.protection.is_some() {
                Worst::Lowest
            } else {
                Worst::Highest
            };
            values.sort_by(|a, b| order(worst, a, b));
            let dimension = RankingDimension {
                check: manifest.id.clone(),
                metric: spec.name.clone(),
                per: spec.per,
                summary: spec.summary.clone(),
                worst,
                measured: values.len() as u64,
                entries: values
                    .iter()
                    .take(top)
                    .zip(1..)
                    .map(|(m, rank)| RankedTarget {
                        rank,
                        value: m.measurement.value,
                        lang: m.lang.clone(),
                        target: m.measurement.target.clone(),
                    })
                    .collect(),
            };
            Ranked {
                dimension,
                protection: spec.protection,
                values,
            }
        })
        .collect();
    let mut targets = recommended(&ranked, findings);
    targets.sort_by(protect_first);
    let recommended = targets.len() as u64;
    Rankings {
        top: top as u64,
        recommended,
        targets: targets
            .into_iter()
            .take(top)
            .zip(1..)
            .map(|(joined, rank)| RecommendedTarget {
                rank,
                ..joined.target
            })
            .collect(),
        dimensions: ranked.into_iter().map(|r| r.dimension).collect(),
    }
}

/// One dimension with every value it measured, worst first.
struct Ranked<'a> {
    dimension: RankingDimension,
    protection: Option<ProtectionMetric>,
    values: Vec<&'a Measured>,
}

/// A target before it is ranked, with what ranks it.
struct Joined {
    target: RecommendedTarget,
    /// Its best rank in any dimension where higher is worse; `None` when it has none.
    best: Option<u64>,
}

/// The symbols an unsuppressed finding asks to protect or simplify, each joined with every
/// value the dimensions measured for it, in the findings' order.
fn recommended(ranked: &[Ranked<'_>], findings: &[Finding]) -> Vec<Joined> {
    let mut joined: Vec<Joined> = Vec::new();
    let mut index: HashMap<(&str, &SymbolHash), usize> = HashMap::new();
    for finding in findings.iter().filter(|finding| !finding.suppressed) {
        let Some(category) = Recommendation::of(finding.action.kind) else {
            continue;
        };
        let Some(symbol) = &finding.symbol else {
            continue;
        };
        let at = *index
            .entry((symbol.display.as_str(), &symbol.hash))
            .or_insert_with(|| {
                joined.push(join(ranked, finding, symbol, category));
                joined.len() - 1
            });
        let target = &mut joined[at].target;
        target.recommend = combined(target.recommend, category);
        target.findings.push(finding.id.clone());
    }
    joined
}

/// The target `finding` names as `symbol`, with its values in every dimension of `ranked`.
fn join(
    ranked: &[Ranked<'_>],
    finding: &Finding,
    symbol: &Symbol,
    category: Recommendation,
) -> Joined {
    let mut target = RecommendedTarget {
        rank: 0,
        recommend: category,
        lang: symbol.lang.clone(),
        target: crate::rank::Target {
            display: symbol.display.clone(),
            symbol: Some(symbol.hash.clone()),
            location: finding.location.clone(),
        },
        protection: Protection::default(),
        values: Vec::new(),
        findings: Vec::new(),
    };
    let mut best = None;
    for dimension in ranked {
        let Some((at, measured)) = dimension.values.iter().enumerate().find(|(_, m)| {
            let of = &m.measurement.target;
            of.display == symbol.display && of.symbol.as_ref() == Some(&symbol.hash)
        }) else {
            continue;
        };
        let rank = at as u64 + 1;
        let value = measured.measurement.value;
        // The measured symbol's span locates it better than a finding's site does.
        if let Some(location) = &measured.measurement.target.location {
            target.target.location = Some(location.clone());
        }
        match dimension.protection {
            Some(ProtectionMetric::MutationScore) => {
                target.protection.mutation_score = Some(value);
            }
            Some(ProtectionMetric::Coverage) => target.protection.coverage = Some(value),
            Some(ProtectionMetric::Tests) => target.protection.tests = Some(count(value)),
            None => best = Some(best.map_or(rank, |best: u64| best.min(rank))),
        }
        target.values.push(TargetValue {
            check: dimension.dimension.check.clone(),
            metric: dimension.dimension.metric.clone(),
            value,
            rank,
        });
    }
    Joined { target, best }
}

fn combined(a: Recommendation, b: Recommendation) -> Recommendation {
    if a == b { a } else { Recommendation::Both }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a test count is a whole, non-negative number far below 2^52"
)]
fn count(value: f64) -> u64 {
    value.max(0.0) as u64
}

/// Targets to protect first; then the weakest protection (the mutation score, else coverage,
/// unmeasured weakest of all); then the best rank in any dimension where higher is worse; then
/// display name, file, and line.
fn protect_first(a: &Joined, b: &Joined) -> Ordering {
    let strength = |joined: &Joined| {
        let protection = &joined.target.protection;
        protection
            .mutation_score
            .or(protection.coverage)
            .unwrap_or(f64::NEG_INFINITY)
    };
    let best = |joined: &Joined| joined.best.unwrap_or(u64::MAX);
    b.target
        .recommend
        .protects()
        .cmp(&a.target.recommend.protects())
        .then_with(|| strength(a).total_cmp(&strength(b)))
        .then_with(|| best(a).cmp(&best(b)))
        .then_with(|| place(&a.target.target).cmp(&place(&b.target.target)))
}

/// Display name, then file and line: how equal values are ordered.
fn place(target: &crate::rank::Target) -> (String, Option<(String, u32)>) {
    (
        target.display.clone(),
        target
            .location
            .as_ref()
            .map(|location| (location.file.clone(), location.line)),
    )
}

/// The worse value first; ties by display name, then file and line.
fn order(worst: Worst, a: &Measured, b: &Measured) -> Ordering {
    let by_value = match worst {
        Worst::Highest => b.measurement.value.total_cmp(&a.measurement.value),
        Worst::Lowest => a.measurement.value.total_cmp(&b.measurement.value),
    };
    by_value.then_with(|| place(&a.measurement.target).cmp(&place(&b.measurement.target)))
}
