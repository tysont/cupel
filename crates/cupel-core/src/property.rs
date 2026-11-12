// ABOUTME: The `PropertyResults` fact payload (ADR-095): every property test in scope, run by the property engine
// ABOUTME: with the run's seed and a case budget, with its outcome and the framework's shrunk counterexample.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactEntry, FactKind};
use crate::id::ProducerId;
use crate::syntax::Span;

/// The property tests in scope, run once with a seed and a case budget (ADR-095).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PropertyResults {
    /// The baseline's runner, which ran the properties, such as `tool.pytest`.
    pub runner: ProducerId,
    /// The run's seed, handed to every framework that takes one.
    pub seed: u64,
    /// The generated cases each property was asked to pass; a property that pins its own count
    /// runs that count instead (`PropertyResult::pinned`, ADR-183).
    pub cases: u32,
    /// The language's property frameworks, in the toolchain table's order.
    pub frameworks: Vec<String>,
    /// Every property test case in scope, by file, then line.
    pub properties: Vec<PropertyResult>,
}

impl Fact for PropertyResults {
    const KIND: FactKind = FactKind::PropertyResults;
    const VERSION: u32 = 2;
}

inventory::submit!(FactEntry::execution::<PropertyResults>("property-results"));

/// One property test and what the run said about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PropertyResult {
    /// The test file, relative to the repository root.
    pub file: String,
    /// The case's name, as `TestFrames` gives it.
    pub name: String,
    pub span: Span,
    /// The framework that generates its inputs, such as `proptest` or `hypothesis`.
    pub framework: String,
    /// The runner's ID of the test; `None` when no test the runner reported is this case.
    pub test: Option<String>,
    pub outcome: PropertyOutcome,
    /// The framework's shrunk failing input, as it printed it.
    pub counterexample: Option<String>,
    /// Whether the framework took the run's seed (quickcheck takes none).
    pub seeded: bool,
    /// The case count the test sets itself, which wins over the run's budget (ADR-183); `None`
    /// when it sets none.
    pub pinned: Option<Pinned>,
}

impl PropertyResult {
    /// The cases the property ran at under a run whose budget is `budget`: its own pinned count,
    /// else the budget; `None` when it pins a count that is not a literal.
    pub fn cases(&self, budget: u32) -> Option<u32> {
        self.pinned.map_or(Some(budget), |pinned| pinned.cases)
    }
}

/// A case count a property test sets itself, such as fast-check's `{ numRuns: 120 }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Pinned {
    /// The count, when the test writes it as a literal; `None` when a variable or an expression
    /// gives it.
    pub cases: Option<u32>,
}

/// How a property test ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PropertyOutcome {
    Passed,
    /// Failed, and the framework reported the input that falsifies it.
    Falsified,
    /// Failed without a counterexample: an error before any case ran, or a crash.
    Failed,
    Skipped,
    /// No test the runner reported is this case.
    NotRun,
}

impl PropertyOutcome {
    /// Whether the property did not hold under the budget.
    pub fn is_failure(self) -> bool {
        matches!(self, Self::Falsified | Self::Failed)
    }
}
