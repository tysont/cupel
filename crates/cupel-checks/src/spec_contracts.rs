// ABOUTME: `spec.contracts`: preconditions, postconditions, and invariants tests violated with contract checking on, from
// ABOUTME: the ContractViolations fact, one failing finding per contract and test, identified without lines (ADR-113).
use std::collections::{BTreeMap, HashMap};

use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, ContractKind, ContractViolation,
    ContractViolations, CostClass, Event, EventSink, FactKind, Facts, Finding, Location, Manifest,
    Plan, Result, RunStatus, Scope, Score, Severity, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::finding::SubjectFinding;
use crate::manifest::{evidence, kind, metric, spelling};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "spec.contracts";

const KIND: &str = "contract.violated";

/// The check has no settings: which contracts run is the project's, which library reads them
/// the language's.
const SETTINGS: Limits = Limits::new(&[]);

/// Evidence of `contract.violated`: the violation as the fact holds it.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Violated {
    /// The producer that read the library's report, such as `tool.icontract`.
    tool: String,
    /// `precondition`, `postcondition`, or `invariant`.
    kind: String,
    /// The function, method, or type the contract is declared on, when the library says.
    function: Option<String>,
    /// The library's message: the contract's description and condition, with the values it
    /// saw when it prints them.
    message: String,
    /// The runner's ID of the test that violated it.
    test: String,
}

pub struct SpecContracts {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(SpecContracts::new())));

impl SpecContracts {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(5).expect("tier 5 exists"),
                cost: CostClass::Standard,
                needs: vec![FactKind::ContractViolations],
                wants: vec![],
                findings: vec![kind::<Violated>(
                    KIND,
                    ActionKind::FixBug,
                    "A test violated a precondition, postcondition, or invariant while the tests \
                     ran with every contract checked and generated inputs at the case budget.",
                )],
                metrics: vec![metric(
                    "violations",
                    "Contracts violated, counted once per test that violated each.",
                )],
                settings: SETTINGS.specs(),
                stability: Stability::Experimental,
                summary: "Runs the tests again with every contract the project's contract library \
                          checks turned on (debug-only and slow ones too) and properties at the \
                          case budget, and reports each precondition, postcondition, and \
                          invariant a test violated."
                    .to_owned(),
            },
        }
    }
}

impl Check for SpecContracts {
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
        let fact = facts.get::<ContractViolations>()?;
        let mut repeats: HashMap<String, usize> = HashMap::new();
        for violation in &fact.violations {
            ctx.cancel.checkpoint()?;
            let subject = subject(violation);
            let repeat = repeats.entry(subject.clone()).or_default();
            *repeat += 1;
            let subject = if *repeat == 1 {
                subject
            } else {
                format!("{subject}|{repeat}")
            };
            let finding = self.finding(fact, violation, subject);
            sink.emit(Event::Finding(Box::new(finding)));
        }
        Ok(self.result(fact))
    }

    /// Violations add up across languages.
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }

    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }
}

impl SpecContracts {
    /// The language's result: a score and metrics only when the library is declared and some
    /// root's contract run completed; a note for each that did not, and for an undeclared
    /// library.
    fn result(&self, fact: &ContractViolations) -> CheckResult {
        let ran = fact.runs.iter().any(|run| run.status == RunStatus::Ran);
        let measured = fact.declared && ran;
        let count = fact.violations.len() as u64;
        #[allow(
            clippy::cast_precision_loss,
            reason = "violation counts stay far below 2^52, where f64 is exact"
        )]
        let value = count as f64;
        let mut result = outcome::result(
            &self.manifest,
            outcome::status(count > 0),
            measured.then(|| Score {
                value,
                unit: "violations".to_owned(),
                n: fact.tests,
            }),
            if measured {
                BTreeMap::from([("violations".to_owned(), value)])
            } else {
                BTreeMap::new()
            },
            count,
        );
        result.notes = if fact.declared {
            fact.runs
                .iter()
                .filter(|run| run.status == RunStatus::Failed)
                .map(|run| {
                    format!(
                        "{}: the contract run of {} failed in {}: {}",
                        fact.tool,
                        fact.runner,
                        run.dir,
                        run.detail.as_deref().unwrap_or("no detail")
                    )
                })
                .collect()
        } else {
            vec![format!(
                "{}: no project root declares the contract library it reads, so no contract \
                 was checked",
                fact.tool
            )]
        };
        result
    }

    fn finding(
        &self,
        fact: &ContractViolations,
        violation: &ContractViolation,
        subject: String,
    ) -> Finding {
        let location = violation.place.as_ref().map(|place| Location {
            file: place.file.clone(),
            line: place.line,
            col: None,
            end_line: place.line,
            end_col: None,
        });
        let repro = match &location {
            Some(at) => format!("cupel check --only {ID} --paths {}", at.file),
            None => format!("cupel check --only {ID}"),
        };
        SubjectFinding {
            kind: KIND,
            subject,
            severity: Severity::Fail,
            confidence: 1.0,
            title: title(violation),
            summary: asks(violation.kind).to_owned(),
            evidence: evidence(&Violated {
                tool: fact.tool.to_string(),
                kind: spelling(&violation.kind),
                function: violation.function.clone(),
                message: violation.message.clone(),
                test: violation.test.clone(),
            }),
            location,
            repro,
        }
        .finding(&self.manifest)
    }
}

/// What a violation is about, for its ID: its kind, function, message, and test, none of
/// which moves with a line.
fn subject(violation: &ContractViolation) -> String {
    [
        spelling(&violation.kind),
        violation.function.clone().unwrap_or_default(),
        violation.message.clone(),
        violation.test.clone(),
    ]
    .join("|")
}

/// `<kind> of `<function>` violated in `<test>`: <message>`.
fn title(violation: &ContractViolation) -> String {
    let kind = spelling(&violation.kind);
    let of = violation
        .function
        .as_ref()
        .map_or_else(String::new, |function| format!(" of `{function}`"));
    format!(
        "{kind}{of} violated in `{}`: {}",
        violation.test, violation.message
    )
}

/// What the action asks, by whose obligation the contract was.
fn asks(kind: ContractKind) -> &'static str {
    match kind {
        ContractKind::Precondition => {
            "A caller passed what the precondition rejects: fix the call the test makes, or the \
             precondition if it is wrong, then rerun."
        }
        ContractKind::Postcondition => {
            "The function returned what its postcondition rejects: fix the function, or the \
             postcondition if it is wrong, then rerun."
        }
        ContractKind::Invariant => {
            "A method left the value in a state its invariant rejects: fix the method, or the \
             invariant if it is wrong, then rerun."
        }
    }
}
