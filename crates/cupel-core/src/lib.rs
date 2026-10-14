// ABOUTME: Root of cupel-core, the crate every other Cupel crate builds on.
// ABOUTME: Holds the output contract types, the fact vocabulary, and the extension-model traits.
//! Core types shared by every Cupel crate: the output contract (design §7), identifiers and
//! structural hashes, and the extension model of facts, producers, and checks (design §8.3).

mod actions;
mod base;
mod cancel;
mod check;
pub mod config;
pub mod coverage;
mod delta;
mod describe;
mod dry_run;
mod envelope;
mod error;
mod event;
mod exclude;
mod explanation;
mod fact;
mod facts;
mod finding;
mod gate;
pub mod git;
mod grade;
mod hash;
mod id;
mod lang;
mod matrix;
mod per_test;
mod per_test_impact;
mod process;
mod producer;
mod rank;
mod ranking;
mod result;
mod schedule;
pub mod schema;
mod scope;
mod select;
mod status;
mod suppression;
pub mod syntax;
mod test_results;
mod writer;

pub use actions::{NEXT_ACTIONS, next_actions};
pub use base::{BaseTree, Range};
pub use cancel::Cancellation;
pub use check::{
    Check, CheckCtx, CheckRegistry, FindingKindSpec, Manifest, MetricSpec, Plan, Stability,
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
pub use gate::{GATE_CHANGED, GATE_TREE, apply_gate, evaluate_gate, gate_for};
pub use grade::{CostClass, Tier};
pub use hash::{FindingId, SiteHash, SymbolHash};
pub use id::{CheckId, LangFamily, LangId, ProducerId, RunId, ToolId, random_seed};
pub use lang::{Lang, Langs};
pub use matrix::{Support, SupportMatrix};
pub use per_test::{CoveredLines, LineIndex, PerTestMap, TestCoverage};
pub use per_test_impact::{ImpactBasis, ImpactedTest, TestCommand, TestsReport};
pub use process::{Captured, Process, find_program};
pub use producer::{
    Availability, Ctx, ProduceCtx, Produced, Producer, ProducerRegistry, RegisteredProducer,
    Runner, RunnerCommand, SharedFact, TestSelection, ToolProbe, Upstream,
};
pub use rank::{
    Measured, Measurement, MetricPer, RANKED, RankedTarget, RankingDimension, Rankings, Target,
};
pub use ranking::rankings;
pub use schedule::{Job, Outcome, Production, Scheduler};
pub use scope::{ResolvedFiles, Scope, ScopeRequest, SymbolSelector, select_symbols};
pub use select::Selection;
pub use suppression::{Suppression, Suppressions, Until, apply_suppressions};
pub use test_results::{
    OUTPUT_TAIL_LINES, RunnerInvocation, TestOutcome, TestResult, TestResults, Unevaluable,
    output_tail,
};
pub use writer::{WriteError, Writer};
