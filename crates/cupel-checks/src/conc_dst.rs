// ABOUTME: `conc.dst` (ADR-120): deterministic simulation harnesses a seed of the run's range failed, each with that seed,
// ABOUTME: the trace it printed, and the command that reruns it alone; roots without a harness and failed runs as notes.

use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, FactKind,
    Facts, Finding, Manifest, Plan, Result, RunStatus, Scope, Score, Severity, SimulationFailure,
    SimulationHarness, SimulationResults, SimulationStatus, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::finding::SubjectFinding;
use crate::manifest::{evidence, kind, metric};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "conc.dst";

/// The check takes no settings; the seed range is `--seed` and `--cases`.
const LIMITS: Limits = Limits::new(&[]);

/// Evidence of `dst.failure`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Failed {
    /// The harness, as the test runner names it.
    harness: String,
    /// The simulator that ran it.
    tool: String,
    /// The failing seed; null when the harness failed outside any one seed.
    seed: Option<u64>,
    /// The first seed of the range the harness was given.
    first_seed: u64,
    /// How many seeds the range holds.
    seeds: u64,
    /// What the harness reported.
    message: String,
    /// What it printed for the failing seed.
    trace: Vec<String>,
    /// Whether replaying the seed alone failed again.
    reproduced: bool,
    /// The command that reruns the failing seed alone, from the project root.
    reproduce: String,
}

pub struct ConcDst {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(ConcDst::new())));

impl ConcDst {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(4).expect("tier 4 exists"),
                cost: CostClass::Exhaustive,
                needs: vec![FactKind::SimulationResults],
                wants: vec![],
                findings: vec![kind::<Failed>(
                    "dst.failure",
                    ActionKind::FixBug,
                    "A deterministic simulation harness failed at a seed of the run's range: the \
                     seed replays the same schedule, faults, and timings, so it reproduces the \
                     failure exactly.",
                )],
                metrics: ["harnesses", "failures"]
                    .into_iter()
                    .zip([
                        "Simulation harnesses found.",
                        "Harnesses a seed of the range failed.",
                    ])
                    .map(|(name, summary)| metric(name, summary))
                    .collect(),
                settings: LIMITS.specs(),
                stability: Stability::Experimental,
                summary: "Runs the deterministic simulation harnesses (madsim) over a range of \
                          seeds from the run's seed (`--seed`, `--cases`); reports the first \
                          failing seed of each with its trace and the command that replays it."
                    .to_owned(),
            },
        }
    }
}

impl Check for ConcDst {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// One target per file in scope; the campaign's size is the seed range's.
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
        let results = facts.get::<SimulationResults>()?;
        let findings: Vec<Finding> = results
            .harnesses
            .iter()
            .filter(|harness| harness.status == SimulationStatus::Failed)
            .filter_map(|harness| {
                let failure = harness.failure.as_ref()?;
                Some(self.failed(results, harness, failure))
            })
            .collect();
        let counts = [
            ("harnesses", results.harnesses.len()),
            ("failures", findings.len()),
        ];
        let reported = findings.len() as u64;
        for finding in findings {
            sink.emit(Event::Finding(Box::new(finding)));
        }
        let mut result = outcome::result(
            &self.manifest,
            outcome::status(reported > 0),
            Some(outcome::counted(counts[1].1, "failing", counts[0].1)),
            outcome::counts(&counts),
            reported,
        );
        result.notes = unsimulated(results);
        Ok(result)
    }

    /// Harnesses and failures add up across languages.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }

    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }
}

/// A note for each root whose run failed, and each that ran but holds no harness.
fn unsimulated(results: &SimulationResults) -> Vec<String> {
    let tool = &results.tool;
    results
        .runs
        .iter()
        .filter_map(|run| match run.status {
            RunStatus::Failed => Some(format!(
                "{tool} failed in {}: {}",
                run.dir,
                run.detail.as_deref().unwrap_or("no output")
            )),
            RunStatus::Ran => (!results
                .harnesses
                .iter()
                .any(|harness| harness.root == run.dir))
            .then(|| format!("{tool} found no simulation harness in {}", run.dir)),
        })
        .collect()
}

impl ConcDst {
    /// `dst.failure` for a harness a seed failed.
    fn failed(
        &self,
        results: &SimulationResults,
        harness: &SimulationHarness,
        failure: &SimulationFailure,
    ) -> Finding {
        let name = &harness.name;
        let paths = harness
            .location
            .as_ref()
            .map_or(harness.root.as_str(), |at| at.file.as_str());
        let (at_seed, rerun) = match failure.seed {
            Some(seed) => (
                format!(" at seed {seed}"),
                format!("--seed {seed} --cases 1"),
            ),
            None => (
                String::new(),
                format!("--seed {} --cases {}", results.seed, results.seeds),
            ),
        };
        SubjectFinding {
            kind: "dst.failure",
            subject: format!("{}|{name}", harness.root),
            severity: Severity::Fail,
            confidence: if failure.reproduced { 1.0 } else { 0.5 },
            title: format!("simulation `{name}` fails{at_seed}: {}", failure.message),
            summary: format!(
                "Fix the failure `{name}` shows{at_seed} (`{}` replays it from `{}`), then keep \
                 the seed as a regression case of the harness.",
                failure.reproduce, harness.root
            ),
            evidence: evidence(&Failed {
                harness: name.clone(),
                tool: results.tool.to_string(),
                seed: failure.seed,
                first_seed: results.seed,
                seeds: results.seeds,
                message: failure.message.clone(),
                trace: failure.trace.clone(),
                reproduced: failure.reproduced,
                reproduce: failure.reproduce.clone(),
            }),
            location: failure
                .location
                .clone()
                .or_else(|| harness.location.clone()),
            repro: format!("cupel check --only {ID} --paths {paths} {rerun}"),
        }
        .finding(&self.manifest)
    }
}
