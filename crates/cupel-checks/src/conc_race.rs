// ABOUTME: `conc.race`: data races and undefined behavior a race detector found running a language's tests, from the
// ABOUTME: RaceReports fact, one failing finding per report, identified by its test, message, and functions (ADR-105).
use std::collections::{BTreeMap, HashMap};

use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, FactKind,
    Facts, Finding, Location, Manifest, Plan, RaceKind, RaceReport, RaceReports, RaceStack, Result,
    RunStatus, Scope, Score, Severity, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::finding::SubjectFinding;
use crate::manifest::{evidence, kind, metric};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "conc.race";

/// The check has no settings: which detector runs is the language's (`[lang.<id>] race`).
const SETTINGS: Limits = Limits::new(&[]);

/// What each report kind is: its finding kind, what the kind means, and what its action asks.
struct Said {
    kind: &'static str,
    means: &'static str,
    asks: &'static str,
}

const RACE: Said = Said {
    kind: "race.detected",
    means: "Two threads accessed the same memory without synchronization while the tests ran, \
            one of them writing.",
    asks: "Synchronize the accesses (a lock, an atomic, a channel) or stop sharing the memory, \
           then rerun the race detector.",
};

const UNDEFINED: Said = Said {
    kind: "ub.detected",
    means: "An operation with undefined behavior ran during the tests: an out-of-bounds or \
            dangling access, an invalid value, a broken aliasing rule.",
    asks: "Fix the operation the detector names so it stays within what the language defines, \
           then rerun it.",
};

/// The metrics, each with what it counts.
const METRICS: [(&str, &str); 2] = [
    ("races", "Data races the detector reported."),
    (
        "undefined_behavior",
        "Operations with undefined behavior the detector reported.",
    ),
];

/// Evidence of `race.detected` and `ub.detected`: the report as the fact holds it.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Reported {
    /// The detector, such as `tool.go-race` or `tool.miri`.
    tool: String,
    /// The test that was running, when the detector says.
    test: Option<String>,
    /// What the detector says happened, with addresses and IDs masked.
    message: String,
    /// Each conflicting access of a data race, or where undefined behavior happened.
    stacks: Vec<Stack>,
}

/// One stack of a report; inlined, as evidence schemas have no `$defs` (ADR-043).
#[derive(Serialize, JsonSchema)]
#[schemars(inline)]
#[serde(deny_unknown_fields)]
struct Stack {
    /// The access, such as `read` or `write`; null for undefined behavior.
    access: Option<String>,
    /// Frames in files of the repository, innermost first.
    frames: Vec<Frame>,
}

/// One frame of a stack.
#[derive(Serialize, JsonSchema)]
#[schemars(inline)]
#[serde(deny_unknown_fields)]
struct Frame {
    function: Option<String>,
    file: String,
    line: u32,
}

impl From<&RaceStack> for Stack {
    fn from(stack: &RaceStack) -> Self {
        Self {
            access: stack.access.clone(),
            frames: stack
                .frames
                .iter()
                .map(|frame| Frame {
                    function: frame.function.clone(),
                    file: frame.file.clone(),
                    line: frame.line,
                })
                .collect(),
        }
    }
}

pub struct ConcRace {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(ConcRace::new())));

impl ConcRace {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(4).expect("tier 4 exists"),
                cost: CostClass::Standard,
                needs: vec![FactKind::RaceReports],
                wants: vec![],
                findings: [RACE, UNDEFINED]
                    .iter()
                    .map(|said| kind::<Reported>(said.kind, ActionKind::FixBug, said.means))
                    .collect(),
                metrics: METRICS
                    .iter()
                    .map(|(name, counts)| metric(name, counts))
                    .collect(),
                settings: SETTINGS.specs(),
                stability: Stability::Experimental,
                summary: "Runs the tests under a race detector (Go's -race, Miri, \
                          ThreadSanitizer) and reports each data race and each undefined \
                          behavior it sees."
                    .to_owned(),
            },
        }
    }
}

