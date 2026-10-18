// ABOUTME: The `Histories` fact payload (ADR-119): the operation histories a project's history command recorded, each
// ABOUTME: checked against the model it declares, with the operations no linearization explains when it is violated.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fact::{Fact, FactEntry, FactKind};
use crate::id::ProducerId;
use crate::root_run::RootRun;

/// The histories one run of a language's history command wrote, each checked by one checker
/// (ADR-119).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Histories {
    /// The checker, such as `tool.porcupine`.
    pub tool: ProducerId,
    /// The command that recorded the histories (`[lang.<id>] history_command`).
    pub command: String,
    /// The seed the command was given (`CUPEL_SEED`), so a randomized harness repeats.
    pub seed: u64,
    /// The command's run from the repository root.
    pub run: RootRun,
    /// Every history the command wrote, by name.
    pub histories: Vec<History>,
}

impl Fact for Histories {
    const KIND: FactKind = FactKind::Histories;
    const VERSION: u32 = 1;
}

inventory::submit!(FactEntry::execution::<Histories>("histories").with_conformance());

/// One recorded history and what checking it against its model found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct History {
    /// The history file, relative to the directory the command wrote it in.
    pub name: String,
    /// The model the history declares, such as `register` or `kv`; `None` when it declares none.
    pub model: Option<String>,
    /// The operations it holds: invocations that completed or whose outcome is unknown.
    pub operations: u64,
    pub status: HistoryStatus,
    /// Why a history is invalid or its check unknown.
    pub detail: Option<String>,
    /// What a violated history's checker found, one entry per independent part (a key of a
    /// key-value store) that no linearization explains.
    pub violations: Vec<HistoryViolation>,
}

/// How checking a history ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HistoryStatus {
    /// Some order of its operations, consistent with real time, is a legal run of the model.
    Linearizable,
    /// No such order exists.
    Violated,
    /// The checker ran out of time before deciding.
    Unknown,
    /// The history cannot be checked: it does not parse, declares no model or one the checker
    /// does not know, or pairs a completion with no invocation.
    Invalid,
}

/// One independent part of a history that no linearization explains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HistoryViolation {
    /// The key of a key-value history; `None` for a single object.
    pub key: Option<Value>,
    /// The anomaly as the checker names it: `non-linearizable` for a linearizability checker.
    pub anomaly: String,
    /// The states the model can be in after the longest linearizable prefix the checker found;
    /// empty when it reports none.
    pub states: Vec<Value>,
    /// The last operations of that prefix, in their linearized order (at most three).
    pub linearized: Vec<HistoryOp>,
    /// The operations that could come next in real time, none of which the model allows from
    /// those states.
    pub stuck: Vec<HistoryOp>,
}

/// One operation of a history, as the excerpt of a violation shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HistoryOp {
    /// The process (client) that invoked it.
    pub process: Value,
    /// The function, such as `read`, `write`, or `cas`.
    pub f: String,
    /// The key of a key-value operation.
    pub key: Option<Value>,
    /// What it wrote or compared (`write`, `cas`), or what it read when it completed.
    pub value: Value,
    pub outcome: OpOutcome,
    /// The line of the history file that invoked it.
    pub line: u32,
}

/// Whether an operation's completion is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OpOutcome {
    /// It completed and took effect.
    Ok,
    /// It may or may not have taken effect: it timed out, its process crashed, or the history
    /// ends before it completes.
    Info,
}
