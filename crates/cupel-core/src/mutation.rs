// ABOUTME: The `MutationOutcomes` fact payload (ADR-080): every mutant an engine planned, with what became of it,
// ABOUTME: the tests that ran against it, and whether its outcome came from the store.
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactKind};
use crate::grade::CostClass;
use crate::hash::{SiteHash, SymbolHash};
use crate::id::ProducerId;
use crate::syntax::{MutationOperator, Span, SymbolKey};

/// What mutation testing found in scope (ADR-080).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MutationOutcomes {
    /// The producer, such as `engine.mutation`.
    pub engine: ProducerId,
    /// The runner that ran the mutants; `None` when none ran.
    pub runner: Option<ProducerId>,
    /// The depth the engine planned for.
    pub depth: CostClass,
    /// Whether the tests run against each mutant were selected by a `PerTestMap`.
    pub per_test: bool,
    /// One per (site, replacement), sorted by file, position, then replacement.
    pub mutants: Vec<Mutant>,
}

impl Fact for MutationOutcomes {
    const KIND: FactKind = FactKind::MutationOutcomes;
    const VERSION: u32 = 1;
}

impl MutationOutcomes {
    /// The outcomes of `mutants`, sorted.
    pub fn new(
        engine: ProducerId,
        runner: Option<ProducerId>,
        depth: CostClass,
        per_test: bool,
        mut mutants: Vec<Mutant>,
    ) -> Self {
        mutants.sort_by(|a, b| {
            (&a.symbol.file, a.span.line, a.span.col, &a.to).cmp(&(
                &b.symbol.file,
                b.span.line,
                b.span.col,
                &b.to,
            ))
        });
        Self {
            engine,
            runner,
            depth,
            per_test,
            mutants,
        }
    }
}

/// One mutant: a site with one replacement, and what became of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Mutant {
    /// The symbol the site belongs to.
    pub symbol: SymbolKey,
    /// The symbol's structural hash (ADR-025), when the engine had one.
    pub symbol_hash: Option<SymbolHash>,
    pub site_hash: SiteHash,
    pub operator: MutationOperator,
    /// The source text replaced.
    pub from: String,
    /// What replaced it.
    pub to: String,
    /// Where `from` is, in the symbol's file.
    pub span: Span,
    pub outcome: MutantOutcome,
    /// The tests run against the mutant, by their runner's IDs; empty when it was not run.
    pub tests: Vec<String>,
    /// Of `tests`, those that failed or errored.
    pub killing_tests: Vec<String>,
    /// The wall time of the mutant's run; `None` when it was not run.
    pub duration_ms: Option<u64>,
    /// Whether the outcome was reused from an earlier run (ADR-080 §10).
    pub reused: bool,
    /// Why the mutant may be equivalent to the original (ADR-004); `None` when nothing says so.
    pub equivalent: Option<String>,
    /// The key its outcome is stored and reused under (ADR-080 §10); `None` from a producer
    /// that does not reuse outcomes.
    pub cache_key: Option<String>,
}

/// What became of a mutant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MutantOutcome {
    /// A test failed against it.
    Killed,
    /// Every test run against it passed.
    Survived,
    /// Its run took longer than its timeout.
    Timeout,
    /// No test runs its site, so it was not run (coverage gating).
    Uncovered,
    /// Its run did not complete and no test failed: it did not build or load.
    Unviable,
}

impl MutantOutcome {
    /// The outcome's name as it serializes.
    pub fn as_str(self) -> &'static str {
        OUTCOMES[self as usize]
    }

    /// For a scored outcome, whether a test killed the mutant; `None` for outcomes the score
    /// leaves out.
    pub fn killed(self) -> Option<bool> {
        match self {
            Self::Killed => Some(true),
            Self::Survived => Some(false),
            Self::Timeout | Self::Uncovered | Self::Unviable => None,
        }
    }
}

/// Each outcome's name, in declaration order (so `OUTCOMES[outcome as usize]` names it).
const OUTCOMES: [&str; 5] = ["killed", "survived", "timeout", "uncovered", "unviable"];

impl fmt::Display for MutantOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
