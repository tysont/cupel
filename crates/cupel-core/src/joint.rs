// ABOUTME: When the fact store makes several facts in one joint run (ADR-084): the facts a run reaches, the joint
// ABOUTME: that stands in for exactly the producers the store would choose and saves a run, and what a dry run says.
use std::collections::BTreeSet;

use crate::fact::FactKind;
use crate::grade::CostClass;
use crate::lang::Lang;
use crate::producer::{
    Availability, Ctx, JointProducer, ProducerRegistry, RegisteredProducer, Upstream,
};

/// The note of an estimate that runs nothing.
pub(crate) const CACHED: &str = "cached";

/// The facts that run a test suite on their own, listed in a dry run's `needs` when a check
/// reaches them (ADR-084 §8).
const SUITE: [FactKind; 2] = [FactKind::TestResults, FactKind::CoverageMap];

/// What the store knows when it decides how to make a fact for one language.
pub(crate) struct Deciding<'a> {
    pub producers: &'a ProducerRegistry,
    pub lang: &'a Lang,
    pub ctx: &'a Ctx,
    pub depth: CostClass,
    /// The facts already made for the language.
    pub upstream: Upstream,
}

impl Deciding<'_> {
    /// Every fact producing `needs` for the language may make: the needs, and through every
    /// producer of each, its needs and the wants it would make within the depth.
    pub fn reached(&self, needs: &[FactKind]) -> BTreeSet<FactKind> {
        let mut reached = BTreeSet::new();
        self.reach(needs, &mut reached);
        reached
    }

    fn reach(&self, kinds: &[FactKind], reached: &mut BTreeSet<FactKind>) {
        for kind in kinds {
            if !reached.insert(*kind) {
                continue;
            }
            for producer in self.producers.producers_for(*kind, self.lang) {
                self.reach(producer.needs(), reached);
                let wants: Vec<FactKind> = producer
                    .wants()
                    .iter()
                    .copied()
                    .filter(|want| self.affordable(*want))
                    .collect();
                self.reach(&wants, reached);
            }
        }
    }

    /// Whether some producer of `kind` estimates making it within the depth.
    fn affordable(&self, kind: FactKind) -> bool {
        self.producers
            .producers_for(kind, self.lang)
            .any(|producer| producer.estimate(self.ctx, &self.upstream).cost <= self.depth)
    }

    /// The producer the store would choose for `kind`: the first ready candidate.
    pub fn chosen(&self, kind: FactKind) -> Option<&dyn RegisteredProducer> {
        self.producers
            .candidates(kind, self.lang, self.ctx)
            .ok()?
            .into_iter()
            .find(|producer| producer.availability(self.ctx) == Availability::Ready)
    }

    /// The joint to make `kind` with: one that makes it, is ready, stands in for exactly the
    /// producer the store would choose for each fact it makes, is within the depth, and saves
    /// a run — it runs nothing, or at least two of its facts in `reached` would each run on
    /// their own (`made` says which are made already).
    pub fn joint(
        &self,
        kind: FactKind,
        reached: &BTreeSet<FactKind>,
        made: &dyn Fn(FactKind) -> bool,
    ) -> Option<(&dyn JointProducer, bool)> {
        self.producers.joints_for(self.lang).find_map(|joint| {
            let makes = joint.makes();
            let applies = makes.iter().any(|(made, _)| *made == kind)
                && joint.availability(self.ctx) == Availability::Ready
                && makes.iter().all(|(made, id)| {
                    self.chosen(*made)
                        .is_some_and(|producer| producer.id().as_str() == *id)
                });
            if !applies {
                return None;
            }
            let estimate = joint.estimate(self.ctx, &self.upstream);
            let cached = estimate.note.as_deref() == Some(CACHED);
            let runs = makes
                .iter()
                .filter(|(kind, _)| reached.contains(kind) && !made(*kind) && self.runs(*kind))
                .count();
            (estimate.cost <= self.depth && (cached || runs >= 2)).then_some((joint, cached))
        })
    }

    /// Whether making `kind` on its own runs something: its producer's estimate is not `cached`.
    fn runs(&self, kind: FactKind) -> bool {
        self.chosen(kind).is_some_and(|producer| {
            producer.estimate(self.ctx, &self.upstream).note.as_deref() != Some(CACHED)
        })
    }

    /// A dry run's entries for the suite facts `reached` holds: one per joint run, naming its
    /// facts (`test-results+coverage+per-test-map(one run)`, or `(cached)`), then one per suite
    /// fact made on its own (`test-results(run)`, `coverage(cached)`); and the kinds the joint
    /// covers, whose own entries are left out.
    pub fn planned(&self, reached: &BTreeSet<FactKind>) -> (Vec<String>, BTreeSet<FactKind>) {
        let mut entries = Vec::new();
        let mut covered = BTreeSet::new();
        let none = |_: FactKind| false;
        let joint = reached
            .iter()
            .find_map(|kind| self.joint(*kind, reached, &none));
        if let Some((joint, cached)) = joint {
            let kinds: Vec<String> = joint
                .makes()
                .iter()
                .map(|(kind, _)| kind.to_string())
                .collect();
            let how = if cached { CACHED } else { "one run" };
            entries.push(format!("{}({how})", kinds.join("+")));
            covered.extend(joint.makes().iter().map(|(kind, _)| *kind));
        }
        for kind in SUITE {
            if !reached.contains(&kind) || covered.contains(&kind) {
                continue;
            }
            if let Some(producer) = self.chosen(kind) {
                let estimate = producer.estimate(self.ctx, &self.upstream);
                let how = if estimate.note.as_deref() == Some(CACHED) {
                    CACHED
                } else {
                    "run"
                };
                entries.push(format!("{kind}({how})"));
            }
        }
        (entries, covered)
    }
}
