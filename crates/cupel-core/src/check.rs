// ABOUTME: The check contract: a manifest of what a check needs and emits, plus plan and run.
// ABOUTME: Checks compute over facts only; they never see a language, file system, or process.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::envelope::CheckResult;
use crate::error::{Error, Result};
use crate::event::EventSink;
use crate::fact::{FactKind, Facts};
use crate::finding::ActionKind;
use crate::grade::{CostClass, Tier};
use crate::id::CheckId;
use crate::scope::Scope;

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

/// What a check can see besides its facts. Grows store access, cancellation, and a scratch
/// directory as those land (P0.7, P2.2); it never offers file-system or process access.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CheckCtx {
    /// This check's resolved configuration, per-language overrides merged.
    pub config: Map<String, Value>,
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
