// ABOUTME: The plugin protocol's wire types (design §8.6, ADR-087): the manifest `describe` prints, the request
// ABOUTME: `plan` and `run` read, and the events `run` prints. `cupel-plugin` runs plugins with them.
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::check::{Manifest, Plan, Stability};
use crate::fact::FactKind;
use crate::finding::Finding;
use crate::grade::CostClass;
use crate::id::{CheckId, LangId, ProducerId};
use crate::result::{CheckStatus, Score};
use crate::syntax::SymbolKey;

/// The protocol version every `RunRequest` carries.
pub const PROTOCOL_VERSION: u32 = 1;

/// What `<plugin> describe` prints: a check's manifest, as native checks declare it, or a
/// producer's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PluginManifest {
    Check(Manifest),
    Producer(ProducerManifest),
}

/// Everything Cupel knows about a producer plugin without running it (design §8.3.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProducerManifest {
    /// Starts with `plugin.` (ADR-088).
    pub id: ProducerId,
    /// Bumped when the producer's output changes for the same inputs.
    pub version: u32,
    pub provides: Provides,
    /// The languages it produces the fact for.
    pub languages: Vec<LangId>,
    /// Facts it cannot produce without, made first for the same language.
    pub needs: Vec<FactKind>,
    /// Facts it uses when they can be made.
    pub wants: Vec<FactKind>,
    pub cost: CostClass,
    pub stability: Stability,
    pub summary: String,
}

/// The fact kind a producer plugin makes, at the payload version it emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Provides {
    pub kind: FactKind,
    pub version: u32,
}

/// What `<plugin> plan` and `<plugin> run` read on standard input (ADR-089 §2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunRequest {
    /// [`PROTOCOL_VERSION`].
    pub protocol: u32,
    /// The repository root, absolute; the plugin's working directory.
    pub root: String,
    pub scope: RequestScope,
    /// The run's depth, for a producer; `null` for a check.
    pub depth: Option<CostClass>,
    /// Milliseconds left before the run is cancelled; `null` without a deadline.
    pub budget_ms: Option<u64>,
    /// The run's seed, for a producer; `null` for a check.
    pub seed: Option<u64>,
    /// A check's `[checks."<id>"]` slice with its language override merged, or a producer's
    /// `[lang.<id>]` table.
    pub config: Map<String, Value>,
    /// The JSON file holding each fact the plugin declared that the run produced, absolute;
    /// empty for `plan`.
    pub facts: BTreeMap<FactKind, String>,
    /// The one directory the plugin may write in, absolute.
    pub scratch: String,
    pub toolchain: Toolchain,
    /// Whether the plugin may write outside `scratch`.
    pub write: bool,
}

/// The files, and optionally the symbols, a plugin works over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RequestScope {
    /// Relative to the root, `/`-separated.
    pub files: Vec<String>,
    /// The selected symbols by where they start; `null` for every symbol in `files`.
    pub symbols: Option<Vec<SymbolKey>>,
}

/// What the plugin runs under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Toolchain {
    /// The `cupel` version running the plugin.
    pub cupel: String,
}

/// One line `<plugin> run` prints (ADR-089 §4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "ev")]
pub enum PluginEvent {
    /// A check plugin's finding, as design §7.2 shapes it.
    #[serde(rename = "finding")]
    Finding(Box<Finding>),
    /// A check plugin's progress (design §7.4).
    #[serde(rename = "progress")]
    Progress {
        id: CheckId,
        done: u64,
        total: u64,
        eta_ms: Option<u64>,
    },
    /// A check plugin's outcome; exactly one ends its run.
    #[serde(rename = "result")]
    Result(PluginResult),
    /// A producer plugin's fact; exactly one is its run's output.
    #[serde(rename = "fact")]
    Fact(PluginFact),
}

/// What a check plugin reports about its run; Cupel adds the rest of the check's result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PluginResult {
    /// `pass`, `fail`, `partial`, or `error`.
    pub status: CheckStatus,
    pub score: Option<Score>,
    /// The value of each `scope` metric the manifest declares.
    pub metrics: BTreeMap<String, f64>,
    pub notes: Vec<String>,
}

/// A producer plugin's fact: the kind and payload version it provides, and the payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PluginFact {
    pub kind: FactKind,
    pub version: u32,
    /// Valid under `cupel describe fact <kind>`'s schema.
    pub payload: Value,
}

/// Any message of the protocol: what `describe` prints, what `plan` and `run` read, what
/// `plan` prints, and one line of `run`'s output. The root of `describe schema plugin`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum PluginMessage {
    Manifest(PluginManifest),
    Request(RunRequest),
    Plan(Plan),
    Event(PluginEvent),
}
