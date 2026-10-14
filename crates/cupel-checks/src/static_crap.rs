// ABOUTME: `static.crap`: per-symbol CRAP, `CC² × (1 − coverage)³ + CC`, from Symbols, Decisions, NormTree, and
// ABOUTME: a CoverageMap; without coverage CRAP is null and nothing is reported (ADR-048, ADR-067).
use std::collections::{BTreeMap, HashMap};

use cupel_core::coverage::CoverageMap;
use cupel_core::syntax::{
    Decision, Decisions, NormTree, SymbolDef, SymbolKey, Symbols, TestConventions,
};
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CheckStatus, CostClass, Event, EventSink,
    FactKind, Facts, Finding, FindingKindSpec, Manifest, Measurement, MetricPer, MetricSpec, Plan,
    Result, Scope, Score, Severity, Stability, SymbolHash, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::finding::SymbolFinding;
use crate::manifest::{kind, metric, ranked};
use crate::metrics::{
    cyclomatic, decisions_by_symbol, hashes_by_symbol, largest, scored, symbol_target,
    weighted_mean,
};
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "static.crap";

/// The settings and their defaults (ADR-048). A symbol whose CRAP is at least `high` is
/// `crap.high`; at least `watch` but below `high`, `crap.watch`.
const SETTINGS: Limits = Limits::new(&[("high", 30), ("watch", 5)]);

/// Evidence of `crap.high` and `crap.watch`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CrapScore {
    /// `cyclomatic² × (1 − coverage)³ + cyclomatic`.
    crap: f64,
    /// 1 plus the cyclomatic weight of every decision.
    cyclomatic: u32,
    /// The fraction of the symbol's instrumented lines that tests ran.
    coverage: f64,
    watch: u32,
    high: u32,
}

pub struct StaticCrap {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(StaticCrap::new())));

impl StaticCrap {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 2,
                tier: Tier::try_from(0).expect("tier 0 exists"),
                cost: CostClass::Quick,
                needs: vec![FactKind::Symbols, FactKind::Decisions, FactKind::NormTree],
                wants: vec![FactKind::CoverageMap, FactKind::TestConventions],
                findings: finding_kinds(),
                metrics: metrics(),
                stability: Stability::Experimental,
                summary: "CRAP per function: cyclomatic complexity weighted by missing test \
                          coverage. Limitation: CRAP exists only where coverage was measured; \
                          without a coverage map (no coverage producer exists until P3.2) the \
                          check passes with no score, no metrics, and no findings, and a function \
                          with no instrumented line is not scored. `static.complexity` reports \
                          cyclomatic complexity alone."
                    .to_owned(),
            },
        }
    }
}

/// The finding kinds this check reports, with their actions and evidence.
fn finding_kinds() -> Vec<FindingKindSpec> {
    vec![
        kind::<CrapScore>(
            "crap.high",
            ActionKind::WriteTest,
            "CRAP is at least high (default 30): complex code that tests do not cover.",
        ),
        kind::<CrapScore>(
            "crap.watch",
            ActionKind::Review,
            "CRAP is at least watch (default 5) but below high.",
        ),
    ]
}

/// The metrics this check reports: gate keys, then what `survey` ranks.
fn metrics() -> Vec<MetricSpec> {
    vec![
        metric("max", "The largest CRAP score in scope."),
        metric("mean", "The mean CRAP score over the symbols in scope."),
        ranked("crap", MetricPer::Symbol, "CRAP per function."),
    ]
}

