// ABOUTME: `gen.fuzz` (ADR-107): fuzz targets a fuzzer crashed or hung under the run's seed and execution budget, with
// ABOUTME: the input that reproduces it, and project roots the fuzzer searched that hold no fuzz target.
use std::collections::BTreeMap;

use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, FactKind,
    Facts, Finding, FuzzCrash, FuzzResults, FuzzStatus, FuzzTarget, Manifest, Plan, Result, Scope,
    Score, Severity, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::finding::SubjectFinding;
use crate::manifest::{evidence, kind, metric};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "gen.fuzz";

/// The check takes no settings; the execution budget is `--cases`.
const LIMITS: Limits = Limits::new(&[]);

/// Evidence of `fuzz.crash` and `fuzz.timeout`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Failing {
    /// The fuzz target, as the fuzzer names it.
    target: String,
    /// The fuzzer that ran it.
    tool: String,
    /// What the target reported when it failed.
    message: String,
    /// The failing input, as text when printable, else `hex:` and its bytes.
    input: String,
    /// The file holding the whole input, relative to the repository root.
    reproducer: Option<String>,
    /// The command that runs the target on it, from its project root.
    reproduce: Option<String>,
    /// The seed the fuzzer was given; null for a fuzzer that takes none.
    seed: Option<u64>,
    /// The executions the target was given.
    cases: u64,
}

/// Evidence of `fuzz.target-missing`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Missing {
    /// The project root, relative to the repository root.
    root: String,
    /// The fuzzer that searched it.
    tool: String,
}

pub struct GenFuzz {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(GenFuzz::new())));

impl GenFuzz {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(4).expect("tier 4 exists"),
                cost: CostClass::Deep,
                needs: vec![FactKind::FuzzResults],
                wants: vec![],
                findings: vec![
                    kind::<Failing>(
                        "fuzz.crash",
                        ActionKind::FixBug,
                        "An input the fuzzer generated crashed a fuzz target: a panic, an \
                         uncaught exception, a sanitizer error, or running out of memory.",
                    ),
                    kind::<Failing>(
                        "fuzz.timeout",
                        ActionKind::FixBug,
                        "An input ran a fuzz target past the fuzzer's per-input time limit: a \
                         hang, or a path slow enough to be one.",
                    ),
                    kind::<Missing>(
                        "fuzz.target-missing",
                        ActionKind::WriteTest,
                        "A project root the fuzzer searched holds no fuzz target.",
                    ),
                ],
                metrics: ["targets", "crashes", "timeouts", "missing"]
                    .into_iter()
                    .zip([
                        "Fuzz targets found.",
                        "Fuzz targets an input crashed.",
                        "Fuzz targets an input ran past the time limit.",
                        "Project roots with no fuzz target.",
                    ])
                    .map(|(name, summary)| metric(name, summary))
                    .collect(),
                stability: Stability::Experimental,
                summary: "Runs the existing fuzz targets (cargo-fuzz, Go native fuzzing, \
                          atheris, Jazzer.js) with the run's seed and an execution budget; \
                          reports crashes and hangs with the input that reproduces them, and \
                          project roots without a fuzz target."
                    .to_owned(),
            },
        }
    }
}

impl Check for GenFuzz {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// One target per file in scope; the fuzz run's size is the fuzzer's to know.
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
        let results = facts.get::<FuzzResults>()?;
        let failing: Vec<Finding> = results
            .targets
            .iter()
            .filter_map(|target| self.failing(results, target))
            .collect();
        let empty: Vec<&String> = results
            .roots
            .iter()
            .filter(|root| !results.targets.iter().any(|target| target.root == **root))
            .collect();
        let count = |status: FuzzStatus| {
            results
                .targets
                .iter()
                .filter(|target| target.status == status)
                .count()
        };
        let counts = [
            ("targets", results.targets.len()),
            ("crashes", count(FuzzStatus::Crashed)),
            ("timeouts", count(FuzzStatus::TimedOut)),
            ("missing", empty.len()),
        ];
        let reported = (failing.len() + empty.len()) as u64;
        let missing = empty.into_iter().map(|root| self.missing(results, root));
        for finding in failing.into_iter().chain(missing) {
            sink.emit(Event::Finding(Box::new(finding)));
        }
        let mut result = outcome::result(
            &self.manifest,
            outcome::status(counts[1].1 > 0),
            Some(score(counts[1].1 + counts[2].1, counts[0].1)),
            metrics(&counts),
            reported,
        );
        result.notes = results
            .targets
            .iter()
            .filter(|target| target.status == FuzzStatus::Failed)
            .map(|target| {
                format!(
                    "{} could not be fuzzed: {}",
                    target.name,
                    target.detail.as_deref().unwrap_or("no output")
                )
            })
            .collect();
        Ok(result)
    }

    /// Targets and findings add up across languages.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }

    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "target and root counts stay far below 2^52, where f64 is exact"
)]
fn metrics(counts: &[(&str, usize)]) -> BTreeMap<String, f64> {
    counts
        .iter()
        .map(|(name, count)| ((*name).to_owned(), *count as f64))
        .collect()
}

