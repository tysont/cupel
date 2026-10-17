// ABOUTME: `perf.bench`: each benchmark of BenchSamples measured at head and at the range's base, compared by a
// ABOUTME: two-sided Mann-Whitney U test; significant changes past the threshold are regressions or improvements (ADR-106).
mod mann_whitney;

use std::collections::BTreeMap;

use cupel_core::{
    ActionKind, BenchSamples, Benchmark, Check, CheckCtx, CheckId, CheckResult, CostClass, Event,
    EventSink, FactKind, Facts, Finding, Location, Manifest, Plan, Result, RootRun, RunStatus,
    Scope, Score, Severity, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::finding::SubjectFinding;
use crate::manifest::{evidence, kind, metric};
use crate::metrics::{largest, sum};
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "perf.bench";

/// `threshold_percent`: the smallest change, in percent of the base's median, a finding reports;
/// `significance_percent`: the test's level, in percent (ADR-106).
const SETTINGS: Limits = Limits::new(&[
    (
        "threshold_percent",
        5,
        "The smallest change of a benchmark's median, in percent of the base's, that is reported.",
    ),
    (
        "significance_percent",
        5,
        "The Mann-Whitney U test's level, in percent: a change with a lower p is significant.",
    ),
]);

/// Evidence of `perf.regression` and `perf.improvement`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Compared {
    /// The benchmark, as the bench tool names it.
    benchmark: String,
    /// What a sample measures per iteration: `ns`, `bytes`, or `allocs`.
    unit: String,
    base_median: f64,
    head_median: f64,
    /// (head − base) / base, the medians' relative change; 1 when the base's median is 0.
    change: f64,
    /// The two-sided Mann-Whitney U test's p-value.
    p_value: f64,
    base_samples: u64,
    head_samples: u64,
    /// The level the p-value is held to.
    significance: f64,
    /// The smallest relative change reported.
    threshold: f64,
}

pub struct PerfBench {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(PerfBench::new())));

impl PerfBench {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(0).expect("tier 0 exists"),
                cost: CostClass::Deep,
                needs: vec![FactKind::BenchSamples],
                wants: vec![],
                findings: vec![
                    kind::<Compared>(
                        "perf.regression",
                        ActionKind::Review,
                        "A benchmark is significantly slower, or allocates significantly more, \
                         than at the range's base commit, by at least the threshold.",
                    ),
                    kind::<Compared>(
                        "perf.improvement",
                        ActionKind::Review,
                        "A benchmark is significantly faster, or allocates significantly less, \
                         than at the range's base commit, by at least the threshold.",
                    ),
                ],
                metrics: vec![
                    metric("benchmarks", "Benchmarks measured at head."),
                    metric(
                        "compared",
                        "Benchmarks measured at head and at the base commit.",
                    ),
                    metric(
                        "regression",
                        "The largest significant slowdown, as a fraction of the base's median \
                         (0.05 is 5%); 0 when none or without a base.",
                    ),
                    metric("regressed", "Benchmarks reported as regressions."),
                    metric("improved", "Benchmarks reported as improvements."),
                ],
                settings: SETTINGS.specs(),
                stability: Stability::Experimental,
                summary: "Runs each language's benchmarks at head and at the range's base commit \
                          and compares their samples with a Mann-Whitney U test."
                    .to_owned(),
            },
        }
    }
}

impl Check for PerfBench {
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
        let settings = SETTINGS.read(&self.manifest.id, &ctx.config)?;
        let levels = Levels {
            threshold: f64::from(settings["threshold_percent"]) / 100.0,
            significance: f64::from(settings["significance_percent"]) / 100.0,
        };
        let samples = facts.get::<BenchSamples>()?;
        let mut tally = Tally::default();
        if let Some(base) = &samples.base {
            for head in &samples.benchmarks {
                ctx.cancel.checkpoint()?;
                let Some(before) = base.benchmarks.iter().find(|b| b.key() == head.key()) else {
                    continue;
                };
                if let Some(finding) = tally.compare(self, &levels, (before, head), &base.commit) {
                    sink.emit(Event::Finding(Box::new(finding)));
                }
            }
        }
        Ok(tally.result(&self.manifest, samples))
    }

    /// The regression is the largest language's; counts add up.
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        if name == "regression" {
            largest(name, results)
        } else {
            sum(name, results)
        }
    }

    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::maximum(scores)
    }
}

/// The levels changes are held to.
struct Levels {
    /// The smallest relative change reported.
    threshold: f64,
    /// The p-value a change must be below.
    significance: f64,
}

/// What comparing a language's benchmarks found so far.
#[derive(Default)]
struct Tally {
    compared: u64,
    regressed: u64,
    improved: u64,
    /// The largest significant slowdown.
    regression: f64,
}

