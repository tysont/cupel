// ABOUTME: The `MutationOutcomes` fact payload (ADR-080, ADR-082, ADR-150): every mutant an engine planned, what became
// ABOUTME: of it, the tests that ran against it, how it was built, whether it was reused, and the production's phases.
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
    /// The phases the production went through, in the order they ended: its timeline, from
    /// which each phase's total is read (ADR-150). Absent when no mutant ran.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-cupel-optional" = true))]
    pub phases: Vec<PhaseDuration>,
}

/// One phase of a mutation production and how long it took (ADR-150).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PhaseDuration {
    pub phase: MutationPhase,
    /// The file whose mutants the phase served, relative to the repository root; `None` for a
    /// phase of the whole production (`plan`, `sync`).
    pub file: Option<String>,
    /// The slot the phase ran in; `None` for a phase that runs in none (`plan`, `rewrite`).
    pub slot: Option<u32>,
    /// How many tests the phase ran, when it ran tests selected for the file's mutants; `None`
    /// for a phase that runs no tests, or that ran the whole suite.
    pub tests: Option<u32>,
    pub duration_ms: u64,
}

/// A phase of a mutation production (ADR-150).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum MutationPhase {
    /// Planning the mutants in focus: coverage gating, per-test selection, and the store's
    /// history; once per production.
    Plan,
    /// One slot's reconciliation with the tree (ADR-080 §8); once per slot used.
    Sync,
    /// One file's schemata rewrite (ADR-082 §2); once per attempt.
    Rewrite,
    /// The build every mutant of a file shares. With a runner that builds once (ADR-150 §8),
    /// the compile alone, once per file in the first slot; otherwise the schemata validation
    /// (ADR-082 §5) or a file copy's reference run (ADR-080 §9), the build and the file's
    /// selected tests with no mutant, once per slot the file used.
    Build,
    /// One mutant's test run, as long as the mutant's `duration_ms`.
    Run,
    /// The file's selected tests with no mutant, run once from a build made once (ADR-150 §8);
    /// its time sets every slot's timeout.
    Validate,
}

impl MutationPhase {
    /// The phase's name as it serializes.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Sync => "sync",
            Self::Rewrite => "rewrite",
            Self::Build => "build",
            Self::Run => "run",
            Self::Validate => "validate",
        }
    }

    /// Every phase, in the order they were added to the breakdown.
    pub const ALL: [Self; 6] = [
        Self::Plan,
        Self::Sync,
        Self::Rewrite,
        Self::Build,
        Self::Run,
        Self::Validate,
    ];
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
            phases: Vec::new(),
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
    /// Its run held more memory than its budget and was killed (ADR-148).
    OutOfMemory,
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
            Self::Timeout | Self::OutOfMemory | Self::Uncovered | Self::Unviable => None,
        }
    }
}

/// Each outcome's name, in declaration order (so `OUTCOMES[outcome as usize]` names it).
const OUTCOMES: [&str; 6] = [
    "killed",
    "survived",
    "timeout",
    "out-of-memory",
    "uncovered",
    "unviable",
];

impl fmt::Display for MutantOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
