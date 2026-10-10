// ABOUTME: Root of cupel-core, the crate every other Cupel crate builds on.
// ABOUTME: Holds the output contract types, the fact vocabulary, and the extension-model traits.
//! Core types shared by every Cupel crate: the output contract (design §7), identifiers and
//! structural hashes, and the extension model of facts, producers, and checks (design §8.3).

mod check;
pub mod config;
mod describe;
mod dry_run;
mod envelope;
mod error;
mod event;
mod fact;
mod finding;
mod grade;
mod hash;
mod id;
mod lang;
mod matrix;
mod producer;
pub mod schema;
mod scope;
pub mod syntax;
mod writer;

pub use check::{
    Check, CheckCtx, CheckRegistry, FindingKindSpec, Manifest, MetricSpec, Plan, Stability,
};
pub use describe::{
    AllDescription, ArgDescription, CliDescription, CommandDescription, CommandSummary,
    Description, Example, ExitCodeDescription, SchemaDescription, SchemaRef, ValueType,
};
pub use dry_run::{PlannedCheck, PlannedInstall, PlannedSkip, PlannedWrite, RunPlan};
pub use envelope::{
    BaselineRef, CheckResult, CheckStatus, ConfigEcho, ConfigSource, CupelInfo, Delta, Envelope,
    ExitCode, GateOutcome, GateResult, GateRule, GitInfo, NextAction, RunInfo, ScopeInfo,
    ScopeMode, Score, SeverityCounts, SkipReason, Skipped, Status, Summary, ToolchainInfo, Verb,
};
pub use error::{Error, Result};
pub use event::{Event, EventSink};
pub use fact::{Fact, FactKind, Facts};
pub use finding::{Action, ActionKind, Evidence, Finding, Location, Severity, Symbol};
pub use grade::{CostClass, Tier};
pub use hash::{FindingId, SiteHash, SymbolHash};
pub use id::{CheckId, LangFamily, LangId, ProducerId, RunId, ToolId};
pub use lang::{Lang, Langs};
pub use matrix::{Support, SupportMatrix};
pub use producer::{Availability, Ctx, ProduceCtx, Producer, ProducerRegistry, RegisteredProducer};
pub use scope::Scope;
pub use writer::{WriteError, Writer};
