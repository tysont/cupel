// ABOUTME: The `DivergenceResults` fact payload (ADR-093): the differential engine's comparison of configured
// ABOUTME: entry points with the run's oracle on generated inputs, and each entry's minimized divergence.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::OracleKind;
use crate::fact::{Fact, FactKind};
use crate::id::ProducerId;
use crate::syntax::SymbolKey;

/// Entry points compared with an oracle on generated inputs (ADR-093 §1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DivergenceResults {
    /// The producer that compared them, such as `engine.differential`.
    pub engine: ProducerId,
    /// The oracle compared against; `None` when the run has none.
    pub oracle: Option<ComparedOracle>,
    /// The run's seed, from which every entry's inputs derive.
    pub seed: u64,
    /// Generated inputs per entry, besides an oracle's known inputs.
    pub cases: u64,
    /// One per configured entry point in scope, in configuration order.
    pub entries: Vec<EntryComparison>,
}

impl Fact for DivergenceResults {
    const KIND: FactKind = FactKind::DivergenceResults;
    const VERSION: u32 = 1;
}

/// The oracle a comparison ran against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComparedOracle {
    pub name: String,
    pub kind: OracleKind,
    /// The commit a `ref` oracle resolved to.
    pub commit: Option<String>,
}

/// One entry point's comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EntryComparison {
    /// The entry as configured: `<file>:<function>`.
    pub entry: String,
    /// The function the entry names, when it resolved.
    pub symbol: Option<SymbolKey>,
    /// Inputs both sides answered.
    pub compared: u64,
    /// Why the entry was not compared; `None` when it was.
    pub skipped: Option<String>,
    /// The first divergence, minimized; `None` when every input agreed.
    pub divergence: Option<Divergence>,
}

/// An input on which the implementation and the oracle answered differently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Divergence {
    /// The minimized arguments.
    pub input: Vec<Value>,
    /// The arguments as first generated.
    pub found: Vec<Value>,
    /// The index of `found` among the entry's inputs.
    pub case: u64,
    /// Minimization steps taken from `found` to `input`.
    pub shrinks: u32,
    /// What the working tree answered on `input`.
    pub implementation: Answer,
    /// What the oracle answered on `input`.
    pub oracle: Answer,
}

/// How one call ended, with its text: a value's rendering, an error, or why there was no answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Answer {
    Returned(String),
    Raised(String),
    Crashed(String),
}

impl Answer {
    /// The answer's text, whatever its kind.
    pub fn text(&self) -> &str {
        match self {
            Self::Returned(text) | Self::Raised(text) | Self::Crashed(text) => text,
        }
    }
}
