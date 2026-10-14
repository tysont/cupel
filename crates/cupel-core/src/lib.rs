// ABOUTME: Root of cupel-core, the crate every other Cupel crate builds on.
// ABOUTME: Holds the output contract types, the fact vocabulary, and the extension-model traits.
//! Core types shared by every Cupel crate: the output contract (design §7), identifiers and
//! structural hashes, and the extension model of facts, producers, and checks (design §8.3).

mod actions;
mod cancel;
mod check;
pub mod config;
mod delta;
mod describe;
mod dry_run;
mod envelope;
mod error;
mod event;
mod explanation;
mod fact;
mod finding;
mod gate;
pub mod git;
mod grade;
mod hash;
mod id;
mod lang;
mod matrix;
mod producer;
mod rank;
mod schedule;
pub mod schema;
mod scope;
mod select;
mod suppression;
pub mod syntax;
mod writer;

pub use actions::{NEXT_ACTIONS, next_actions};
pub use cancel::Cancellation;
pub use check::{
    Check, CheckCtx, CheckRegistry, FindingKindSpec, Manifest, MetricPer, MetricSpec, Plan,
    Stability,
};
pub use delta::{Deltas, deltas};
pub use describe::{
    AllDescription, ArgDescription, CheckDescription, CheckSupport, CliDescription,
    CommandDescription, CommandSummary, CommandTemplate, Description, Example, ExitCodeDescription,
    FactDescription, FactProducers, FindingDescription, LanguageDescription, LanguageProducers,
    LanguageSummary, LanguageSupport, LanguagesDescription, LauncherDescription, MatrixCell,
    MatrixDescription, ProducerDescription, SchemaDescription, SchemaRef, SupportDescription,
    ToolDescription, ToolchainDescription, ValueType,
};
pub use dry_run::{PlannedCheck, PlannedInstall, PlannedSkip, PlannedWrite, RunPlan};
pub use envelope::{
    BaselineRef, CheckResult, CheckStatus, ConfigEcho, ConfigSource, CupelInfo, Delta, Envelope,
    ExitCode, GateOutcome, GateResult, GateRule, GitInfo, NextAction, RuleOutcome, RunInfo,
    ScopeInfo, ScopeMode, Score, SeverityCounts, SkipReason, Skipped, Status, Summary,
    ToolchainInfo, Verb,
};
pub use error::{ArgumentError, Error, Result};
pub use event::{Event, EventSink, JsonlSink};
pub use explanation::{Excerpt, ExcerptLine, ExplainSubject, Explanation, Metric};
pub use fact::{Fact, FactKind, Facts};
pub use finding::{Action, ActionKind, Evidence, Finding, Location, Severity, Symbol};
pub use gate::{GATE_CHANGED, GATE_TREE, apply_gate, evaluate_gate, gate_for};
pub use grade::{CostClass, Tier};
pub use hash::{FindingId, SiteHash, SymbolHash};
pub use id::{CheckId, LangFamily, LangId, ProducerId, RunId, ToolId, random_seed};
pub use lang::{Lang, Langs};
pub use matrix::{Support, SupportMatrix};
pub use producer::{
    Availability, Ctx, ProduceCtx, Produced, Producer, ProducerRegistry, RegisteredProducer,
    SharedFact,
};
pub use rank::{
    Measured, Measurement, RANKED, RankedTarget, RankingDimension, Rankings, Target, rankings,
};
pub use schedule::{Job, Outcome, Scheduler};
pub use scope::{ResolvedFiles, Scope, ScopeRequest, SymbolSelector, select_symbols};
pub use select::Selection;
pub use suppression::{Suppression, Suppressions, Until, apply_suppressions};
pub use writer::{WriteError, Writer};
