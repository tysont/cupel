// ABOUTME: Deltas between two runs: findings new or resolved by `FindingId` and across symbol matches, check score
// ABOUTME: changes, and per-symbol metrics over matched sets, applied to an envelope (ADR-046, ADR-131).
use std::collections::{BTreeMap, BTreeSet};

use crate::envelope::{BaselineRef, Envelope};
use crate::finding::Finding;
use crate::hash::{FindingId, SymbolHash};
use crate::id::CheckId;
use crate::matching::{Matched, SymbolMatch};
use crate::rank::Measured;
use crate::result::{Delta, SetDelta, SetFindings, SetMetric};

/// How a run differs from the run it is compared with.
#[derive(Debug, Clone, PartialEq)]
pub struct Deltas {
    /// IDs of the current run's findings the reference run lacks, in the current run's order.
    pub new: Vec<FindingId>,
    /// The reference run's findings the current run lacks, in the reference run's order.
    pub resolved: Vec<Finding>,
    /// Current score minus reference score, for each check scored in both runs.
    pub scores: BTreeMap<CheckId, f64>,
    /// Per check, the matched sets it had findings or per-symbol metrics on (ADR-131).
    pub sets: BTreeMap<CheckId, Vec<SetDelta>>,
}

/// Per-symbol metric values of one run, by check, metric, and symbol.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SymbolMetrics {
    values: BTreeMap<CheckId, BTreeMap<String, BTreeMap<SymbolHash, f64>>>,
}

impl SymbolMetrics {
    /// The measurements of symbols among `measured`; others (modules, clusters) are left out.
    pub fn of(measured: &[Measured]) -> Self {
        let mut metrics = Self::default();
        for each in measured {
            if let Some(symbol) = &each.measurement.target.symbol {
                metrics.insert(
                    &each.check,
                    &each.measurement.metric,
                    symbol.clone(),
                    each.measurement.value,
                );
            }
        }
        metrics
    }

    /// Records `value` of `metric` for `symbol`, replacing any earlier one.
    pub fn insert(&mut self, check: &CheckId, metric: &str, symbol: SymbolHash, value: f64) {
        self.values
            .entry(check.clone())
            .or_default()
            .entry(metric.to_owned())
            .or_default()
            .insert(symbol, value);
    }

    /// The names of the metrics `check` measured.
    fn metrics_of(&self, check: &CheckId) -> impl Iterator<Item = &String> {
        self.values.get(check).into_iter().flat_map(BTreeMap::keys)
    }

    /// `metric` of `check` over `members`: the sum, or with `mean` the mean weighted by tree
    /// size, of the members measured; `None` when none was.
    fn over(&self, check: &CheckId, metric: &str, members: &[Matched], mean: bool) -> Option<f64> {
        let values = self.values.get(check)?.get(metric)?;
        let measured: Vec<(f64, f64)> = members
            .iter()
            .filter_map(|member| {
                let value = values.get(&member.symbol.hash)?;
                Some((*value, f64::from(member.nodes)))
            })
            .collect();
        if measured.is_empty() {
            return None;
        }
        let sum: f64 = measured.iter().map(|(value, _)| value).sum();
        if !mean {
            return Some(sum);
        }
        let weight: f64 = measured.iter().map(|(_, weight)| weight).sum();
        let weighted: f64 = measured.iter().map(|(value, weight)| value * weight).sum();
        Some(if weight > 0.0 { weighted / weight } else { sum })
    }
}

/// What two runs know about their symbols beyond finding IDs (ADR-131): the matches between the
/// reference's symbols and the current run's, and each run's per-symbol metrics.
#[derive(Debug, Default)]
pub struct Matching<'a> {
    pub matches: &'a [SymbolMatch],
    pub current: SymbolMetrics,
    pub reference: SymbolMetrics,
    /// The `(check, metric)` pairs that are scores, compared as weighted means; every other
    /// per-symbol metric is a count, compared as a sum.
    pub means: BTreeSet<(CheckId, String)>,
}

/// Compares `current` with `against` by finding ID and check score.
pub fn deltas(current: &Envelope, against: &Envelope) -> Deltas {
    deltas_matched(current, against, &Matching::default())
}

/// Compares `current` with `against` by finding ID and check score, then across `matching`'s
/// matches: a reference finding on a match's base set and a current finding of the same check
/// and kind on its head set carry each other on, so neither is resolved or new (ADR-131).
pub fn deltas_matched(current: &Envelope, against: &Envelope, matching: &Matching<'_>) -> Deltas {
    let before: BTreeSet<&FindingId> = against.findings.iter().map(|f| &f.id).collect();
    let after: BTreeSet<&FindingId> = current.findings.iter().map(|f| &f.id).collect();
    let new: Vec<&Finding> = current
        .findings
        .iter()
        .filter(|f| !before.contains(&f.id))
        .collect();
    let resolved: Vec<&Finding> = against
        .findings
        .iter()
        .filter(|f| !after.contains(&f.id))
        .collect();
    let mut carried: BTreeSet<FindingId> = BTreeSet::new();
    let mut sets: BTreeMap<CheckId, Vec<SetDelta>> = BTreeMap::new();
    for found in matching.matches {
        for (check, set) in compare_set(found, &resolved, &new, matching) {
            carried.extend(set.findings.base.iter().cloned());
            carried.extend(set.findings.head.iter().cloned());
            sets.entry(check).or_default().push(set);
        }
    }
    Deltas {
        new: new
            .iter()
            .filter(|f| !carried.contains(&f.id))
            .map(|f| f.id.clone())
            .collect(),
        resolved: resolved
            .into_iter()
            .filter(|f| !carried.contains(&f.id))
            .cloned()
            .collect(),
        scores: score_changes(current, against),
        sets,
    }
}

