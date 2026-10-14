// ABOUTME: The envelope: the single JSON document every verb writes to stdout (design §7.1).
// ABOUTME: Also holds run metadata, check results, the summary, gate results, and exit codes.
use std::borrow::Cow;
use std::collections::BTreeMap;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::describe::Description;
use crate::dry_run::RunPlan;
use crate::error::{Error, Result};
use crate::explanation::Explanation;
use crate::fact::FactKind;
use crate::finding::{ActionKind, Finding};
use crate::grade::{CostClass, Tier};
use crate::hash::FindingId;
use crate::id::{CheckId, LangId, RunId, ToolId};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ScopeMode {
    Tree,
    Changed,
    Range,
    Paths,
    Symbols,
    SinceBaseline,
}

/// Tool name → version for every toolchain component the run used.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct ToolchainInfo(pub BTreeMap<String, String>);

/// The effective configuration and the layers it came from.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ConfigEcho {
    pub effective: Map<String, Value>,
    pub sources: Vec<ConfigSource>,
}

/// A configuration layer, in precedence order (design §6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigSource {
    Flags,
    Env,
    Profile,
    #[serde(rename = "cupel.toml")]
    CupelToml,
    Defaults,
}

/// The baseline deltas were computed against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BaselineRef {
    pub id: RunId,
    pub name: String,
}

/// Overall outcome. `partial` and `error` are distinct from `fail`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Pass,
    Fail,
    Partial,
    Error,
    /// A `--dry-run` plan; nothing ran.
    Plan,
}

/// Process exit codes (design §6.6). Serialized as the integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum ExitCode {
    /// Ran fully; gate passed or no gate.
    Pass = 0,
    /// Ran fully; gate failed.
    Fail = 1,
    /// Bad flags or config; the envelope names the fix.
    Usage = 2,
    /// Missing tool, oracle, or toolchain; `doctor` findings attached.
    Environment = 3,
    /// Baseline test suite fails or build broken; scoring refused.
    Unevaluable = 4,
    /// Timeout or budget hit; the envelope has valid partial results.
    Partial = 5,
    /// Cupel bug; the envelope includes a bug-report payload.
    Internal = 6,
}

impl ExitCode {
    pub const ALL: [ExitCode; 7] = [
        Self::Pass,
        Self::Fail,
        Self::Usage,
        Self::Environment,
        Self::Unevaluable,
        Self::Partial,
        Self::Internal,
    ];

    pub fn code(self) -> u8 {
        self as u8
    }

    /// What the code means, as in the design's exit-code table.
    pub fn meaning(self) -> &'static str {
        match self {
            Self::Pass => "Ran fully; gate passed or no gate.",
            Self::Fail => "Ran fully; gate failed.",
            Self::Usage => "Bad flags or config; the envelope names the fix.",
            Self::Environment => "Missing tool, oracle, or toolchain; doctor findings attached.",
            Self::Unevaluable => "Baseline test suite fails or build broken; scoring refused.",
            Self::Partial => "Timeout or budget hit; the envelope has valid partial results.",
            Self::Internal => "Cupel bug; the envelope includes a bug-report payload.",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Usage => "usage",
            Self::Environment => "environment",
            Self::Unevaluable => "unevaluable",
            Self::Partial => "partial",
            Self::Internal => "internal",
        }
    }
}

impl TryFrom<u8> for ExitCode {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|code| code.code() == value)
            .ok_or(Error::OutOfRange {
                what: "exit code",
                value: value.into(),
                expected: "0 to 6",
            })
    }
}

impl From<ExitCode> for u8 {
    fn from(value: ExitCode) -> Self {
        value.code()
    }
}

impl JsonSchema for ExitCode {
    fn schema_name() -> Cow<'static, str> {
        "ExitCode".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "integer",
            "enum": [0, 1, 2, 3, 4, 5, 6],
            "description": "0 pass · 1 fail · 2 usage · 3 environment · 4 unevaluable · 5 partial · 6 internal"
        })
    }
}

/// One check's outcome within a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckResult {
    pub id: CheckId,
    pub status: CheckStatus,
    pub tier: Tier,
    pub cost: CostClass,
    pub score: Option<Score>,
    pub delta: Option<Delta>,
    /// Number of findings this check produced.
    pub findings: u64,
    pub duration_ms: u64,
    pub skipped: Option<Skipped>,
    /// The oracle a differential or trace check compared against.
    pub oracle: Option<String>,
}

/// A check's outcome (ADR-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    Pass,
    Fail,
    Partial,
    Error,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Score {
    pub value: f64,
    /// What `value` measures, such as `killed_ratio`.
    pub unit: String,
    /// How many targets the score covers.
    pub n: u64,
}

/// Change in score against a reference run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Delta {
    /// `baseline`, a git ref, or `run:<id>`.
    pub vs: String,
    pub value: f64,
}

/// Why a check did not run (design §8.3.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Skipped {
    pub reason: SkipReason,
    /// The missing fact, for `unsupported`.
    pub fact: Option<FactKind>,
    pub lang: Option<LangId>,
    /// The missing tool, for `unavailable`.
    pub tool: Option<ToolId>,
    /// A command that explains the reason.
    pub see: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SkipReason {
    Unsupported,
    Unavailable,
    Unconfigured,
    Budget,
    Timeout,
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
    /// The measured value; null when the metric was not produced.
    pub actual: Option<f64>,
    pub result: GateOutcome,
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