impl Check for ConcRace {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        SETTINGS.plan_files(&self.manifest, ctx, scope)
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        SETTINGS.read(&self.manifest.id, &ctx.config)?;
        let fact = facts.get::<RaceReports>()?;
        let mut repeats: HashMap<String, usize> = HashMap::new();
        let (mut races, mut undefined) = (0u64, 0u64);
        for report in &fact.reports {
            ctx.cancel.checkpoint()?;
            match report.kind {
                RaceKind::DataRace => races += 1,
                RaceKind::UndefinedBehavior => undefined += 1,
            }
            let subject = subject(report);
            let repeat = repeats.entry(subject.clone()).or_default();
            *repeat += 1;
            let finding = self.finding(fact, report, format!("{subject}|{repeat}"));
            sink.emit(Event::Finding(Box::new(finding)));
        }
        Ok(self.result(fact, races, undefined))
    }

    /// Reports add up across languages.
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }

    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }
}

impl ConcRace {
    /// The language's result: a score and metrics only when some root's run completed, and a
    /// note for each that did not.
    fn result(&self, fact: &RaceReports, races: u64, undefined: u64) -> CheckResult {
        let ran = fact
            .runs
            .iter()
            .filter(|run| run.status == RunStatus::Ran)
            .count() as u64;
        #[allow(
            clippy::cast_precision_loss,
            reason = "report counts stay far below 2^52, where f64 is exact"
        )]
        let (races, undefined) = (races as f64, undefined as f64);
        let measured = ran > 0;
        let reports = fact.reports.len() as u64;
        let mut result = outcome::result(
            &self.manifest,
            outcome::status(reports > 0),
            measured.then(|| Score {
                value: races + undefined,
                unit: "reports".to_owned(),
                n: ran,
            }),
            if measured {
                BTreeMap::from([
                    ("races".to_owned(), races),
                    ("undefined_behavior".to_owned(), undefined),
                ])
            } else {
                BTreeMap::new()
            },
            reports,
        );
        result.notes = fact
            .runs
            .iter()
            .filter(|run| run.status == RunStatus::Failed)
            .map(|run| {
                format!(
                    "{} failed in {}: {}",
                    fact.tool,
                    run.dir,
                    run.detail.as_deref().unwrap_or("no detail")
                )
            })
            .collect();
        result
    }

    fn finding(&self, fact: &RaceReports, report: &RaceReport, subject: String) -> Finding {
        let first = report.stacks.iter().find_map(|stack| stack.frames.first());
        let location = first.map(|frame| Location {
            file: frame.file.clone(),
            line: frame.line,
            col: None,
            end_line: frame.line,
            end_col: None,
        });
        let repro = match &location {
            Some(at) => format!("cupel check --only {ID} --paths {}", at.file),
            None => format!("cupel check --only {ID}"),
        };
        let said = match report.kind {
            RaceKind::DataRace => RACE,
            RaceKind::UndefinedBehavior => UNDEFINED,
        };
        SubjectFinding {
            kind: said.kind,
            subject,
            severity: Severity::Fail,
            confidence: 1.0,
            title: title(report),
            summary: said.asks.to_owned(),
            evidence: evidence(&Reported {
                tool: fact.tool.to_string(),
                test: report.test.clone(),
                message: report.message.clone(),
                stacks: report.stacks.iter().map(Stack::from).collect(),
            }),
            location,
            repro,
        }
        .finding(&self.manifest)
    }
}

/// What a report is about, for its ID: its test, message, and the function of each stack's
/// first frame, none of which moves with a line.
fn subject(report: &RaceReport) -> String {
    let mut parts = vec![
        report.test.clone().unwrap_or_default(),
        report.message.clone(),
    ];
    parts.extend(
        report
            .stacks
            .iter()
            .map(|stack| innermost(stack).unwrap_or_default().to_owned()),
    );
    parts.join("|")
}

/// The function of a stack's first frame, when the detector named it.
fn innermost(stack: &RaceStack) -> Option<&str> {
    stack.frames.first()?.function.as_deref()
}

/// A race names its accesses and their functions (the detector's words are in the evidence);
/// undefined behavior its function and message.
fn title(report: &RaceReport) -> String {
    match report.kind {
        RaceKind::DataRace => {
            let accesses: Vec<String> = report
                .stacks
                .iter()
                .map(|stack| {
                    let access = stack.access.as_deref().unwrap_or("access");
                    match innermost(stack) {
                        Some(function) => format!("`{access}` in `{function}`"),
                        None => format!("`{access}` in an unnamed function"),
                    }
                })
                .collect();
            format!("data race: {}", accesses.join(" and "))
        }
        RaceKind::UndefinedBehavior => match report.stacks.first().and_then(innermost) {
            Some(function) => format!("undefined behavior in `{function}`: {}", report.message),
            None => format!("undefined behavior: {}", report.message),
        },
    }
}