impl Tally {
    /// Compares one benchmark's `(base, head)` samples: counted, and its finding when the change
    /// is significant and at least the threshold.
    fn compare(
        &mut self,
        check: &PerfBench,
        levels: &Levels,
        (base, head): (&Benchmark, &Benchmark),
        commit: &str,
    ) -> Option<Finding> {
        let (before, after) = (finite(&base.samples), finite(&head.samples));
        if before.is_empty() || after.is_empty() {
            return None;
        }
        self.compared += 1;
        let p_value = mann_whitney::p_value(&before, &after);
        let (base_median, head_median) = (median(&before), median(&after));
        let change = relative_change(base_median, head_median);
        if p_value >= levels.significance {
            return None;
        }
        self.regression = self.regression.max(change);
        if change.abs() < levels.threshold {
            return None;
        }
        let regressed = change > 0.0;
        if regressed {
            self.regressed += 1;
        } else {
            self.improved += 1;
        }
        let compared = Compared {
            benchmark: head.name.clone(),
            unit: head.unit.clone(),
            base_median,
            head_median,
            change,
            p_value,
            base_samples: before.len() as u64,
            head_samples: after.len() as u64,
            significance: levels.significance,
            threshold: levels.threshold,
        };
        Some(check.finding(head, &compared, regressed, commit))
    }

    fn result(self, manifest: &Manifest, samples: &BenchSamples) -> CheckResult {
        #[allow(
            clippy::cast_precision_loss,
            reason = "benchmark counts stay far below 2^52, where f64 is exact"
        )]
        let metrics = BTreeMap::from([
            ("benchmarks".to_owned(), samples.benchmarks.len() as f64),
            ("compared".to_owned(), self.compared as f64),
            ("regression".to_owned(), self.regression),
            ("regressed".to_owned(), self.regressed as f64),
            ("improved".to_owned(), self.improved as f64),
        ]);
        let mut result = outcome::result(
            manifest,
            outcome::status(self.regressed > 0),
            Some(Score {
                value: self.regression,
                unit: "regression".to_owned(),
                n: self.compared,
            }),
            metrics,
            self.regressed + self.improved,
        );
        result.notes = notes(samples);
        result
    }
}

impl PerfBench {
    fn finding(
        &self,
        head: &Benchmark,
        compared: &Compared,
        regressed: bool,
        commit: &str,
    ) -> Finding {
        let (kind, severity, verb) = if regressed {
            ("perf.regression", Severity::Warn, "regressed")
        } else {
            ("perf.improvement", Severity::Info, "improved")
        };
        let percent = compared.change.abs() * 100.0;
        SubjectFinding {
            kind,
            subject: format!("{}|{}", head.name, head.unit),
            severity,
            confidence: 1.0 - compared.p_value,
            title: format!(
                "`{}` {verb} {percent:.1}% in {} (p = {:.4})",
                head.name, head.unit, compared.p_value
            ),
            summary: if regressed {
                format!(
                    "Find what made `{}` cost more {} than at {commit} and remove it, or accept \
                     the cost and suppress this finding.",
                    head.name, head.unit
                )
            } else {
                format!("Keep what made `{}` cheaper.", head.name)
            },
            evidence: evidence(compared),
            location: head.file.as_ref().map(|file| {
                let line = head.line.unwrap_or(1);
                Location {
                    file: file.clone(),
                    line,
                    col: None,
                    end_line: line,
                    end_col: None,
                }
            }),
            repro: format!("cupel check --only {ID} --base {commit}"),
        }
        .finding(&self.manifest)
    }
}

/// The samples that are numbers.
fn finite(samples: &[f64]) -> Vec<f64> {
    samples.iter().copied().filter(|s| s.is_finite()).collect()
}

/// The median of non-empty `values`.
fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        f64::midpoint(sorted[middle - 1], sorted[middle])
    } else {
        sorted[middle]
    }
}

/// `(head − base) / base`; a base of 0 counts any growth as 1 and none as 0.
fn relative_change(base: f64, head: f64) -> f64 {
    if base == 0.0 {
        if head > 0.0 { 1.0 } else { 0.0 }
    } else {
        (head - base) / base
    }
}

/// What the comparison could not cover: no base, failed runs on either side, and benchmarks
/// measured on one side only.
fn notes(samples: &BenchSamples) -> Vec<String> {
    let tool = &samples.tool;
    let failed = |runs: &[RootRun], side: &str| -> Vec<String> {
        runs.iter()
            .filter(|run| run.status == RunStatus::Failed)
            .map(|run| {
                let detail = run.detail.as_deref().unwrap_or("no detail");
                format!("{tool} failed in {}{side}: {detail}", run.dir)
            })
            .collect()
    };
    let mut notes = failed(&samples.runs, "");
    let Some(base) = &samples.base else {
        notes.push("no base to compare with: run under --changed or --base <commit>".to_owned());
        return notes;
    };
    notes.extend(failed(
        &base.runs,
        &format!(" at the base commit {}", base.commit),
    ));
    let only = |side: &str, these: &[Benchmark], those: &[Benchmark]| -> Vec<String> {
        these
            .iter()
            .filter(|b| !those.iter().any(|other| other.key() == b.key()))
            .map(|b| format!("measured at {side} only: {} ({})", b.name, b.unit))
            .collect()
    };
    notes.extend(only("head", &samples.benchmarks, &base.benchmarks));
    notes.extend(only("the base", &base.benchmarks, &samples.benchmarks));
    notes
}
