// ABOUTME: The `FuzzResults` fact payload (ADR-107): every fuzz target a fuzzer found in a language's projects, run
// ABOUTME: with the run's seed and an execution budget, with how it ended and the crash input that reproduces a failure.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactEntry, FactKind};
use crate::finding::Location;
use crate::id::ProducerId;

/// The fuzz targets of a language's projects, each fuzzed once with an execution budget
/// (ADR-107).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FuzzResults {
    /// The fuzzer, such as `tool.cargo-fuzz` or `tool.go-fuzz`.
    pub tool: ProducerId,
    /// The run's seed, handed to the fuzzer when it takes one.
    pub seed: u64,
    /// Whether the fuzzer took the seed (Go's does not).
    pub seeded: bool,
    /// The executions each target was given.
    pub cases: u64,
    /// Every project root searched for targets, relative to the repository root.
    pub roots: Vec<String>,
    /// Every target found there, by root, then name.
    pub targets: Vec<FuzzTarget>,
}

impl Fact for FuzzResults {
    const KIND: FactKind = FactKind::FuzzResults;
    const VERSION: u32 = 1;
}

inventory::submit!(FactEntry::execution::<FuzzResults>("fuzz-results"));

/// One fuzz target and how its run ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FuzzTarget {
    /// The project root it belongs to, as in `roots`.
    pub root: String,
    /// The target as the fuzzer names it: a cargo-fuzz binary, a Go `FuzzXxx` function with its
    /// package, a fuzz module.
    pub name: String,
    /// Where the target is defined, when known.
    pub location: Option<Location>,
    pub status: FuzzStatus,
    /// Why a failed run failed: the tail of its output.
    pub detail: Option<String>,
    /// Inputs the target ran on, when the fuzzer reports it.
    pub executions: Option<u64>,
    /// The coverage the fuzzer reached, in its own unit (libFuzzer's `cov`), when it reports it.
    pub coverage: Option<u64>,
    /// The input that crashed or hung the target.
    pub crash: Option<FuzzCrash>,
}

/// How a fuzz target's run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FuzzStatus {
    /// It ran its budget without failing.
    Passed,
    /// An input crashed it: a panic, an uncaught exception, a sanitizer error, running out of
    /// memory.
    Crashed,
    /// An input ran past the fuzzer's per-input time limit.
    TimedOut,
    /// The target could not be fuzzed: it did not build, or the fuzzer stopped without saying why.
    Failed,
}

/// An input that crashed or hung a target, and how to run it again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FuzzCrash {
    /// What the target reported, with addresses and thread numbers masked.
    pub message: String,
    /// Where it failed in the repository, when the failure names a place there.
    pub location: Option<Location>,
    /// The input, as text when it is printable UTF-8, else `hex:` and its bytes; at most 256
    /// characters of it.
    pub input: String,
    /// The file holding the whole input, relative to the repository root.
    pub reproducer: Option<String>,
    /// The command that runs the target on it, from the project root.
    pub reproduce: Option<String>,
}
