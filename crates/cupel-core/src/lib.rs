// ABOUTME: Root of cupel-core, the crate every other Cupel crate builds on.
// ABOUTME: Holds the output contract types, the fact vocabulary, and the extension-model traits.
//! Core types shared by every Cupel crate: the output contract (design §7), identifiers and
//! structural hashes, and the extension model of facts, producers, and checks (design §8.3).

mod actions;
mod analyzer;
mod attribution;
mod base;
mod bench;
pub mod budget;
mod cancel;
mod check;
pub mod config;
mod contract;
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
pub mod hint;
mod histories;
mod id;
mod joint;
mod json_schema;
mod lang;
pub mod matching;
mod matrix;
mod memory;
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
pub mod similarity;
mod simulation;
mod status;
mod suppression;
pub mod syntax;
mod test_results;
mod trace;
mod writer;

pub use actions::{NEXT_ACTIONS, next_actions};
pub use analyzer::{AnalyzerLevel, AnalyzerReport, AnalyzerResult, AnalyzerRun, AnalyzerStatus};
pub use attribution::{ATTRIBUTION_KEY, Attribution, attributions};
pub use base::{BaseTree, Range};
pub use bench::{BenchBase, BenchSamples, Benchmark};
pub use cancel::Cancellation;
pub use check::{
    Check, CheckCtx, CheckRegistry, FindingKindSpec, Manifest, MetricSpec, Plan, SettingSpec,
    Stability,
};
pub use config::{ConfigEcho, ConfigSource};
pub use contract::{ContractKind, ContractPlace, ContractViolation, ContractViolations};
pub use delta::{Deltas, Matching, SymbolMetrics, deltas, deltas_matched};
pub use describe::{
    AgentDescription, AllDescription, ArgDescription, CheckDescription, CheckSupport,
    CliDescription, CommandDescription, CommandSummary, CommandTemplate, Description, Example,
    ExitCodeDescription, FactDescription, FactProducers, FindingDescription, InstallRoute,
    LanguageDescription, LanguageProducers, LanguageSummary, LanguageSupport, LanguagesDescription,
    LauncherDescription, MatrixCell, MatrixDescription, ProducerDescription, SchemaDescription,
    SchemaRef, SupportDescription, ToolDescription, ToolchainDescription, ValueType, VerbSupport,
};
pub use divergence::{Answer, ComparedOracle, Divergence, DivergenceResults, EntryComparison};
pub use dry_run::{PlannedCheck, PlannedInstall, PlannedSkip, PlannedWrite, RunPlan};
pub use envelope::{
    BaselineRef, CheckResult, CheckStatus, CupelInfo, Delta, Envelope, ExitCode, GateOutcome,
    GateResult, GateRule, GitInfo, NextAction, RuleOutcome, RunInfo, ScopeInfo, ScopeMode, Score,
    SetDelta, SetFindings, SetMetric, SeverityCounts, SkipReason, Skipped, Status, Summary,
    ToolchainInfo, Verb,
};
pub use error::{ArgumentError, Error, Result};
pub use event::{Event, EventSink, JsonlSink};
pub use exclude::{ALWAYS_EXCLUDED, Exclude, PathGlobs};
pub use explanation::{Excerpt, ExcerptLine, ExplainSubject, Explanation, Metric};
pub use fact::{Fact, FactEntry, FactKind, Produced, SharedFact};
pub use facts::Facts;
pub use finding::{Action, ActionKind, Evidence, Finding, Location, Severity, Symbol};
pub use flake::{RunOrder, TestRerun, TestReruns};
pub use footprint::{Artifact, Dependency, Footprint, FootprintBase, Vulnerability};
pub use fuzz::{FuzzCrash, FuzzResults, FuzzStatus, FuzzTarget};
pub use gate::{
    GATE_CHANGED, GATE_TREE, GateReference, NO_REFERENCE, apply_gate, evaluate_gate, gate_for,
    needs_reference,
};
pub use grade::{CostClass, Tier};
pub use hash::{FindingId, SiteHash, SymbolHash};
pub use histories::{Histories, History, HistoryOp, HistoryStatus, HistoryViolation, OpOutcome};
pub use id::{CheckId, LangFamily, LangId, ProducerId, RunId, ToolId, random_seed};
pub use lang::{Lang, Langs};
pub use matrix::{Support, SupportMatrix};
pub use memory::physical_memory;
pub use model_check::{
    Counterexample, HarnessStatus, ModelCheckResults, ModelHarness, RefutedProperty,
};
pub use mutation::{
    ExcludedTest, Mutant, MutantOutcome, MutationOutcomes, MutationPhase, MutationStrategy,
    PhaseDuration, SLOW_TEST_MS,
};
pub use oracle::{OracleReport, OracleState, RunOracle, oracle_content_digest, oracle_identity};
pub use per_test::{CoveredLines, LineIndex, PerTestMap, TestCoverage, case_name};
pub use per_test_impact::{ImpactBasis, ImpactedTest, TestCommand, TestsReport};
pub use process::{Captured, Process, find_program};
pub use producer::{
    Availability, BuildOutcome, Ctx, Estimate, Focus, Install, JointProducer, Prebuilt, ProduceCtx,
    Producer, ProducerRegistry, RegisteredProducer, RunOptions, RunOrdering, Runner, RunnerCommand,
    Runners, TestRun, TestSelection, ToolProbe, Upstream,
};
pub use proof::{Obligation, ObligationStatus, ProofResults, RelatedPlace};
pub use property::{PropertyOutcome, PropertyResult, PropertyResults};
pub use race::{RaceFrame, RaceKind, RaceReport, RaceReports, RaceStack};
pub use rank::{
    Measured, Measurement, MetricPer, Protection, ProtectionMetric, RANKED, RankedTarget,
    RankingDimension, Rankings, Recommendation, RecommendedTarget, Target, TargetValue, Worst,
};
pub use ranking::rankings;
pub use relation::{
    Evaluated, InputType, RELATIONS_KEY, Relation, RelationResult, RelationResults, Violation,
    relations,
};
pub use root_run::{RootRun, RunStatus};
pub use schedule::{Job, Outcome, Production, Scheduler};
pub use scope::{ChangedLines, ResolvedFiles, Scope, ScopeRequest, SymbolSelector, select_symbols};
pub use select::Selection;
pub use simulation::{SimulationFailure, SimulationHarness, SimulationResults, SimulationStatus};
pub use suppression::{Suppression, Suppressions, Until, apply_suppressions};
pub use test_results::{
    OUTPUT_TAIL_LINES, RunnerInvocation, SuiteMode, SuiteReuse, TestOutcome, TestResult,
    TestResults, Unevaluable, names_case, output_tail,
};
pub use trace::{Trace, TraceDivergence, TraceSpec, TraceStatus, Traces};
pub use writer::{WriteError, Writer};
