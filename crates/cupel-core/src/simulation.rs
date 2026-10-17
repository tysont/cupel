// ABOUTME: The `SimulationResults` fact payload (ADR-120): every deterministic simulation harness a simulator found in
// ABOUTME: a language's projects, run over a range of seeds, with the first failing seed, its trace, and how to rerun it.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactKind};
use crate::finding::Location;
use crate::id::ProducerId;
use crate::root_run::RootRun;

/// The simulation harnesses of a language's projects, each run over the same range of seeds
/// (ADR-120).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SimulationResults {
    /// The simulator, such as `tool.madsim`.
    pub tool: ProducerId,
    /// The first seed of the range, the run's seed.
    pub seed: u64,
    /// How many seeds each harness was given: `seed` up to `seed + seeds - 1`. A harness stops at
    /// its first failing seed.
    pub seeds: u64,
    /// Each project root the simulator ran in.
    pub runs: Vec<RootRun>,
    /// Every harness found there, by root, then name.
    pub harnesses: Vec<SimulationHarness>,
}

impl Fact for SimulationResults {
    const KIND: FactKind = FactKind::SimulationResults;
    const VERSION: u32 = 1;
}

/// One simulation harness and how its seeds ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SimulationHarness {
    /// The project root it belongs to, as in `runs`.
    pub root: String,
    /// The harness as the test runner names it.
    pub name: String,
    /// Where it is defined, when known.
    pub location: Option<Location>,
    pub status: SimulationStatus,
    /// What failed, for a failed harness.
    pub failure: Option<SimulationFailure>,
}

/// How a harness's seeds ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SimulationStatus {
    /// Every seed of the range passed.
    Passed,
    /// A seed failed, or the harness failed outside any one seed.
    Failed,
}

/// The first failure of a harness, and how to see it again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SimulationFailure {
    /// The failing seed, when the simulator names it.
    pub seed: Option<u64>,
    /// What the harness reported: a panic's or an assertion's first line.
    pub message: String,
    /// Where it failed in the repository, when the failure names a place there.
    pub location: Option<Location>,
    /// What the harness printed for that seed, replayed alone, with thread numbers and paths
    /// outside the repository masked; at most 50 lines, the last ones.
    pub trace: Vec<String>,
    /// Whether replaying the seed alone failed again; a failure that does not repeat is a
    /// harness that is not deterministic.
    pub reproduced: bool,
    /// The command that runs that seed alone, from the project root.
    pub reproduce: String,
}
