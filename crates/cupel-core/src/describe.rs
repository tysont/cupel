// ABOUTME: The `describe` payload: self-description of the CLI, its commands, flags, exit codes,
// ABOUTME: and schemas, carried in the envelope's `description` field (ADR-018).
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::check::{FindingKindSpec, Manifest, Stability};
use crate::config::ConfigSource;
use crate::fact::FactKind;
use crate::finding::ActionKind;
use crate::grade::CostClass;
use crate::id::{CheckId, LangFamily, LangId, ProducerId, ToolId};
use crate::matrix::Support;
use crate::status::ExitCode;

/// What `describe` returns, tagged by the kind of thing described.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "subject", rename_all = "kebab-case")]
pub enum Description {
    /// `cupel describe`: the whole command line at a glance.
    Cli(CliDescription),
    /// `cupel describe <verb>` or `cupel <verb> --describe`.
    Command(CommandDescription),
    /// `cupel describe schema <name>`.
    Schema(SchemaDescription),
    /// `cupel describe --all`: everything, in one document.
    All(AllDescription),
    /// `cupel describe language <id>`.
    Language(LanguageDescription),
    /// `cupel describe languages`.
    Languages(LanguagesDescription),
    /// `cupel describe fact <kind>`.
    Fact(FactDescription),
    /// `cupel describe matrix`.
    Matrix(MatrixDescription),
    /// `cupel describe <check-id>`.
    Check(CheckDescription),
    /// `cupel describe finding <kind>`.
    Finding(FindingDescription),
    /// `cupel describe --agent`: the guide an agent harness pastes into its instructions.
    Agent(AgentDescription),
}

/// Verbs, global flags, exit codes, schema URLs, and configuration precedence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CliDescription {
    pub name: String,
    pub version: String,
    pub summary: String,
    pub stability: Stability,
    pub verbs: Vec<CommandSummary>,
    /// Flags every verb accepts.
    pub global_flags: Vec<ArgDescription>,
    /// Flags accepted only without a verb, such as `--version`.
    pub flags: Vec<ArgDescription>,
    pub exit_codes: Vec<ExitCodeDescription>,
    pub schemas: Vec<SchemaRef>,
    /// Configuration layers, highest precedence first.
    pub config_precedence: Vec<ConfigSource>,
    /// The ways to install `cupel`, those available now first (ADR-128).
    pub install_routes: Vec<InstallRoute>,
}

/// One way to install `cupel`, and whether it installs a released binary today (ADR-128).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InstallRoute {
    /// Such as `Install script` or `Homebrew`.
    pub name: String,
    /// The command line that installs `cupel` this way.
    pub command: String,
    /// The operating systems it installs on, as Rust spells them (`linux`, `macos`, `windows`).
    pub platforms: Vec<String>,
    /// Whether the route works today.
    pub live: bool,
    /// What the route waits for, such as "once the `cupel` package is published to npm"; null
    /// when it is live.
    pub pending: Option<String>,
    pub stability: Stability,
}

/// One line about a verb; `describe <verb>` has the rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandSummary {
    pub name: String,
    pub summary: String,
    pub stability: Stability,
    /// The implementation-plan step that gives this verb its behavior; null once it has it.
    pub pending_step: Option<String>,
}

/// A verb or sub-verb with its own arguments (global flags are listed once, on the CLI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CommandDescription {
    /// The words that invoke it after `cupel`, such as `check` or `baseline set`.
    pub name: String,
    pub summary: String,
    pub stability: Stability,
    /// The implementation-plan step that gives this command its behavior; null once it has it.
    pub pending_step: Option<String>,
    pub arguments: Vec<ArgDescription>,
    pub subcommands: Vec<CommandDescription>,
    pub examples: Vec<Example>,
}