/// Current score minus reference score, for each check scored in both runs.
fn score_changes(current: &Envelope, against: &Envelope) -> BTreeMap<CheckId, f64> {
    let reference_scores: BTreeMap<&CheckId, f64> = against
        .checks
        .iter()
        .filter_map(|check| Some((&check.id, check.score.as_ref()?.value)))
        .collect();
    current
        .checks
        .iter()
        .filter_map(|check| {
            let reference = reference_scores.get(&check.id)?;
            Some((check.id.clone(), check.score.as_ref()?.value - reference))
        })
        .collect()
}

/// For each check with findings to carry or per-symbol metrics to compare across `found`, its
/// set delta.
fn compare_set(
    found: &SymbolMatch,
    resolved: &[&Finding],
    new: &[&Finding],
    matching: &Matching<'_>,
) -> Vec<(CheckId, SetDelta)> {
    let on = |findings: &[&Finding], members: &[Matched]| -> Vec<(CheckId, String, FindingId)> {
        findings
            .iter()
            .filter(|finding| {
                finding.symbol.as_ref().is_some_and(|symbol| {
                    members
                        .iter()
                        .any(|member| member.symbol.hash == symbol.hash)
                })
            })
            .map(|finding| {
                (
                    finding.check.clone(),
                    finding.kind.clone(),
                    finding.id.clone(),
                )
            })
            .collect()
    };
    let (gone, came) = (on(resolved, &found.base), on(new, &found.head));
    let checks: BTreeSet<&CheckId> = gone
        .iter()
        .chain(&came)
        .map(|(check, _, _)| check)
        .chain(matching.current.values.keys())
        .chain(matching.reference.values.keys())
        .collect();
    checks
        .into_iter()
        .filter_map(|check| {
            let findings = carried(check, &gone, &came);
            let metrics = set_metrics(check, found, matching);
            if findings.base.is_empty() && metrics.is_empty() {
                return None;
            }
            let symbols = |members: &[Matched]| members.iter().map(|m| m.symbol.clone()).collect();
            Some((
                check.clone(),
                SetDelta {
                    base: symbols(&found.base),
                    head: symbols(&found.head),
                    similarity: found.similarity,
                    findings,
                    metrics,
                },
            ))
        })
        .collect()
}

/// The findings of `check` carried across a match: of every kind with findings on both sides,
/// the reference's (`gone`) and the current run's (`came`).
fn carried(
    check: &CheckId,
    gone: &[(CheckId, String, FindingId)],
    came: &[(CheckId, String, FindingId)],
) -> SetFindings {
    let kinds = |side: &[(CheckId, String, FindingId)]| -> BTreeSet<String> {
        side.iter()
            .filter(|(of, _, _)| of == check)
            .map(|(_, kind, _)| kind.clone())
            .collect()
    };
    let both: BTreeSet<String> = kinds(gone).intersection(&kinds(came)).cloned().collect();
    let ids = |side: &[(CheckId, String, FindingId)]| -> Vec<FindingId> {
        side.iter()
            .filter(|(of, kind, _)| of == check && both.contains(kind))
            .map(|(_, _, id)| id.clone())
            .collect()
    };
    SetFindings {
        base: ids(gone),
        head: ids(came),
    }
}

/// Each per-symbol metric of `check` measured on both sides of `found`, over its sets.
fn set_metrics(
    check: &CheckId,
    found: &SymbolMatch,
    matching: &Matching<'_>,
) -> BTreeMap<String, SetMetric> {
    matching
        .current
        .metrics_of(check)
        .filter_map(|metric| {
            let mean = matching.means.contains(&(check.clone(), metric.clone()));
            let base = matching.reference.over(check, metric, &found.base, mean)?;
            let head = matching.current.over(check, metric, &found.head, mean)?;
            Some((metric.clone(), SetMetric { base, head }))
        })
        .collect()
}

impl Deltas {
    /// Records these deltas on `envelope`, the current run: `new_since_baseline` on each
    /// finding, `summary.new`/`resolved`, `run.baseline`, and each scored check's `delta`, whose
    /// `vs` is `vs` (`baseline`, `run:<id>`, or a git ref) and which carries the check's sets.
    pub fn apply(&self, envelope: &mut Envelope, baseline: BaselineRef, vs: &str) {
        let new: BTreeSet<&FindingId> = self.new.iter().collect();
        for finding in &mut envelope.findings {
            finding.new_since_baseline = new.contains(&finding.id);
        }
        for check in &mut envelope.checks {
            check.delta = self.scores.get(&check.id).map(|value| Delta {
                vs: vs.to_owned(),
                value: *value,
                sets: self.sets.get(&check.id).cloned().unwrap_or_default(),
            });
        }
        envelope.summary.new = self.new.len() as u64;
        envelope.summary.resolved = self.resolved.len() as u64;
        envelope.run.baseline = Some(baseline);
    }
}
