// ABOUTME: The check contract: a manifest of what a check needs and emits, plus plan and run.
// ABOUTME: Checks compute over facts only; they never see a language, file system, or process.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::cancel::Cancellation;
use crate::envelope::{CheckResult, Score};
use crate::error::{Error, Result};
use crate::event::EventSink;
use crate::fact::{FactKind, Facts};
use crate::finding::ActionKind;
use crate::grade::{CostClass, Tier};
use crate::id::CheckId;
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
    /// Metrics usable as gate keys (`<check-id>.<metric>`).
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MetricSpec {
    pub name: String,
    pub summary: String,
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
            return Err(Error::DuplicateCheck { id: id.clone() });
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
