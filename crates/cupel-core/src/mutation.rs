// ABOUTME: The `MutationOutcomes` fact payload (ADR-080, ADR-082): every mutant an engine planned, what became of it,
// ABOUTME: the tests that ran against it, how it was built, and whether its outcome came from the store.
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactEntry, FactKind};
use crate::grade::CostClass;
use crate::hash::{SiteHash, SymbolHash};
use crate::id::ProducerId;
use crate::syntax::{MutationOperator, Span, SymbolKey};

/// What mutation testing found in scope (ADR-080; version 2 adds how each mutant was built,
/// ADR-082).
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
    /// The tests slower than 60 s (`SLOW_TEST_MS`) in the baseline that were left out of the
    /// mutants' whole-suite runs (ADR-147), sorted by ID; absent when none was.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-cupel-optional" = true))]
    pub excluded: Vec<ExcludedTest>,
}

/// A test whose baseline run is longer than this, in milliseconds, is left out of every mutant
/// run that per-test selection did not narrow, where the runner can leave it out (ADR-147).
pub const SLOW_TEST_MS: u64 = 60_000;

/// A test left out of the mutants' whole-suite runs for its baseline duration (ADR-147).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExcludedTest {
    /// The runner's ID of the test.
    pub id: String,
    /// The file the runner attributes it to, relative to the repository root.
    pub file: Option<String>,
    /// Its duration in the baseline run.
    pub baseline_ms: u64,
}

impl Fact for MutationOutcomes {
    const KIND: FactKind = FactKind::MutationOutcomes;
    const VERSION: u32 = 2;
}

inventory::submit!(
    FactEntry::execution::<MutationOutcomes>("mutation-outcomes").pinned_by("mutator")
);

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
            excluded: Vec::new(),
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
    /// How the mutant was built (ADR-082); `None` when it was not run, or its outcome was
    /// stored before strategies were recorded.
    #[serde(default)]
    pub strategy: Option<MutationStrategy>,
    /// Why a mutant of a language with schemata was built as a file copy instead; `None`
    /// otherwise.
    #[serde(default)]
    pub fallback: Option<String>,
}

/// How a mutant was built (ADR-082).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MutationStrategy {
    /// One build of the file with every mutant behind a run-time switch, selected by
    /// `CUPEL_MUTANT`.
    Schemata,
    /// The file rewritten with only this mutant, and rebuilt.
    FileCopy,
}

impl MutationStrategy {
    /// The strategy's name as it serializes.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Schemata => "schemata",
            Self::FileCopy => "file-copy",
        }
    }
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
