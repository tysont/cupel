// ABOUTME: The `PropertyResults` fact payload (ADR-095): every property test in scope, run by the property engine
// ABOUTME: with the run's seed and a case budget, with its outcome and the framework's shrunk counterexample.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactKind};
use crate::id::ProducerId;
use crate::syntax::Span;

/// The property tests in scope, run once with a seed and a case budget (ADR-095).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PropertyResults {
    /// The baseline's runner, which ran the properties, such as `tool.pytest`.
    pub runner: ProducerId,
    /// The run's seed, handed to every framework that takes one.
    pub seed: u64,
    /// The generated cases each property was asked to pass.
    pub cases: u32,
    /// The language's property frameworks, in the toolchain table's order.
    pub frameworks: Vec<String>,
    /// Every property test case in scope, by file, then line.
    pub properties: Vec<PropertyResult>,
}

impl Fact for PropertyResults {
    const KIND: FactKind = FactKind::PropertyResults;
    const VERSION: u32 = 1;
}

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
