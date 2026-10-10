// ABOUTME: The `describe` payload: self-description of the CLI, its commands, flags, exit codes,
// ABOUTME: and schemas, carried in the envelope's `description` field (ADR-018).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::check::Stability;
use crate::envelope::{ConfigSource, ExitCode};
use crate::fact::FactKind;
use crate::grade::CostClass;
use crate::id::{CheckId, LangFamily, LangId, ProducerId, ToolId};
use crate::matrix::Support;

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
