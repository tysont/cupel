// ABOUTME: The `describe` payload: self-description of the CLI, its commands, flags, exit codes,
// ABOUTME: and schemas, carried in the envelope's `description` field (ADR-018).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::check::Stability;
use crate::envelope::{ConfigSource, ExitCode};

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