#[expect(
    clippy::cast_precision_loss,
    reason = "target counts stay far below 2^52, where f64 is exact"
)]
fn score(failing: usize, targets: usize) -> Score {
    Score {
        value: failing as f64,
        unit: "failing".to_owned(),
        n: targets as u64,
    }
}

/// How a title names a failing input.
fn named(input: &str) -> String {
    if input.is_empty() {
        "the empty input".to_owned()
    } else {
        format!("`{input}`")
    }
}

impl GenFuzz {
    /// `fuzz.crash` or `fuzz.timeout` for a target an input crashed or hung; `None` otherwise.
    fn failing(&self, run: &FuzzResults, target: &FuzzTarget) -> Option<Finding> {
        let crash = target.crash.as_ref()?;
        let (kind, severity, confidence, verb) = match target.status {
            FuzzStatus::Crashed => ("fuzz.crash", Severity::Fail, 1.0, "crashes on"),
            FuzzStatus::TimedOut => (
                "fuzz.timeout",
                Severity::Warn,
                0.7,
                "runs past its time limit on",
            ),
            FuzzStatus::Passed | FuzzStatus::Failed => return None,
        };
        let name = &target.name;
        let paths = target
            .location
            .as_ref()
            .map_or(target.root.as_str(), |at| at.file.as_str());
        let seed = run
            .seeded
            .then(|| format!(" --seed {}", run.seed))
            .unwrap_or_default();
        Some(
            SubjectFinding {
                kind,
                subject: format!("{}|{name}", target.root),
                severity,
                confidence,
                title: format!(
                    "fuzz target `{name}` {verb} {}: {}",
                    named(&crash.input),
                    crash.message
                ),
                summary: summary(name, crash),
                evidence: evidence(&Failing {
                    target: name.clone(),
                    tool: run.tool.to_string(),
                    message: crash.message.clone(),
                    input: crash.input.clone(),
                    reproducer: crash.reproducer.clone(),
                    reproduce: crash.reproduce.clone(),
                    seed: run.seeded.then_some(run.seed),
                    cases: run.cases,
                }),
                location: crash.location.clone().or_else(|| target.location.clone()),
                repro: format!(
                    "cupel check --only {ID} --paths {paths}{seed} --cases {}",
                    run.cases
                ),
            }
            .finding(&self.manifest),
        )
    }

    fn missing(&self, run: &FuzzResults, root: &str) -> Finding {
        SubjectFinding {
            kind: "fuzz.target-missing",
            subject: root.to_owned(),
            severity: Severity::Info,
            confidence: 1.0,
            title: format!("no fuzz target in `{root}`"),
            summary: format!(
                "Add a fuzz target `{}` runs in `{root}` for the code that reads untrusted \
                 input.",
                run.tool
            ),
            evidence: evidence(&Missing {
                root: root.to_owned(),
                tool: run.tool.to_string(),
            }),
            location: None,
            repro: format!("cupel check --only {ID} --paths {root}"),
        }
        .finding(&self.manifest)
    }
}

/// What a failing target's action asks: fix the failure the reproducer shows.
fn summary(name: &str, crash: &FuzzCrash) -> String {
    let rerun = crash.reproduce.as_ref().map_or_else(
        || " and rerun the target".to_owned(),
        |command| format!(" (`{command}` reruns it)"),
    );
    format!(
        "Fix `{name}`'s failure on the saved input{rerun}, then keep the input in the target's \
         corpus as a regression case."
    )
}
