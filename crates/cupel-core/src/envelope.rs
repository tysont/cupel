// ABOUTME: The envelope: the single JSON document every verb writes to stdout (design §7.1).
// ABOUTME: Holds run metadata, the summary, and gate results; check results and exit codes live below it.
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::ConfigEcho;
use crate::describe::Description;
use crate::dry_run::RunPlan;
use crate::explanation::Explanation;
use crate::finding::{ActionKind, Finding, Severity};
use crate::grade::CostClass;
use crate::hash::FindingId;
use crate::id::{LangId, RunId};
use crate::oracle::OracleReport;
use crate::per_test_impact::TestsReport;
use crate::rank::Rankings;
pub use crate::result::{CheckResult, CheckStatus, Delta, Score, SkipReason, Skipped};
pub use crate::scope::ScopeMode;
pub use crate::status::{ExitCode, Status};

/// The single output shape of every verb.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Envelope {
    pub cupel: CupelInfo,
    pub run: RunInfo,
    pub status: Status,
    pub exit_code: ExitCode,
    pub checks: Vec<CheckResult>,
    pub findings: Vec<Finding>,
    pub summary: Summary,
    pub next_actions: Vec<NextAction>,
    /// What `describe` returns; null for every other verb (ADR-018).
    pub description: Option<Description>,
    /// What a `--dry-run` would do; null unless `status` is `plan` (ADR-020).
    pub plan: Option<RunPlan>,
    /// What `explain` adds to the finding or check result it returns; null for every other verb.
    pub explanation: Option<Explanation>,
    /// What `survey` ranks: the worst targets per dimension; null for every other verb (ADR-062).
    pub rankings: Option<Rankings>,
    /// What `tests --impacted` selects: the impacted tests and how to run them; null for every
    /// other verb (ADR-077).
    pub tests: Option<TestsReport>,
    /// What the `oracle` verb reports: the oracles it listed, added, removed, or verified, with
    /// their state; null for every other verb (ADR-091 §6).
    pub oracles: Option<Vec<OracleReport>>,
    /// True when paging (`--limit`/`--offset`) left findings out.
    pub truncated: bool,
    pub total_findings: u64,
}

/// The producing binary's version and the schema the envelope conforms to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CupelInfo {
    pub version: String,
    pub schema: String,
}

/// What ran, when, where, and with which settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunInfo {
    pub id: RunId,
    /// Null only when the command line named no valid verb (ADR-018).
    pub verb: Option<Verb>,
    /// Start time, RFC 3339 in UTC (ADR-013).
    pub started: String,
    pub duration_ms: u64,
    /// Null outside a git repository.
    pub git: Option<GitInfo>,
    /// Null for verbs that do not resolve a scope.
    pub scope: Option<ScopeInfo>,
    /// The effective depth from configuration; null only when configuration did not load.
    pub depth: Option<CostClass>,
    /// The effective budget in milliseconds; null when no layer set one.
    pub budget_ms: Option<u64>,
    pub seed: u64,
    pub toolchain: ToolchainInfo,
    pub config: ConfigEcho,
    pub baseline: Option<BaselineRef>,
}

/// The verbs of design §6.2.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Verb {
    Init,
    Doctor,
    Describe,
    Check,
    Survey,
    Explain,
    Baseline,
    Oracle,
    Tests,
    Suppress,
    Serve,
}

impl Verb {
    pub const ALL: [Verb; 11] = [
        Self::Init,
        Self::Doctor,
        Self::Describe,
        Self::Check,
        Self::Survey,
        Self::Explain,
        Self::Baseline,
        Self::Oracle,
        Self::Tests,
        Self::Suppress,
        Self::Serve,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GitInfo {
    pub sha: String,
    pub dirty: bool,
    pub base: Option<String>,
    pub branch: Option<String>,
}

/// The set of files and symbols a run considered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScopeInfo {
    pub mode: ScopeMode,
    pub files: u64,
    pub symbols: u64,
    pub roots: Vec<String>,
    /// Languages that saw at least one file in scope.
    pub languages: Vec<LangId>,
}

/// Tool name → version for every toolchain component the run used.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct ToolchainInfo(pub BTreeMap<String, String>);

/// The baseline deltas were computed against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BaselineRef {
    pub id: RunId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Summary {
    pub findings: SeverityCounts,
    pub new: u64,
    pub resolved: u64,
    pub suppressed: u64,
    /// Null when no gate was evaluated.
    pub gate: Option<GateResult>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SeverityCounts {
    pub fail: u64,
    pub warn: u64,
    pub info: u64,
}

impl SeverityCounts {
    /// How many of `findings` have each severity.
    pub fn of(findings: &[Finding]) -> Self {
        let mut counts = Self::default();
        for finding in findings {
            match finding.severity {
                Severity::Fail => counts.fail += 1,
                Severity::Warn => counts.warn += 1,
                Severity::Info => counts.info += 1,
            }
        }
        counts
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GateResult {
    pub result: GateOutcome,
    /// The gate section evaluated, such as `gate.changed`.
    pub evaluated: String,
    pub rules: Vec<GateRule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GateRule {
    /// The rule as written, such as `tests.mutation.score >= 0.80`.
    pub rule: String,
    /// The measured value; null when the rule could not be evaluated.
    pub actual: Option<f64>,
    /// The value `actual` was compared with: the threshold, the reference value plus the
    /// offset for a rule against `base` (ADR-130), or 0 for a presence rule; null when the
    /// rule was not evaluated.
    pub bound: Option<f64>,
    pub result: RuleOutcome,
    /// Why a `skipped` rule was not evaluated, such as `no reference: no baseline is set`;
    /// null otherwise.
    pub reason: Option<String>,
}

/// One gate rule's result (ADR-060 §6). Only `pass` passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RuleOutcome {
    Pass,
    Fail,
    /// The rule's check did not run to completion here, or did not report the metric.
    Skipped,
    /// The rule's operator cannot be evaluated yet (`no-breaking`).
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum GateOutcome {
    Pass,
    Fail,
}

/// A ranked suggestion of what to do next.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NextAction {
    pub priority: u32,
    pub finding: Option<FindingId>,
    pub kind: ActionKind,
    pub summary: String,
    pub command: Option<String>,
}
