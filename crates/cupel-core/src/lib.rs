// ABOUTME: Root of cupel-core, the crate every other Cupel crate builds on.
// ABOUTME: Holds the output contract types, the fact vocabulary, and the extension-model traits.
//! Core types shared by every Cupel crate: the output contract (design §7), identifiers and
//! structural hashes, and the extension model of facts, producers, and checks (design §8.3).

mod actions;
mod analyzer;
mod base;
mod bench;
pub mod budget;
mod cancel;
mod check;
pub mod config;
pub mod coverage;
mod delta;
mod describe;
mod divergence;
mod dry_run;
mod envelope;
mod error;
mod event;
mod exclude;
mod explanation;
mod fact;
mod facts;
mod finding;
mod flake;
mod footprint;
mod fuzz;
mod gate;
pub mod git;
mod grade;
mod hash;
mod histories;
mod id;
mod joint;
mod lang;
mod matrix;
mod model_check;
mod mutation;
mod oracle;
mod per_test;
mod per_test_impact;
pub mod plugin;
mod process;
mod producer;
mod proof;
mod property;
mod race;
mod rank;
mod ranking;
mod relation;
mod result;
mod root_run;
mod schedule;
pub mod schema;
mod scope;
mod select;
mod simulation;
mod status;
mod suppression;
pub mod syntax;
mod test_results;
mod trace;
mod writer;

pub use actions::{NEXT_ACTIONS, next_actions};
pub use analyzer::{AnalyzerLevel, AnalyzerReport, AnalyzerResult, AnalyzerRun, AnalyzerStatus};
pub use base::{BaseTree, Range};
pub use bench::{BenchBase, BenchSamples, Benchmark};
pub use cancel::Cancellation;
pub use check::{
    Check, CheckCtx, CheckRegistry, FindingKindSpec, Manifest, MetricSpec, Plan, SettingSpec,
    Stability,
};
pub use config::{ConfigEcho, ConfigSource};
pub use delta::{Deltas, deltas};
pub use describe::{
    AllDescription, ArgDescription, CheckDescription, CheckSupport, CliDescription,
    CommandDescription, CommandSummary, CommandTemplate, Description, Example, ExitCodeDescription,
    FactDescription, FactProducers, FindingDescription, LanguageDescription, LanguageProducers,
    LanguageSummary, LanguageSupport, LanguagesDescription, LauncherDescription, MatrixCell,
    MatrixDescription, ProducerDescription, SchemaDescription, SchemaRef, SupportDescription,
    ToolDescription, ToolchainDescription, ValueType, VerbSupport,
};
pub use divergence::{Answer, ComparedOracle, Divergence, DivergenceResults, EntryComparison};
pub use dry_run::{PlannedCheck, PlannedInstall, PlannedSkip, PlannedWrite, RunPlan};
pub use envelope::{
    BaselineRef, CheckResult, CheckStatus, CupelInfo, Delta, Envelope, ExitCode, GateOutcome,
    GateResult, GateRule, GitInfo, NextAction, RuleOutcome, RunInfo, ScopeInfo, ScopeMode, Score,
    SeverityCounts, SkipReason, Skipped, Status, Summary, ToolchainInfo, Verb,
};
pub use error::{ArgumentError, Error, Result};
pub use event::{Event, EventSink, JsonlSink};
pub use exclude::{ALWAYS_EXCLUDED, Exclude};
pub use explanation::{Excerpt, ExcerptLine, ExplainSubject, Explanation, Metric};
pub use fact::{Fact, FactKind};
pub use facts::Facts;
pub use finding::{Action, ActionKind, Evidence, Finding, Location, Severity, Symbol};
pub use flake::{RunOrder, TestRerun, TestReruns};
pub use footprint::{Artifact, Dependency, Footprint, FootprintBase, Vulnerability};
pub use fuzz::{FuzzCrash, FuzzResults, FuzzStatus, FuzzTarget};
pub use gate::{GATE_CHANGED, GATE_TREE, apply_gate, evaluate_gate, gate_for};
pub use grade::{CostClass, Tier};
pub use hash::{FindingId, SiteHash, SymbolHash};
pub use histories::{Histories, History, HistoryOp, HistoryStatus, HistoryViolation, OpOutcome};
pub use id::{CheckId, LangFamily, LangId, ProducerId, RunId, ToolId, random_seed};
pub use lang::{Lang, Langs};
pub use matrix::{Support, SupportMatrix};
pub use model_check::{
    Counterexample, HarnessStatus, ModelCheckResults, ModelHarness, RefutedProperty,
};
pub use mutation::{Mutant, MutantOutcome, MutationOutcomes, MutationStrategy};
pub use oracle::{OracleReport, OracleState, RunOracle, oracle_content_digest, oracle_identity};
pub use per_test::{CoveredLines, LineIndex, PerTestMap, TestCoverage};
pub use per_test_impact::{ImpactBasis, ImpactedTest, TestCommand, TestsReport};
pub use process::{Captured, Process, find_program};
pub use producer::{
    Availability, ChangedLines, Ctx, Estimate, Focus, Install, JointProducer, ProduceCtx, Produced,
    Producer, ProducerRegistry, RegisteredProducer, RunOptions, RunOrdering, Runner, RunnerCommand,
    Runners, SharedFact, TestRun, TestSelection, ToolProbe, Upstream,
};
pub use proof::{Obligation, ObligationStatus, ProofResults, RelatedPlace};
pub use property::{PropertyOutcome, PropertyResult, PropertyResults};
pub use race::{RaceFrame, RaceKind, RaceReport, RaceReports, RaceStack};
pub use rank::{
    Measured, Measurement, MetricPer, RANKED, RankedTarget, RankingDimension, Rankings, Target,
};
pub use ranking::rankings;
pub use relation::{
    Evaluated, InputType, RELATIONS_KEY, Relation, RelationResult, RelationResults, Violation,
    relations,
};
pub use root_run::{RootRun, RunStatus};
pub use schedule::{Job, Outcome, Production, Scheduler};
pub use scope::{ResolvedFiles, Scope, ScopeRequest, SymbolSelector, select_symbols};
pub use select::Selection;
pub use simulation::{SimulationFailure, SimulationHarness, SimulationResults, SimulationStatus};
pub use suppression::{Suppression, Suppressions, Until, apply_suppressions};
pub use test_results::{
    OUTPUT_TAIL_LINES, RunnerInvocation, TestOutcome, TestResult, TestResults, Unevaluable,
    names_case, output_tail,
};
pub use trace::{Trace, TraceDivergence, TraceSpec, TraceStatus, Traces};
pub use writer::{WriteError, Writer};
