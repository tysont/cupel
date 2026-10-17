// ABOUTME: The `AnalyzerReport` fact payload (ADR-102): each type checker and linter a language's analyzers producer ran,
// ABOUTME: with its status and its results normalized to SARIF's rule, level, message, and one location.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactKind};
use crate::finding::Location;
use crate::id::ToolId;

/// What a language's analyzers reported, one entry per analyzer in the toolchain table's order
/// (ADR-102).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzerReport {
    pub analyzers: Vec<AnalyzerRun>,
}

impl Fact for AnalyzerReport {
    const KIND: FactKind = FactKind::AnalyzerReport;
    const VERSION: u32 = 1;
}

/// One analyzer: whether it ran, and what it reported in the files in scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzerRun {
    /// The analyzer, such as `mypy` or `go-vet`.
    pub tool: ToolId,
    pub status: AnalyzerStatus,
    /// Missing: how to install it; unconfigured: what to add; failed: the tail of its output.
    pub detail: Option<String>,
    /// Its results in files in scope, relative to the repository root, in the order it reported
    /// them, without exact duplicates.
    pub results: Vec<AnalyzerResult>,
}

/// Whether an analyzer ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AnalyzerStatus {
    Ran,
    /// Its program is not installed.
    Missing,
    /// It needs configuration the project does not have, such as an `ESLint` flat config.
    Unconfigured,
    /// It ran but its output could not be read, or it crashed.
    Failed,
}

/// One result, as SARIF's `result`: a rule, a level, a message, and one physical location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AnalyzerResult {
    /// The tool's rule or error code, such as `F401`, `TS2322`, or `needless_return`.
    pub rule: String,
    pub level: AnalyzerLevel,
    pub message: String,
    pub location: Location,
}

/// SARIF's result levels (`none` is not reported).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AnalyzerLevel {
    Error,
    Warning,
    Note,
}