/// A flag or positional argument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArgDescription {
    /// `--name` for a flag, the value name (such as `FINDING`) for a positional.
    pub name: String,
    pub positional: bool,
    pub value: ValueType,
    /// The placeholder shown in help, such as `PATH`; null for switches.
    pub value_name: Option<String>,
    /// The accepted values of an `enum`; empty otherwise.
    pub values: Vec<String>,
    /// The value used when the argument is absent, if it is a fixed value.
    pub default: Option<String>,
    /// The environment variable read when the flag is absent.
    pub env: Option<String>,
    pub required: bool,
    /// Whether the argument accepts several values.
    pub repeated: bool,
    /// Flags that cannot be combined with this one.
    pub conflicts: Vec<String>,
    pub summary: String,
    pub stability: Stability,
    /// The implementation-plan step that gives this argument its behavior; null once it has it.
    /// Until then it parses and is otherwise ignored.
    pub pending_step: Option<String>,
}

/// The type of value an argument takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ValueType {
    /// A flag that takes no value.
    Switch,
    String,
    Integer,
    Path,
    /// A duration such as `10s`, `5m`, or `1h`.
    Duration,
    /// One of `values`.
    Enum,
}

/// An invocation and the exit code it produces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Example {
    pub command: String,
    pub summary: String,
    pub exit_code: ExitCode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExitCodeDescription {
    pub code: ExitCode,
    pub name: String,
    pub meaning: String,
    pub stability: Stability,
}

/// Where a published schema lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SchemaRef {
    pub name: String,
    pub url: String,
    pub stability: Stability,
}

/// A published schema and the schema itself (JSON Schema draft 2020-12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SchemaDescription {
    pub name: String,
    pub url: String,
    pub stability: Stability,
    pub schema: Map<String, Value>,
}

/// The agent guide: Markdown for `CLAUDE.md` or `AGENTS.md`, which `--format text` prints as is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgentDescription {
    pub markdown: String,
}

/// Everything `describe` knows, in one document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AllDescription {
    pub cli: CliDescription,
    /// Every verb, with its sub-verbs nested.
    pub commands: Vec<CommandDescription>,
    pub schemas: Vec<SchemaDescription>,
}

/// One language: identity, the facts it produces and by which producers, and which checks can
/// therefore run for it (design §8.4.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LanguageDescription {
    pub id: LangId,
    pub family: LangFamily,
    /// With the leading dot, such as `.rs`.
    pub extensions: Vec<String>,
    pub stability: Stability,
    /// Every fact kind the language has a producer for, in catalog order.
    pub facts: Vec<FactProducers>,
    /// Every registered check and whether it can run for this language.
    pub checks: Vec<CheckSupport>,
    /// The language's toolchain-table row (design §8.7); null when it has none.
    pub toolchain: Option<ToolchainDescription>,
}

/// A toolchain-table row: the tools Cupel knows for a language, each list in default order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ToolchainDescription {
    /// Package managers and other prefixes commands are launched through.
    pub launchers: Vec<LauncherDescription>,
    /// Test-runner producers (`TestResults`).
    pub runners: Vec<ToolDescription>,
    /// Coverage producers (`CoverageMap`), each with its report format.
    pub coverage: Vec<ToolDescription>,
    /// Benchmark producers (`BenchSamples`).
    pub bench: Vec<ToolDescription>,
    /// Type checkers and linters, run in this order for `AnalyzerReport` (ADR-102).
    pub analyzers: Vec<ToolDescription>,
    /// Property-testing tools.
    pub property: Vec<ToolId>,
    /// Fuzzers.
    pub fuzz: Vec<ToolId>,
    /// Race and undefined-behavior detectors.
    pub race: Vec<ToolId>,
}

/// A launcher: when it is chosen, and the values it gives `<name>` placeholders in commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LauncherDescription {
    pub tool: ToolId,
    /// Detection rules, any one of which selects it; empty for the fallback.
    pub detect: Vec<String>,
    pub values: BTreeMap<String, String>,
}

/// A tool producer in a toolchain row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ToolDescription {
    pub producer: ProducerId,
    /// The report format it reads, for coverage producers.
    pub format: Option<String>,
    /// Detection rules, any one of which selects it; empty for the fallback.
    pub detect: Vec<String>,
    /// Default command templates.
    pub commands: Vec<CommandTemplate>,
    /// The command template that installs it, with the launcher's values as placeholders;
    /// null when Cupel does not install it (ADR-081).
    pub install: Option<String>,
}

/// A default command and the runner it goes with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandTemplate {
    /// The runner this command wraps or accompanies; null when it fits every runner.
    pub runner: Option<ProducerId>,
    pub command: String,
}