impl Check for StaticCrap {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        SETTINGS.read(&self.manifest.id, &ctx.config)?;
        Ok(outcome::plan_symbols(&self.manifest, scope))
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        let limits = SETTINGS.read(&self.manifest.id, &ctx.config)?;
        // Without coverage CRAP is undefined, not CC alone (ADR-067).
        let Ok(coverage) = facts.get::<CoverageMap>() else {
            return Ok(outcome::result(
                &self.manifest,
                CheckStatus::Pass,
                None,
                BTreeMap::new(),
                0,
            ));
        };
        let inputs = Inputs {
            coverage,
            conventions: facts.get::<TestConventions>().ok(),
            decisions: decisions_by_symbol(facts.get::<Decisions>()?),
            hashes: hashes_by_symbol(facts.get::<NormTree>()?),
            bands: Bands::of(&limits),
        };
        let mut tally = Tally::default();
        // Test code is what lowers CRAP, not what it scores (ADR-048).
        for symbol in facts
            .get::<Symbols>()?
            .symbols
            .iter()
            .filter(|symbol| ctx.scope.includes(&symbol.key()))
            .filter(|symbol| !inputs.conventions.is_some_and(|c| c.is_test(&symbol.file)))
        {
            ctx.cancel.checkpoint()?;
            if let Some(finding) = self.score(symbol, &inputs, &mut tally, sink) {
                tally.report(&finding);
                sink.emit(Event::Finding(Box::new(finding)));
            }
        }
        Ok(tally.result(&self.manifest))
    }

    /// The largest value, over the symbols of every language (ADR-048).
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::maximum(scores)
    }

    /// `max` is the largest; `mean` is the mean over every language's scored symbols
    /// (ADR-063).
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        match name {
            "mean" => weighted_mean(name, results, scored),
            _ => largest(name, results),
        }
    }
}

/// What scoring reads besides the symbols.
struct Inputs<'a> {
    coverage: &'a CoverageMap,
    conventions: Option<&'a TestConventions>,
    decisions: HashMap<&'a SymbolKey, &'a [Decision]>,
    hashes: HashMap<&'a SymbolKey, &'a SymbolHash>,
    bands: Bands,
}

/// What the scored symbols added up to.
#[derive(Default)]
struct Tally {
    scored: u64,
    worst: f64,
    total: f64,
    findings: u64,
    /// Some symbol is `crap.high`.
    flagged: bool,
}

impl Tally {
    fn add(&mut self, crap: f64) {
        self.scored += 1;
        self.worst = self.worst.max(crap);
        self.total += crap;
    }

    fn report(&mut self, finding: &Finding) {
        self.findings += 1;
        self.flagged |= finding.severity != Severity::Info;
    }

    /// The language's result: `fail` with a `crap.high`, scored by the largest CRAP.
    fn result(&self, manifest: &Manifest) -> CheckResult {
        let score = Score {
            value: self.worst,
            unit: "max".to_owned(),
            n: self.scored,
        };
        let metrics = BTreeMap::from([
            ("max".to_owned(), self.worst),
            ("mean".to_owned(), self.mean()),
        ]);
        outcome::result(
            manifest,
            outcome::status(self.flagged),
            Some(score),
            metrics,
            self.findings,
        )
    }

    /// The mean CRAP of the scored symbols; 0 for none.
    #[allow(
        clippy::cast_precision_loss,
        reason = "symbol counts stay far below 2^52, where f64 is exact"
    )]
    fn mean(&self) -> f64 {
        if self.scored == 0 {
            0.0
        } else {
            self.total / self.scored as f64
        }
    }
}

/// The CRAP thresholds in force.
struct Bands {
    watch: u32,
    high: u32,
}

impl Bands {
    fn of(limits: &BTreeMap<&'static str, u32>) -> Self {
        let get = |name| limits.get(name).copied().unwrap_or(u32::MAX);
        Self {
            watch: get("watch"),
            high: get("high"),
        }
    }
}

/// The symbol a finding is about.
#[derive(Clone, Copy)]
struct Subject<'a> {
    symbol: &'a SymbolDef,
    hash: &'a SymbolHash,
}

/// `cyclomatic² × (1 − coverage)³ + cyclomatic`.
fn crap(cyclomatic: u32, coverage: f64) -> f64 {
    let cyclomatic = f64::from(cyclomatic);
    let uncovered = 1.0 - coverage;
    cyclomatic * cyclomatic * uncovered.powi(3) + cyclomatic
}

