// ABOUTME: The `Traces` fact payload (ADR-116): the traces a test run recorded through Cupel's trace logging
// ABOUTME: convention, each validated as a behavior of the run's specification, with the first step it does not allow.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fact::{Fact, FactEntry, FactKind};
use crate::finding::Location;
use crate::id::ProducerId;

/// The traces a language's tests recorded, validated against the run's spec oracle (ADR-116).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Traces {
    /// The validator that ran, such as `tool.tlc`.
    pub tool: ProducerId,
    /// The specification the traces were validated against; `None` when the run has no spec
    /// oracle the validator reads.
    pub spec: Option<TraceSpec>,
    /// Why no trace was validated: no spec oracle, a spec in a language the validator does not
    /// read, a test run that did not complete; `None` when the recorded traces were validated.
    pub problem: Option<String>,
    /// Every trace the tests recorded, by name.
    pub traces: Vec<Trace>,
}

impl Fact for Traces {
    const KIND: FactKind = FactKind::Traces;
    const VERSION: u32 = 1;
}

inventory::submit!(FactEntry::execution::<Traces>("traces").with_conformance());

/// The spec oracle traces are validated against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TraceSpec {
    /// The oracle's name in `cupel.toml`.
    pub oracle: String,
    /// The specification file, relative to the repository root.
    pub file: String,
    /// Its language: `tla+`, `quint`, or `p`.
    pub lang: String,
}

/// One recorded trace and how its validation ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Trace {
    /// The trace file's name in the directory the tests recorded into.
    pub name: String,
    /// The states it records, the initial one included.
    pub steps: u64,
    pub status: TraceStatus,
    /// Why a failed validation failed: an unreadable trace, a spec the validator rejected.
    pub detail: Option<String>,
    /// The first step the spec does not allow, for a diverged trace.
    pub divergence: Option<TraceDivergence>,
}

/// How a trace's validation ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TraceStatus {
    /// Every step is one the spec allows: the trace is a behavior of the spec.
    Conforms,
    /// Some step is not: the trace leaves the spec's behaviors there.
    Diverged,
    /// It could not be validated.
    Failed,
}

/// The first step of a trace the spec does not allow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TraceDivergence {
    /// The step, counting the initial state as 1.
    pub step: u64,
    /// The spec action the implementation logged for the step, when it named one.
    pub action: Option<String>,
    /// The state the implementation logged at the step, as recorded.
    pub state: Value,
    /// The state at the step before, the last the spec allows; `None` at the initial state.
    pub previous: Option<Value>,
    /// Where in the repository the implementation logged the step, when it said.
    pub location: Option<Location>,
}