/// The producers registered for one fact kind, in selection order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FactProducers {
    pub kind: FactKind,
    pub producers: Vec<ProducerDescription>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProducerDescription {
    pub id: ProducerId,
    pub cost: CostClass,
    /// Facts the producer consumes.
    pub needs: Vec<FactKind>,
}

/// One check: its manifest, and whether it can run for each registered language (design §6.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckDescription {
    pub manifest: Manifest,
    /// Every registered language and whether the check can run there, derived from its needs.
    pub languages: Vec<LanguageSupport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LanguageSupport {
    pub lang: LangId,
    pub support: SupportDescription,
}

/// One finding kind: the check that emits it, the action it asks for, and the JSON Schema of its
/// evidence (ADR-012).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FindingDescription {
    pub kind: String,
    pub check: CheckId,
    pub action: ActionKind,
    pub summary: String,
    /// JSON Schema of this kind's `evidence` object.
    pub evidence: Map<String, Value>,
    pub stability: Stability,
}

impl FindingDescription {
    /// `spec`, a finding kind of the check `manifest` describes.
    pub fn of(manifest: &Manifest, spec: &FindingKindSpec) -> Self {
        Self {
            kind: spec.kind.clone(),
            check: manifest.id.clone(),
            action: spec.action,
            summary: spec.summary.clone(),
            evidence: spec.evidence.clone(),
            stability: manifest.stability,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckSupport {
    pub check: CheckId,
    pub support: SupportDescription,
}

/// Every language, one line each.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LanguagesDescription {
    pub languages: Vec<LanguageSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LanguageSummary {
    pub id: LangId,
    pub family: LangFamily,
    pub extensions: Vec<String>,
    pub stability: Stability,
    /// The fact kinds the language has a producer for, in catalog order.
    pub facts: Vec<FactKind>,
}

/// One fact kind: its payload version and schema, its producers per language, and the checks
/// that consume it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FactDescription {
    pub kind: FactKind,
    /// The payload's `Fact::VERSION`; null until the payload type exists.
    pub version: Option<u32>,
    /// Whether language producers make it from the syntax tree.
    pub syntactic: bool,
    /// The payload's JSON Schema; null until the payload type exists.
    pub schema: Option<Map<String, Value>>,
    /// Every registered language, with its producers of this fact (empty when it has none).
    pub producers: Vec<LanguageProducers>,
    /// Checks that need or want this fact.
    pub consumers: Vec<CheckId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LanguageProducers {
    pub lang: LangId,
    pub producers: Vec<ProducerDescription>,
}

/// Checks × languages (design §8.3.5): derived from check needs and registered producers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MatrixDescription {
    pub languages: Vec<LangId>,
    pub checks: Vec<CheckId>,
    /// One cell per check and language, ordered by check, then language.
    pub cells: Vec<MatrixCell>,
    /// Verbs whose behavior depends on a fact a language may lack, such as `tests --impacted`
    /// on `per-test-map` (ADR-006): per language, whether that fact can be produced.
    pub verbs: Vec<VerbSupport>,
}

/// A verb that reads a fact directly, and per language whether the fact can be produced there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VerbSupport {
    /// Such as `tests --impacted`.
    pub verb: String,
    pub fact: FactKind,
    /// One per language, in ID order.
    pub languages: Vec<LanguageSupport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MatrixCell {
    pub check: CheckId,
    pub lang: LangId,
    pub support: SupportDescription,
}

/// A [`Support`] value as `describe` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum SupportDescription {
    Supported,
    /// No producer can make this fact for the language.
    Unsupported {
        fact: FactKind,
    },
    /// A producer's tool is missing here.
    Unavailable {
        tool: ToolId,
    },
    /// A producer needs this configuration key set.
    Unconfigured {
        key: String,
    },
}

impl From<&Support> for SupportDescription {
    fn from(support: &Support) -> Self {
        match support {
            Support::Supported => Self::Supported,
            Support::Unsupported(fact) => Self::Unsupported { fact: *fact },
            Support::Unavailable(tool) => Self::Unavailable { tool: tool.clone() },
            Support::Unconfigured(key) => Self::Unconfigured { key: key.clone() },
        }
    }
}
