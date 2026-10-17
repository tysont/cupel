// ABOUTME: The `BenchSamples` fact payload (ADR-106): every benchmark a bench tool measured in a language's projects,
// ABOUTME: each with its samples and unit, and the same benchmarks measured at the range's base commit.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactKind};
use crate::id::ProducerId;
use crate::root_run::RootRun;

/// The benchmarks one bench tool measured at head and, under a range, at its base commit
/// (ADR-106). Every sample is a cost: lower is better.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BenchSamples {
    /// The bench tool, such as `tool.criterion` or `tool.go-bench`.
    pub tool: ProducerId,
    /// The samples asked of every benchmark on each side.
    pub samples: u32,
    /// Each project root it ran in at head, in `ProjectInfo` order.
    pub runs: Vec<RootRun>,
    /// Every benchmark measured at head, in the order the tool reported them.
    pub benchmarks: Vec<Benchmark>,
    /// The same roots measured at the range's base commit; `None` outside a range.
    pub base: Option<BenchBase>,
}

impl Fact for BenchSamples {
    const KIND: FactKind = FactKind::BenchSamples;
    const VERSION: u32 = 1;
}

/// One benchmark's samples of one measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Benchmark {
    /// The benchmark, unique within its side with its unit: a Go benchmark with its package
    /// (`example.com/units.BenchmarkSum`), a criterion ID, a pytest node ID, a hyperfine command.
    pub name: String,
    /// What one sample measures, per iteration of the benchmark: `ns` of wall time, `bytes`
    /// or `allocs` allocated.
    pub unit: String,
    /// Where the benchmark is defined, relative to the repository root, when the tool says.
    pub file: Option<String>,
    pub line: Option<u32>,
    pub samples: Vec<f64>,
}

impl Benchmark {
    /// What identifies the benchmark on both sides: its name and unit.
    pub fn key(&self) -> (&str, &str) {
        (&self.name, &self.unit)
    }
}

/// The benchmarks measured at the base commit of a range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BenchBase {
    pub commit: String,
    /// Each project root it ran in there.
    pub runs: Vec<RootRun>,
    pub benchmarks: Vec<Benchmark>,
}
