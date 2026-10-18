// ABOUTME: The `ModelCheckResults` fact payload (ADR-117): every proof harness a bounded model checker found in a
// ABOUTME: language's projects, checked over all inputs up to its bounds, with the properties refuted and a counterexample.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactEntry, FactKind};
use crate::finding::Location;
use crate::id::ProducerId;
use crate::root_run::RootRun;

/// The proof harnesses of a language's projects, each model-checked once (ADR-117).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelCheckResults {
    /// The model checker, such as `tool.kani`.
    pub tool: ProducerId,
    /// Each project root it ran in, in `ProjectInfo` order.
    pub runs: Vec<RootRun>,
    /// Every harness it checked, by root, then name.
    pub harnesses: Vec<ModelHarness>,
}

impl Fact for ModelCheckResults {
    const KIND: FactKind = FactKind::ModelCheckResults;
    const VERSION: u32 = 1;
}

inventory::submit!(
    FactEntry::execution::<ModelCheckResults>("model-check-results").with_conformance()
);

/// One proof harness and what checking it found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelHarness {
    /// The project root it belongs to, relative to the repository root.
    pub root: String,
    /// The harness as the checker names it, such as `proofs::average_never_overflows`.
    pub name: String,
    /// Where the harness is defined, when found.
    pub location: Option<Location>,
    pub status: HarnessStatus,
    /// The properties the checker refuted: assertions, panics, overflows, out-of-bounds
    /// accesses, each where it fails.
    pub refuted: Vec<RefutedProperty>,
}

/// How checking a harness ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HarnessStatus {
    /// Every property holds for every input within the harness's bounds.
    Verified,
    /// Some input refutes a property.
    Failed,
    /// The checker could not decide: an unsupported construct, a bound too small to be sound.
    Undetermined,
}

/// A property a model checker refuted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RefutedProperty {
    /// What the property is, in the checker's words (`attempt to add with overflow`).
    pub description: String,
    /// The function it fails in, as the checker names it.
    pub function: Option<String>,
    /// Where it fails in the repository; `None` outside it (the standard library).
    pub location: Option<Location>,
    /// Inputs that refute it, when the checker gives them.
    pub counterexample: Option<Counterexample>,
}

/// Concrete inputs that refute a harness's property.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Counterexample {
    /// Each nondeterministic value the harness drew, in the order it drew them, as the
    /// checker renders them.
    pub values: Vec<String>,
    /// A unit test the checker wrote that replays them, when it writes one.
    pub replay: Option<String>,
}
