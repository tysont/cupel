// ABOUTME: The `ProofResults` fact payload (ADR-118): what a deductive verifier proved in a language's projects, and each
// ABOUTME: proof obligation it could not discharge (refuted or undecided) or that the proof assumes instead of proving.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactKind};
use crate::finding::Location;
use crate::id::ProducerId;
use crate::root_run::RootRun;

/// A deductive verifier's run over the proofs of a language's projects (ADR-118).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProofResults {
    /// The verifier, such as `tool.dafny`.
    pub tool: ProducerId,
    /// Each project root it ran in, in `ProjectInfo` order.
    pub runs: Vec<RootRun>,
    /// The proof files it verified, relative to the repository root, sorted.
    pub files: Vec<String>,
    /// The declarations it verified completely.
    pub verified: u64,
    /// Every obligation not discharged, in the order the verifier reported them.
    pub obligations: Vec<Obligation>,
}

impl Fact for ProofResults {
    const KIND: FactKind = FactKind::ProofResults;
    const VERSION: u32 = 1;
}

/// A proof obligation that is not discharged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Obligation {
    pub status: ObligationStatus,
    /// What the verifier says, in its words.
    pub message: String,
    /// Where the obligation is, in the repository.
    pub location: Location,
    /// The places the verifier relates to it, such as the postcondition a return path breaks.
    pub related: Vec<RelatedPlace>,
}

/// Why an obligation is not discharged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ObligationStatus {
    /// The verifier could not prove it: the code or the proof is wrong.
    Failed,
    /// It is not decided: assumed rather than proved (an `assume`, a lemma without a body), or
    /// the verifier ran out of time or resources on it.
    Open,
}

/// A place a verifier relates to an obligation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelatedPlace {
    pub message: String,
    pub location: Location,
}
