// ABOUTME: Oracle identity and state (ADR-090 §4–5): the hash a verification is recorded under, and
// ABOUTME: whether a recorded verification still holds; plus how an oracle reads in the envelope (ADR-091).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::OracleConfig;
use crate::id::RunId;

/// What the identity hash covers; bumped when that changes, so older verifications lapse.
const IDENTITY_VERSION: u32 = 1;

/// The identity hash of the oracle `definition` resolving to `resolved` (the commit of a ref,
/// the BLAKE3 of a spec file, nothing for a command): BLAKE3 hex of canonical JSON.
pub fn oracle_identity(definition: &OracleConfig, resolved: Option<&str>) -> String {
    let canonical = json!({
        "version": IDENTITY_VERSION,
        "definition": definition,
        "resolved": resolved,
    });
    blake3::hash(canonical.to_string().as_bytes())
        .to_hex()
        .to_string()
}

/// Whether an oracle's recorded verification holds now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OracleState {
    /// A verification is recorded under the oracle's identity now.
    Verified,
    /// No verification is recorded.
    Unverified,
    /// A verification is recorded under another identity: the definition, the ref's commit, or
    /// the spec file changed since.
    Changed,
    /// The identity cannot be computed now: the ref or the spec file is gone.
    Unreachable,
}

impl OracleState {
    /// The state of an oracle whose identity is `now` (`None` when unreachable) and whose
    /// recorded verification has hash `recorded`.
    pub fn of(now: Option<&str>, recorded: Option<&str>) -> Self {
        match (now, recorded) {
            (None, _) => Self::Unreachable,
            (Some(_), None) => Self::Unverified,
            (Some(now), Some(recorded)) if now == recorded => Self::Verified,
            (Some(_), Some(_)) => Self::Changed,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Unverified => "unverified",
            Self::Changed => "changed",
            Self::Unreachable => "unreachable",
        }
    }
}

/// One oracle as the `oracle` verb reports it (ADR-091 §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OracleReport {
    pub name: String,
    pub definition: OracleConfig,
    pub state: OracleState,
    /// The identity hash now; null when the oracle cannot be reached.
    pub hash: Option<String>,
    /// When the recorded verification was made; null without one.
    pub verified_at: Option<String>,
    /// The `oracle verify` run that recorded the verification; null without one.
    pub verified_run: Option<RunId>,
}