impl StaticCrap {
    /// Scores `symbol` when coverage measured it, measuring its CRAP; its finding, if any.
    fn score(
        &self,
        symbol: &SymbolDef,
        inputs: &Inputs<'_>,
        tally: &mut Tally,
        sink: &mut dyn EventSink,
    ) -> Option<Finding> {
        let key = symbol.key();
        let covered =
            inputs
                .coverage
                .line_coverage(&symbol.file, symbol.span.line, symbol.span.end_line)?;
        let cyclomatic = cyclomatic(inputs.decisions.get(&key).copied().unwrap_or_default());
        let crap = crap(cyclomatic, covered);
        tally.add(crap);
        sink.measure(Measurement {
            metric: "crap".to_owned(),
            value: crap,
            target: symbol_target(symbol, inputs.hashes.get(&key).copied()),
        });
        // A symbol without a structural hash cannot be identified, so it is scored but not
        // reported; every language producing NormTree hashes every symbol.
        let hash = inputs.hashes.get(&key)?;
        let score = CrapScore {
            crap,
            cyclomatic,
            coverage: covered,
            watch: inputs.bands.watch,
            high: inputs.bands.high,
        };
        self.band(
            Subject { symbol, hash },
            score,
            &inputs.bands,
            inputs.conventions,
        )
    }

    /// The finding for a scored symbol in the band its CRAP falls in; none below `watch`.
    fn band(
        &self,
        subject: Subject<'_>,
        score: CrapScore,
        bands: &Bands,
        conventions: Option<&TestConventions>,
    ) -> Option<Finding> {
        if score.crap >= f64::from(bands.high) {
            Some(self.high(subject, score, conventions))
        } else if score.crap >= f64::from(bands.watch) {
            Some(self.watch(subject, score))
        } else {
            None
        }
    }

    fn high(
        &self,
        subject: Subject<'_>,
        score: CrapScore,
        conventions: Option<&TestConventions>,
    ) -> Finding {
        let symbol = subject.symbol;
        let title = format!(
            "`{}` has CRAP {:.1} (high at {}): cyclomatic {} with {}",
            symbol.display,
            score.crap,
            score.high,
            score.cyclomatic,
            coverage_text(score.coverage)
        );
        let summary = format!(
            "Write tests that cover every branch of `{}` (fully covered, its CRAP is its \
             cyclomatic complexity, {}), or split it until CRAP is below {}.",
            symbol.name, score.cyclomatic, score.high
        );
        let mut finding = self
            .report(subject, "crap.high", score)
            .described(title, summary)
            .finding();
        if let Some(test) =
            conventions.and_then(|conventions| conventions.test_files_for(&symbol.file).first())
        {
            finding
                .action
                .hints
                .insert("nearest_test_file".to_owned(), Value::String(test.clone()));
        }
        finding
    }

    fn watch(&self, subject: Subject<'_>, score: CrapScore) -> Finding {
        let symbol = subject.symbol;
        let title = format!(
            "`{}` has CRAP {:.1} (watch at {}, high at {})",
            symbol.display, score.crap, score.watch, score.high
        );
        let summary = format!(
            "Review `{}` when changing it: keep its CRAP below {} with tests or simpler control \
             flow.",
            symbol.name, score.high
        );
        self.report(subject, "crap.watch", score)
            .described(title, summary)
            .at(Severity::Info)
            .finding()
    }

    /// A finding of `kind` about the subject.
    fn report<'a>(
        &'a self,
        subject: Subject<'a>,
        kind: &'static str,
        score: CrapScore,
    ) -> SymbolFinding<'a, CrapScore> {
        SymbolFinding::new(&self.manifest, kind, subject.symbol, subject.hash, score)
    }
}

fn coverage_text(coverage: f64) -> String {
    format!("{:.0}% coverage", coverage * 100.0)
}
