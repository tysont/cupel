// ABOUTME: One project root an execution tool ran in and whether its run completed, as the `RaceReports` and
// ABOUTME: `BenchSamples` payloads record it (ADR-105, ADR-106), so a check can tell "nothing found" from "did not run".
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A project root a tool ran in, and how its run ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RootRun {
    /// The project root, relative to the repository root; `.` for the root itself.
    pub dir: String,
    pub status: RunStatus,
    /// Why a failed run failed: the tail of its output.
    pub detail: Option<String>,
}

/// Whether a tool's run in a root completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RunStatus {
    /// It ran to the end, whatever it found.
    Ran,
    /// It could not run to the end: a build failure, a crash of the tool, output it cannot read.
    Failed,
}
