// ABOUTME: The `TestReruns` fact payload (ADR-078): the baseline's tests rerun several times by the flake
// ABOUTME: engine, each rerun with the order, seed, and parallelism it used and every test's outcome.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactEntry, FactKind};
use crate::id::ProducerId;
use crate::test_results::{RunnerInvocation, TestResult};

/// The baseline's tests, rerun (ADR-078).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestReruns {
    /// The runner whose baseline was rerun, such as `tool.pytest`.
    pub runner: ProducerId,
    /// The run's seed; every rerun's seed derives from it, so the same seed repeats the reruns.
    pub seed: u64,
    /// Every rerun, in the order they ran.
    pub runs: Vec<TestRerun>,
}

impl Fact for TestReruns {
    const KIND: FactKind = FactKind::TestReruns;
    const VERSION: u32 = 1;
}

inventory::submit!(FactEntry::execution::<TestReruns>("test-reruns"));

/// One rerun of the suite: what it varied and what every test did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestRerun {
    pub order: RunOrder,
    /// The seed this rerun handed the runner: its shuffle seed, its hash seed.
    pub seed: u64,
    /// The worker or thread count asked for; `None` for the runner's default.
    pub parallelism: Option<u32>,
    /// Test IDs in the order they ran, when the runner reports it.
    pub executed: Option<Vec<String>>,
    /// One per project root, as in `TestResults`.
    pub invocations: Vec<RunnerInvocation>,
    /// Every test the runner reported, sorted by ID, with the baseline's IDs.
    pub tests: Vec<TestResult>,
}

/// The order a rerun asked the runner for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RunOrder {
    /// The runner's own order: as declared, or sorted by name.
    Declared,
    /// The declared order backwards, for runners that take an explicit order.
    Reversed,
    /// The runner's seeded shuffle, with this rerun's seed.
    Shuffled,
}

impl TestRerun {
    /// What two reruns compare to say whether they ran the tests in the same order: the tests
    /// in the order they ran, when the runner reported it; else the order asked for (with the
    /// seed, for a shuffle).
    pub fn order_key(&self) -> String {
        match (&self.executed, self.order) {
            (Some(executed), _) => executed.join("\n"),
            (None, RunOrder::Declared) => "declared".to_owned(),
            (None, RunOrder::Reversed) => "reversed".to_owned(),
            (None, RunOrder::Shuffled) => format!("shuffled:{}", self.seed),
        }
    }
}
