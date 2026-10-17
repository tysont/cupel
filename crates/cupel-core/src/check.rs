// ABOUTME: The check contract: a manifest of what a check needs and emits, plus plan and run.
// ABOUTME: Checks compute over facts only; they never see a language, file system, or process.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::cancel::Cancellation;
use crate::error::{Error, Result};
use crate::event::EventSink;
use crate::fact::FactKind;
use crate::facts::Facts;
use crate::finding::ActionKind;
use crate::grade::{CostClass, Tier};
use crate::id::CheckId;
use crate::rank::MetricPer;
use crate::result::{CheckResult, Score};
use crate::scope::Scope;
use crate::writer::Writer;

/// Everything Cupel knows about a check without running it. Plugins describe themselves with
/// the same shape. Language applicability is never declared; it is derived (design §8.3.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Manifest {
    pub id: CheckId,
    /// Bumped when results change for the same facts; invalidates cached results.
    pub version: u32,
    pub tier: Tier,
    pub cost: CostClass,
    /// Facts the check cannot run without.
    pub needs: Vec<FactKind>,
    /// Facts the check uses for enrichment when present.
    pub wants: Vec<FactKind>,
    pub findings: Vec<FindingKindSpec>,
    /// What the check measures: `scope` metrics are gate keys (`<check-id>.<metric>`), the others
    /// are `survey` ranking dimensions (ADR-060, ADR-062).
    pub metrics: Vec<MetricSpec>,
    pub stability: Stability,
    pub summary: String,
}

/// A finding kind a check emits, the action it maps to, and its evidence schema (ADR-012).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FindingKindSpec {
    pub kind: String,
    pub action: ActionKind,
    pub summary: String,
    /// JSON Schema of this kind's `evidence` object.
    pub evidence: Map<String, Value>,
}

impl FindingKindSpec {
    /// Whether a finding of `kind` is of this spec: the same kind, or, for a *template* such as
    /// `analyze.<tool>.<rule>`, each `<…>` segment standing for one non-empty segment of `kind`
    /// and a final one for one or more (ADR-102 §3).
    pub fn matches(&self, kind: &str) -> bool {
        if self.kind == kind {
            return true;
        }
        let pattern: Vec<&str> = self.kind.split('.').collect();
        let segments: Vec<&str> = kind.split('.').collect();
        if segments.iter().any(|segment| segment.is_empty()) {
            return false;
        }
        let placeholder = |segment: &str| segment.starts_with('<') && segment.ends_with('>');
        let Some((last, fixed)) = pattern.split_last() else {
            return false;
        };
        let rest_ok = if placeholder(last) {
            segments.len() > fixed.len()
        } else {
            segments.len() == pattern.len() && segments.last() == Some(last)
        };
        rest_ok
            && fixed
                .iter()
                .zip(&segments)
                .all(|(expected, actual)| placeholder(expected) || expected == actual)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MetricSpec {
    pub name: String,
    pub summary: String,
    /// What one value describes (ADR-062).
    pub per: MetricPer,
}

/// Whether a described entity's contract may still change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Stability {
    Stable,
    Experimental,
}

/// A check's planned work for a scope; also what `--dry-run` reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Plan {
    pub targets: u64,
    pub est_ms: u64,
}

impl Plan {
    /// `targets` units of work estimated from the cost class (ADR-041).
    pub fn estimate(cost: CostClass, targets: u64) -> Self {
        Self {
            targets,
            est_ms: targets.saturating_mul(cost.est_ms_per_target()),
        }
    }
}

/// What a check can see besides its facts. Grows store access in P2.2. It offers no file-system
/// or process access beyond a writer confined to the run's scratch directory.
#[derive(Debug, Clone)]
pub struct CheckCtx {
    /// What the run considers for this check's language: the files and the symbols in scope.
    pub scope: Scope,
    /// This check's resolved configuration, per-language overrides merged.
    pub config: Map<String, Value>,
    /// Writes only inside `.cupel/work/<run>/` (`Writer::scratch`).
    pub writer: Writer,
    /// Fires on `--timeout`; poll it between targets.
    pub cancel: Cancellation,
}

/// One verification technique.
pub trait Check: Send + Sync {
    fn manifest(&self) -> &Manifest;
    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan>;
    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        plan: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult>;

    /// Whether the check compares symbols across the languages of a family (design §8.3.5):
    /// it then runs once per family in scope, and `Facts::family` serves every member
    /// language's facts (ADR-051). The default is once per language.
    fn spans_family(&self) -> bool {
        false
    }

    /// The kinds this check also reads as they were at a range's base commit, through
    /// `Facts::base` (ADR-056). Each must also be in `needs`. A check that declares any runs
    /// only under a range scope and is skipped with `needs-range` otherwise.
    fn base_needs(&self) -> &[FactKind] {
        &[]
    }

    /// Whether the check compares against the run's oracle (ADR-091 §1): its results name the
    /// oracle, and a result against an oracle that is not verified is capped at tier 3.
    fn uses_oracle(&self) -> bool {
        false
    }

    /// One score from this check's per-language scores, for the envelope's single entry per
    /// check (ADR-041). The default is the mean weighted by each score's `n`.
    #[allow(
        clippy::cast_precision_loss,
        reason = "target counts stay far below 2^52, where f64 is exact"
    )]
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        let first = scores.first()?;
        let n: u64 = scores.iter().map(|score| score.n).sum();
        let weighted: f64 = scores
            .iter()
            .map(|score| score.value * score.n as f64)
            .sum();
        Some(Score {
            value: if n == 0 { 0.0 } else { weighted / n as f64 },
            unit: first.unit.clone(),
            n,
        })
    }

    /// One value of the `scope` metric `name` from the per-language results that reported it,
    /// for the check's one entry (ADR-060). Each result carries its whole score and metric map,
    /// so a ratio can be weighted by the count it is a ratio of (ADR-063). The default is the
    /// largest value, since metrics are bounds over a scope; a check whose metric is a ratio or
    /// a count overrides this.
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        results
            .iter()
            .filter_map(|result| result.metrics.get(name).copied())
            .reduce(f64::max)
    }
}

/// Every registered check, in registration order, unique by ID.
#[derive(Default)]
pub struct CheckRegistry {
    checks: Vec<Box<dyn Check>>,
}

impl CheckRegistry {
    pub fn register(&mut self, check: Box<dyn Check>) -> Result<()> {
        let id = &check.manifest().id;
        if self.get(id).is_some() {
            return Err(Error::DuplicateCheck { id: id.to_string() });
        }
        self.checks.push(check);
        Ok(())
    }

    pub fn get(&self, id: &CheckId) -> Option<&dyn Check> {
        self.checks
            .iter()
            .map(AsRef::as_ref)
            .find(|check| check.manifest().id == *id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn Check> {
        self.checks.iter().map(AsRef::as_ref)
    }
}

impl Facts {
    /// Facts declaring exactly what `manifest` needs and wants, holding none yet.
    pub fn for_manifest(manifest: &Manifest) -> Self {
        Self::declaring(manifest.needs.iter().chain(&manifest.wants).copied())
    }
}
