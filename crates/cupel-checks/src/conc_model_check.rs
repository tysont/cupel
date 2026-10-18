// ABOUTME: `conc.model-check` (ADR-117): properties a bounded model checker refuted in the project's proof harnesses,
// ABOUTME: from the ModelCheckResults fact, one failing finding per refuted property with its counterexample.
use std::collections::BTreeMap;

use cupel_core::{
    ActionKind, Check, CheckCtx, CheckResult, CostClass, Event, EventSink, FactKind, Facts,
    Finding, HarnessStatus, Manifest, ModelCheckResults, ModelHarness, Plan, RefutedProperty,
    Result, RunStatus, Scope, Score, Severity,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::finding::SubjectFinding;
use crate::manifest::{Declared, evidence, kind};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "conc.model-check";

/// The check takes no settings: a harness's bounds are the harness's own.
const LIMITS: Limits = Limits::new(&[]);

/// The metrics, each with what it counts.
const METRICS: [(&str, &str); 4] = [
    ("harnesses", "Proof harnesses the model checker checked."),
    (
        "verified",
        "Harnesses whose properties hold for every input within their bounds.",
    ),
    (
        "failed",
        "Harnesses some input of which refutes a property.",
    ),
    (
        "undetermined",
        "Harnesses the model checker could not decide.",
    ),
];

/// Evidence of `model.counterexample`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Refuted {
    /// The model checker, such as `tool.kani`.
    tool: String,
    /// The proof harness, as the checker names it.
    harness: String,
    /// The property refuted, in the checker's words.
    description: String,
    /// The function it fails in, as the checker names it.
    function: Option<String>,
    /// The values the harness drew that refute it, in order; empty when the checker gave none.
    values: Vec<String>,
    /// A unit test that replays them, when the checker wrote one.
    replay: Option<String>,
}

const DECLARED: Declared = Declared {
    id: ID,
    tier: 5,
    cost: CostClass::Exhaustive,
    needs: FactKind::ModelCheckResults,
    findings: || {
        vec![kind::<Refuted>(
            "model.counterexample",
            ActionKind::FixBug,
            "A bounded model checker found inputs to a proof harness that break a property: an \
             assertion, an arithmetic overflow, an out-of-bounds access, a panic.",
        )]
    },
    metrics: &METRICS,
    summary: "Model-checks the project's proof harnesses (Kani's `#[kani::proof]`) over every \
              input within their bounds and reports each property refuted, with the inputs that \
              refute it.",
};

pub struct ConcModelCheck {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(ConcModelCheck::new())));

impl ConcModelCheck {
    pub fn new() -> Self {
        Self {
            manifest: DECLARED.manifest(),
        }
    }
}

impl Check for ConcModelCheck {
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
        let fact = facts.get::<ModelCheckResults>()?;
        let mut reported = 0u64;
        for harness in &fact.harnesses {
            for property in &harness.refuted {
                let finding = self.finding(fact, harness, property);
                sink.emit(Event::Finding(Box::new(finding)));
                reported += 1;
            }
        }
        let count = |status: HarnessStatus| {
            outcome::tally(&fact.harnesses, |harness| harness.status, &status)
        };
        let failed = count(HarnessStatus::Failed);
        let measured = fact.runs.iter().any(|run| run.status == RunStatus::Ran);
        let mut result = outcome::result(
            &self.manifest,
            outcome::status(reported > 0),
            measured.then(|| outcome::counted(failed, "failing", fact.harnesses.len())),
            if measured {
                outcome::counts(&[
                    ("harnesses", fact.harnesses.len()),
                    ("verified", count(HarnessStatus::Verified)),
                    ("failed", failed),
                    ("undetermined", count(HarnessStatus::Undetermined)),
                ])
            } else {
                BTreeMap::new()
            },
            reported,
        );
        result.notes = notes(fact);
        Ok(result)
    }

    /// Harnesses and refutations add up across languages.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }

    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }
}

/// A note for each root whose run failed and each harness left undecided.
fn notes(fact: &ModelCheckResults) -> Vec<String> {
    let failed = fact
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
        });
    let undecided = fact
        .harnesses
        .iter()
        .filter(|harness| harness.status == HarnessStatus::Undetermined)
        .map(|harness| {
            format!(
                "harness `{}` is undetermined: the model checker could not decide it",
                harness.name
            )
        });
    failed.chain(undecided).collect()
}

impl ConcModelCheck {
    fn finding(
        &self,
        fact: &ModelCheckResults,
        harness: &ModelHarness,
        property: &RefutedProperty,
    ) -> Finding {
        let values = property
            .counterexample
            .as_ref()
            .map(|counterexample| counterexample.values.clone())
            .unwrap_or_default();
        let mut title = format!(
            "harness `{}` refutes `{}`",
            harness.name, property.description
        );
        if let Some(function) = &property.function {
            title.push_str(&format!(" in `{function}`"));
        }
        if !values.is_empty() {
            title.push_str(&format!(" with {}", values.join(", ")));
        }
        let location = property
            .location
            .clone()
            .or_else(|| harness.location.clone());
        let paths = location
            .as_ref()
            .map_or_else(|| harness.root.clone(), |at| at.file.clone());
        let function = property.function.as_deref().unwrap_or("the code");
        SubjectFinding {
            kind: "model.counterexample",
            subject: format!("{}|{}|{}", harness.root, harness.name, property.description),
            severity: Severity::Fail,
            confidence: 1.0,
            title,
            summary: format!(
                "Fix `{function}` so the property holds for the inputs the model checker found \
                 (the replay test reproduces them), then check the harness again."
            ),
            evidence: evidence(&Refuted {
                tool: fact.tool.to_string(),
                harness: harness.name.clone(),
                description: property.description.clone(),
                function: property.function.clone(),
                values,
                replay: property
                    .counterexample
                    .as_ref()
                    .and_then(|counterexample| counterexample.replay.clone()),
            }),
            location,
            repro: format!("cupel check --only {ID} --paths {paths}"),
        }
        .finding(&self.manifest)
    }
}
