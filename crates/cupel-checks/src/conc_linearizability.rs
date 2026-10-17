// ABOUTME: `conc.linearizability` (ADR-119): recorded operation histories no order consistent with real time explains
// ABOUTME: under their model, each with the operations that cannot come next and how to record the history again.

use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, FactKind,
    Facts, Finding, Histories, History, HistoryOp, HistoryStatus, HistoryViolation, Manifest, Plan,
    Result, RunStatus, Scope, Score, Severity, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::finding::SubjectFinding;
use crate::manifest::{evidence, kind, metric};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "conc.linearizability";

/// The check takes no settings; the checker's time limit follows the depth.
const LIMITS: Limits = Limits::new(&[]);

/// Evidence of `consistency.violation`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Violated {
    /// The history, as the history command named it.
    history: String,
    /// The model it declares.
    model: Option<String>,
    /// The key of a key-value history the violation is about; null for a single object.
    key: Option<Value>,
    /// The anomaly as the checker names it.
    anomaly: String,
    /// The checker.
    tool: String,
    /// The states the model can be in after the longest linearizable prefix.
    states: Vec<Value>,
    /// The last operations of that prefix, in order.
    linearized: Vec<Op>,
    /// The operations that could come next, none of which the model allows.
    stuck: Vec<Op>,
    /// The command that recorded the history.
    command: String,
    /// The seed it was given.
    seed: u64,
}

/// One operation of an excerpt; inlined, as evidence schemas have no `$defs` (ADR-043).
#[derive(Serialize, JsonSchema)]
#[schemars(inline)]
#[serde(deny_unknown_fields)]
struct Op {
    /// The process that invoked it.
    process: Value,
    /// Its function, such as `read`.
    f: String,
    /// Its key, in a key-value history.
    key: Option<Value>,
    /// What it wrote or compared, or what it read.
    value: Value,
    /// `ok`, or `info` when its outcome is unknown.
    outcome: String,
    /// The line of the history that invoked it.
    line: u32,
}

impl From<&HistoryOp> for Op {
    fn from(op: &HistoryOp) -> Self {
        Self {
            process: op.process.clone(),
            f: op.f.clone(),
            key: op.key.clone(),
            value: op.value.clone(),
            outcome: serde_json::to_value(op.outcome)
                .ok()
                .and_then(|outcome| outcome.as_str().map(str::to_owned))
                .unwrap_or_default(),
            line: op.line,
        }
    }
}

pub struct ConcLinearizability {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(ConcLinearizability::new())));

const METRICS: [(&str, &str); 5] = [
    ("histories", "Histories the history command wrote."),
    ("operations", "Operations in those histories."),
    (
        "violations",
        "Histories, or keys of key-value histories, no linearization explains.",
    ),
    ("unknown", "Histories the checker ran out of time on."),
    ("invalid", "Histories that could not be checked."),
];

impl ConcLinearizability {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(5).expect("tier 5 exists"),
                cost: CostClass::Deep,
                needs: vec![FactKind::Histories],
                wants: vec![],
                findings: vec![kind::<Violated>(
                    "consistency.violation",
                    ActionKind::FixBug,
                    "A recorded history of concurrent operations that no order consistent with \
                     real time explains under its model: the system is not linearizable.",
                )],
                metrics: METRICS
                    .into_iter()
                    .map(|(name, summary)| metric(name, summary))
                    .collect(),
                settings: LIMITS.specs(),
                stability: Stability::Experimental,
                summary: "Checks the operation histories a project's history command records \
                          against the model each declares (register, key-value store) with a \
                          linearizability checker (Porcupine); reports the operations no order \
                          explains."
                    .to_owned(),
            },
        }
    }
}

impl Check for ConcLinearizability {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// One target per file in scope; how many histories the command records is its own.
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
        let fact = facts.get::<Histories>()?;
        let findings: Vec<Finding> = fact
            .histories
            .iter()
            .flat_map(|history| {
                history
                    .violations
                    .iter()
                    .map(move |violation| self.violated(fact, history, violation))
            })
            .collect();
        let count = |status: HistoryStatus| {
            fact.histories
                .iter()
                .filter(|history| history.status == status)
                .count()
        };
        let operations: u64 = fact
            .histories
            .iter()
            .map(|history| history.operations)
            .sum();
        let counts = [
            ("histories", fact.histories.len()),
            (
                "operations",
                usize::try_from(operations).unwrap_or(usize::MAX),
            ),
            ("violations", findings.len()),
            ("unknown", count(HistoryStatus::Unknown)),
            ("invalid", count(HistoryStatus::Invalid)),
        ];
        let reported = findings.len() as u64;
        for finding in findings {
            sink.emit(Event::Finding(Box::new(finding)));
        }
        let mut result = outcome::result(
            &self.manifest,
            outcome::status(reported > 0),
            Some(outcome::counted(
                counts[2].1,
                "violations",
                fact.histories.len(),
            )),
            outcome::counts(&counts),
            reported,
        );
        result.notes = unchecked(fact);
        Ok(result)
    }

    /// Histories and violations add up across languages.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }

    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }
}

