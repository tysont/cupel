// ABOUTME: The finding: one actionable observation with a stable ID, severity, evidence, and action.
// ABOUTME: Mirrors design §7.2; evidence is open JSON whose shape the emitting check declares (ADR-012).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::grade::Tier;
use crate::hash::{FindingId, SymbolHash};
use crate::id::{CheckId, LangId};

/// One actionable observation produced by a check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    pub id: FindingId,
    pub check: CheckId,
    /// Namespaced finding kind, such as `mutant.survived`.
    pub kind: String,
    pub severity: Severity,
    /// How sure the check is, from 0 to 1.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    pub tier: Tier,
    pub symbol: Option<Symbol>,
    pub location: Option<Location>,
    pub title: String,
    pub detail: Option<String>,
    pub evidence: Evidence,
    pub action: Action,
    /// A command that reproduces this finding.
    pub repro: String,
    /// A command that suppresses this finding.
    pub suppress: String,
    pub new_since_baseline: bool,
    pub suppressed: bool,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Fail,
    Warn,
    Info,
}

/// The function or method a finding is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Symbol {
    /// `namespace::name`, for people.
    pub display: String,
    /// Structural identity, for machines.
    pub hash: SymbolHash,
    pub lang: LangId,
}

/// A span in a file. Paths are relative to the repository root; lines and columns are 1-based.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Location {
    pub file: String,
    pub line: u32,
    pub col: Option<u32>,
    pub end_line: u32,
    pub end_col: Option<u32>,
}

/// Check-specific supporting data. Its shape per finding kind is the JSON Schema the emitting
/// check declares in its manifest (ADR-012).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Evidence(pub Map<String, Value>);

/// The concrete next step a finding asks for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Action {
    pub kind: ActionKind,
    pub priority: u32,
    pub summary: String,
    /// Kind-specific pointers, such as `nearest_test_file` or `covering_tests`, and the
    /// remediation hints of `crate::hint` (ADR-133).
    #[schemars(schema_with = "crate::hint::hints_schema")]
    pub hints: Map<String, Value>,
}

/// The closed set of things a finding can ask an agent to do (design §7.2).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    WriteTest,
    StrengthenAssertion,
    RestoreTest,
    RefactorExtract,
    ReduceComplexity,
    BreakCycle,
    FixBug,
    AddContract,
    AddProperty,
    InvestigateFlake,
    Rerun,
    Configure,
    InstallTool,
    VerifyOracle,
    /// Last resort when no mechanical action exists.
    Review,
}

impl ActionKind {
    pub const ALL: [ActionKind; 15] = [
        Self::WriteTest,
        Self::StrengthenAssertion,
        Self::RestoreTest,
        Self::RefactorExtract,
        Self::ReduceComplexity,
        Self::BreakCycle,
        Self::FixBug,
        Self::AddContract,
        Self::AddProperty,
        Self::InvestigateFlake,
        Self::Rerun,
        Self::Configure,
        Self::InstallTool,
        Self::VerifyOracle,
        Self::Review,
    ];
}
