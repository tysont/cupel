// ABOUTME: `spec.trace` (ADR-116): traces the tests recorded that are not behaviors of the run's spec oracle, from
// ABOUTME: the Traces fact, one failing finding per diverged trace at the step it left the spec.
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, FactKind,
    Facts, Finding, Manifest, Plan, Result, Scope, Score, Severity, Stability, Tier, Trace,
    TraceDivergence, TraceStatus, Traces,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::finding::SubjectFinding;
use crate::manifest::{evidence, kind, metric};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "spec.trace";

/// The check takes no settings: the spec is the run's oracle, the traces the tests' own.
const LIMITS: Limits = Limits::new(&[]);

/// The metrics, each with what it counts.
const METRICS: [(&str, &str); 3] = [
    ("traces", "Recorded traces validated against the spec."),
    ("diverged", "Validated traces the spec does not allow."),
    (
        "unvalidated",
        "Recorded traces that could not be validated.",
    ),
];

/// Evidence of `trace.diverged`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Diverged {
    /// The validator that ran, such as `tool.tlc`.
    tool: String,
    /// The spec oracle the trace was validated against.
    oracle: String,
    /// Its specification file, relative to the repository root.
    spec: String,
    /// The trace file's name in the directory the tests recorded into.
    trace: String,
    /// The states the trace records.
    steps: u64,
    /// The first step the spec does not allow, counting the initial state as 1.
    step: u64,
    /// The spec action the implementation logged for the step, when it named one.
    action: Option<String>,
    /// The state the implementation logged at the step.
    state: Value,
    /// The last state the spec allows, before the step; null at the initial state.
    previous: Option<Value>,
}

pub struct SpecTrace {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(SpecTrace::new())));

impl SpecTrace {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(5).expect("tier 5 exists"),
                cost: CostClass::Exhaustive,
                needs: vec![FactKind::Traces],
                wants: vec![],
                findings: vec![kind::<Diverged>(
                    "trace.diverged",
                    ActionKind::FixBug,
                    "A trace the tests recorded is not a behavior of the specification: at \
                     some step the implementation reached a state the spec does not allow \
                     from the state before.",
                )],
                metrics: METRICS
                    .iter()
                    .map(|(name, counts)| metric(name, counts))
                    .collect(),
                settings: LIMITS.specs(),
                stability: Stability::Experimental,
                summary: "Runs the tests with `CUPEL_TRACE_DIR` set, so the implementation logs \
                          its steps as JSON lines, and validates each recorded trace as a \
                          behavior of the run's spec oracle (TLA+ with TLC)."
                    .to_owned(),
            },
        }
    }
}

impl Check for SpecTrace {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        LIMITS.plan_files(&self.manifest, ctx, scope)
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        LIMITS.read(&self.manifest.id, &ctx.config)?;
        let fact = facts.get::<Traces>()?;
        let count = |status: TraceStatus| {
            fact.traces
                .iter()
                .filter(|trace| trace.status == status)
                .count()
        };
        let diverged = count(TraceStatus::Diverged);
        let validated = diverged + count(TraceStatus::Conforms);
        for trace in &fact.traces {
            if let Some(divergence) = &trace.divergence {
                let finding = self.finding(fact, trace, divergence);
                sink.emit(Event::Finding(Box::new(finding)));
            }
        }
        let measured = validated > 0;
        let mut result = outcome::result(
            &self.manifest,
            outcome::status(diverged > 0),
            measured.then(|| outcome::counted(diverged, "diverged", validated)),
            if measured {
                outcome::counts(&[
                    ("traces", validated),
                    ("diverged", diverged),
                    ("unvalidated", count(TraceStatus::Failed)),
                ])
            } else {
                std::collections::BTreeMap::new()
            },
            diverged as u64,
        );
        result.notes = notes(fact);
        Ok(result)
    }

    /// Its results name the run's oracle and are capped at tier 3 against one that is not
    /// verified (design §6.5, ADR-091).
    fn uses_oracle(&self) -> bool {
        true
    }

    /// Traces and divergences add up across languages.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }

    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }
}

/// What kept traces from being validated: the recording's problem, and each trace that failed.
fn notes(fact: &Traces) -> Vec<String> {
    let problem = fact
        .problem
        .iter()
        .map(|problem| format!("{}: {problem}", fact.tool));
    let failed = fact
        .traces
        .iter()
        .filter(|trace| trace.status == TraceStatus::Failed)
        .map(|trace| {
            format!(
                "trace `{}` could not be validated: {}",
                trace.name,
                trace.detail.as_deref().unwrap_or("no detail")
            )
        });
    problem.chain(failed).collect()
}

impl SpecTrace {
    fn finding(&self, fact: &Traces, trace: &Trace, divergence: &TraceDivergence) -> Finding {
        let (oracle, spec) = fact
            .spec
            .as_ref()
            .map(|spec| (spec.oracle.clone(), spec.file.clone()))
            .unwrap_or_default();
        let state = divergence.state.to_string();
        let title = match (&divergence.previous, &divergence.action) {
            (None, _) => format!(
                "trace `{}` starts at {state}, which is not an initial state of {spec}",
                trace.name
            ),
            (Some(previous), action) => format!(
                "trace `{}` leaves {spec} at step {}: {} to {state} is not a step the spec allows \
                 from {previous}",
                trace.name,
                divergence.step,
                action
                    .as_ref()
                    .map_or_else(|| "a step".to_owned(), |action| format!("`{action}`")),
            ),
        };
        let paths = divergence
            .location
            .as_ref()
            .map(|at| format!(" --paths {}", at.file))
            .unwrap_or_default();
        SubjectFinding {
            kind: "trace.diverged",
            subject: format!(
                "{}|{}|{state}",
                trace.name,
                divergence.action.as_deref().unwrap_or_default()
            ),
            severity: Severity::Fail,
            confidence: 1.0,
            title,
            summary: format!(
                "Make the implementation take only steps {spec} allows from the state before \
                 (or correct the spec, if it is what is wrong), then rerun the tests that record \
                 the trace."
            ),
            evidence: evidence(&Diverged {
                tool: fact.tool.to_string(),
                oracle: oracle.clone(),
                spec,
                trace: trace.name.clone(),
                steps: trace.steps,
                step: divergence.step,
                action: divergence.action.clone(),
                state: divergence.state.clone(),
                previous: divergence.previous.clone(),
            }),
            location: divergence.location.clone(),
            repro: format!("cupel check --only {ID} --oracle {oracle}{paths}"),
        }
        .finding(&self.manifest)
    }
}