/// A note for a failed history command, one that wrote nothing, and each history that could
/// not be decided or checked.
fn unchecked(fact: &Histories) -> Vec<String> {
    let command = &fact.command;
    let mut notes = Vec::new();
    if fact.run.status == RunStatus::Failed {
        notes.push(format!(
            "the history command `{command}` failed: {}",
            fact.run.detail.as_deref().unwrap_or("no output")
        ));
    }
    if fact.histories.is_empty() {
        notes.push(format!("the history command `{command}` wrote no history"));
    }
    for history in &fact.histories {
        let verb = match history.status {
            HistoryStatus::Unknown => "decided",
            HistoryStatus::Invalid => "checked",
            HistoryStatus::Linearizable | HistoryStatus::Violated => continue,
        };
        notes.push(format!(
            "{} could not be {verb}: {}",
            history.name,
            history.detail.as_deref().unwrap_or("no detail")
        ));
    }
    notes
}

/// An operation as a title shows it: its function and value, its process, and its line.
fn shown(op: &HistoryOp) -> String {
    format!(
        "`{} {}` by process {} (line {})",
        op.f,
        compact(&op.value),
        compact(&op.process),
        op.line
    )
}

/// `value` as compact JSON.
fn compact(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// The stuck operations of a title: the first, and how many more.
fn stuck(ops: &[HistoryOp]) -> String {
    match ops {
        [] => "its operations".to_owned(),
        [only] => shown(only),
        [first, rest @ ..] => {
            let more = if rest.len() == 1 {
                "operation"
            } else {
                "operations"
            };
            format!("{} and {} more {more}", shown(first), rest.len())
        }
    }
}

/// What the stuck operations cannot do: follow the states the model is in, or be ordered at all.
fn after(states: &[Value]) -> String {
    match states {
        [] => "cannot be ordered".to_owned(),
        [state] => format!("cannot follow state `{}`", compact(state)),
        _ => format!(
            "cannot follow any of the states {}",
            states
                .iter()
                .map(|state| format!("`{}`", compact(state)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

impl ConcLinearizability {
    /// `consistency.violation` for one part of a history no linearization explains.
    fn violated(
        &self,
        fact: &Histories,
        history: &History,
        violation: &HistoryViolation,
    ) -> Finding {
        let name = &history.name;
        let model = history
            .model
            .as_ref()
            .map_or_else(String::new, |model| format!(" ({model})"));
        let (key, subject) = match &violation.key {
            Some(key) => (
                format!(" key `{}`", compact(key)),
                format!("{name}|{}", compact(key)),
            ),
            None => (String::new(), name.clone()),
        };
        let what = format!("{} {}", stuck(&violation.stuck), after(&violation.states));
        SubjectFinding {
            kind: "consistency.violation",
            subject,
            severity: Severity::Fail,
            confidence: 1.0,
            title: format!(
                "history `{name}`{model}{key} is {}: {what}",
                violation.anomaly
            ),
            summary: format!(
                "Find why {what} in `{name}`: the operations a linearizable system completes \
                 take effect in an order real time allows. `CUPEL_SEED={} {}` records the \
                 history again.",
                fact.seed, fact.command
            ),
            evidence: evidence(&Violated {
                history: name.clone(),
                model: history.model.clone(),
                key: violation.key.clone(),
                anomaly: violation.anomaly.clone(),
                tool: fact.tool.to_string(),
                states: violation.states.clone(),
                linearized: violation.linearized.iter().map(Op::from).collect(),
                stuck: violation.stuck.iter().map(Op::from).collect(),
                command: fact.command.clone(),
                seed: fact.seed,
            }),
            location: None,
            repro: format!("cupel check --only {ID} --seed {}", fact.seed),
        }
        .finding(&self.manifest)
    }
}
