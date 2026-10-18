// ABOUTME: The `RaceReports` fact payload (ADR-105): what a race detector or undefined-behavior checker reported
// ABOUTME: running a language's tests, each report with its kind, test, message, and the stacks inside the repository.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactEntry, FactKind};
use crate::id::ProducerId;
use crate::root_run::RootRun;

/// Data races and undefined behavior one detector found running a language's tests (ADR-105).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RaceReports {
    /// The detector that ran, such as `tool.go-race` or `tool.miri`.
    pub tool: ProducerId,
    /// Each project root it ran in, in `ProjectInfo` order.
    pub runs: Vec<RootRun>,
    /// Every report touching the scope, in the order the detector printed them, without
    /// duplicates.
    pub reports: Vec<RaceReport>,
}

impl Fact for RaceReports {
    const KIND: FactKind = FactKind::RaceReports;
    const VERSION: u32 = 1;
}

inventory::submit!(FactEntry::execution::<RaceReports>("race-reports").pinned_by("race"));

/// One report: a data race between two accesses, or one operation with undefined behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RaceReport {
    pub kind: RaceKind,
    /// The test that was running, as the test runner names it; `None` when the detector does not
    /// say.
    pub test: Option<String>,
    /// What the detector says happened, with addresses, thread and goroutine numbers, and
    /// allocation IDs masked, so the same defect reads the same on every run.
    pub message: String,
    /// The stacks the detector printed: each conflicting access of a data race, ordered by their
    /// first frame so the order the accesses happened in does not matter; where undefined
    /// behavior happened.
    pub stacks: Vec<RaceStack>,
}

/// What a report is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RaceKind {
    /// Two threads accessed the same memory without synchronization, one of them writing.
    DataRace,
    /// An operation the language leaves undefined: an out-of-bounds or dangling access, an
    /// invalid value, a violated aliasing rule.
    UndefinedBehavior,
}

/// One stack of a report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RaceStack {
    /// The access the stack made, such as `read` or `write`; `None` for the stack of undefined
    /// behavior.
    pub access: Option<String>,
    /// Its frames in files of the repository, innermost first; frames in the language's runtime
    /// and dependencies, whose paths differ between machines, are left out.
    pub frames: Vec<RaceFrame>,
}

/// One frame of a stack, in a file of the repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RaceFrame {
    /// The function, as the detector names it, without a compiler's hash suffix; `None` when the
    /// detector gives only a place (Miri's earlier access of a race).
    pub function: Option<String>,
    /// Relative to the repository root.
    pub file: String,
    pub line: u32,
}
