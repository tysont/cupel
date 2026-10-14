// ABOUTME: Deltas between two runs: findings new or resolved by `FindingId`, and check score changes,
// ABOUTME: applied to an envelope's findings, summary, checks, and `run.baseline` (ADR-046).
use std::collections::{BTreeMap, BTreeSet};

use crate::envelope::{BaselineRef, Delta, Envelope};
use crate::finding::Finding;
use crate::hash::FindingId;
use crate::id::CheckId;

/// How a run differs from the run it is compared with.
#[derive(Debug, Clone, PartialEq)]
pub struct Deltas {
    /// IDs of the current run's findings the reference run lacks, in the current run's order.
    pub new: Vec<FindingId>,
    /// The reference run's findings the current run lacks, in the reference run's order.
    pub resolved: Vec<Finding>,
    /// Current score minus reference score, for each check scored in both runs.
    pub scores: BTreeMap<CheckId, f64>,
}

/// Compares `current` with `against` by finding ID and check score.
pub fn deltas(current: &Envelope, against: &Envelope) -> Deltas {
    let before: BTreeSet<&FindingId> = against.findings.iter().map(|f| &f.id).collect();
    let after: BTreeSet<&FindingId> = current.findings.iter().map(|f| &f.id).collect();
    let reference_scores: BTreeMap<&CheckId, f64> = against
        .checks
        .iter()
        .filter_map(|check| Some((&check.id, check.score.as_ref()?.value)))
        .collect();
    Deltas {
        new: current
            .findings
            .iter()
            .filter(|f| !before.contains(&f.id))
            .map(|f| f.id.clone())
            .collect(),
        resolved: against
            .findings
            .iter()
            .filter(|f| !after.contains(&f.id))
            .cloned()
            .collect(),
        scores: current
            .checks
            .iter()
            .filter_map(|check| {
                let reference = reference_scores.get(&check.id)?;
                Some((check.id.clone(), check.score.as_ref()?.value - reference))
            })
            .collect(),
    }
}

impl Deltas {
    /// Records these deltas on `envelope`, the current run: `new_since_baseline` on each
    /// finding, `summary.new`/`resolved`, `run.baseline`, and each scored check's `delta`, whose
    /// `vs` is `vs` (`baseline`, `run:<id>`, or a git ref).
    pub fn apply(&self, envelope: &mut Envelope, baseline: BaselineRef, vs: &str) {
        let new: BTreeSet<&FindingId> = self.new.iter().collect();
        for finding in &mut envelope.findings {
            finding.new_since_baseline = new.contains(&finding.id);
        }
        for check in &mut envelope.checks {
            check.delta = self.scores.get(&check.id).map(|value| Delta {
                vs: vs.to_owned(),
                value: *value,
            });
        }
        envelope.summary.new = self.new.len() as u64;
        envelope.summary.resolved = self.resolved.len() as u64;
        envelope.run.baseline = Some(baseline);
    }
}
