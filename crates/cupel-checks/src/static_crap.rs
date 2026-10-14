// ABOUTME: `static.crap`: per-symbol CRAP, `CC² × (1 − coverage)³ + CC`, from Symbols, Decisions, and NormTree;
// ABOUTME: until a CoverageMap exists it scores with coverage 0 only when `no_coverage` is set (ADR-048).
use std::collections::{BTreeMap, HashMap};

use cupel_core::syntax::{
    Decision, Decisions, NormTree, SymbolDef, SymbolKey, Symbols, TestConventions,
};
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CheckStatus, CostClass, Event, EventSink,
    FactKind, Facts, Finding, FindingKindSpec, LangId, Manifest, MetricSpec, Plan, Result, Scope,
    Score, Severity, SkipReason, Skipped, Stability, SymbolHash, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::finding::SymbolFinding;
use crate::metrics::cyclomatic;
use crate::settings::Limits;

const ID: &str = "static.crap";

/// The settings and their defaults (ADR-048). A symbol whose CRAP is at least `high` is
/// `crap.high`; at least `watch` but below `high`, `crap.watch`.
const SETTINGS: Limits = Limits::new(&[("high", 30), ("watch", 5)]).with_flags(&[NO_COVERAGE]);

/// Score with coverage counted as 0 when no coverage map is available.
const NO_COVERAGE: &str = "no_coverage";

/// Evidence of `crap.high` and `crap.watch`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CrapScore {
    /// `cyclomatic² × (1 − coverage)³ + cyclomatic`, with coverage 0 when it was not measured.
    crap: f64,
    /// 1 plus the cyclomatic weight of every decision.
    cyclomatic: u32,
    /// The fraction of the symbol's lines that tests cover; null when not measured.
    coverage: Option<f64>,
    watch: u32,
    high: u32,
}

pub struct StaticCrap {
    manifest: Manifest,
}

impl StaticCrap {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(0).expect("tier 0 exists"),
                cost: CostClass::Quick,
                needs: vec![FactKind::Symbols, FactKind::Decisions, FactKind::NormTree],
                wants: vec![FactKind::CoverageMap, FactKind::TestConventions],
                findings: vec![
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
                ],
                metrics: vec![
                    metric("max", "The largest CRAP score in scope."),
                    metric("mean", "The mean CRAP score over the symbols in scope."),
                ],
                stability: Stability::Experimental,
                summary: "CRAP per function: cyclomatic complexity weighted by missing test \
                          coverage. Until coverage is measured, set `no_coverage = true` under \
                          [checks.\"static.crap\"] to score with coverage 0; otherwise the check \
                          is skipped."
                    .to_owned(),
            },
        }
    }
}

impl Check for StaticCrap {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        SETTINGS.read(&self.manifest.id, &ctx.config)?;
        SETTINGS.flag(&self.manifest.id, &ctx.config, NO_COVERAGE)?;
        let targets = scope.symbols.as_ref().map_or(0, |keys| keys.len() as u64);
        Ok(Plan::estimate(self.manifest.cost, targets))
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        let limits = SETTINGS.read(&self.manifest.id, &ctx.config)?;
        let no_coverage = SETTINGS.flag(&self.manifest.id, &ctx.config, NO_COVERAGE)?;
        let symbols = facts.get::<Symbols>()?;
        let conventions = facts.get::<TestConventions>().ok();
        // Test code is what lowers CRAP, not what it scores (ADR-048).
        let in_scope: Vec<&SymbolDef> = symbols
            .symbols
            .iter()
            .filter(|symbol| ctx.scope.includes(&symbol.key()))
            .filter(|symbol| !conventions.is_some_and(|c| c.is_test(&symbol.file)))
            .collect();
        // No producer makes a CoverageMap until P3.2, so coverage is never measured yet. Scoring
        // CC alone must be asked for, never silently assumed (ADR-048).
        if let Some(first) = in_scope.first().filter(|_| !no_coverage) {
            return Ok(self.skipped(&first.lang));
        }
        let decisions: HashMap<&SymbolKey, &[Decision]> = facts
            .get::<Decisions>()?
            .symbols
            .iter()
            .map(|entry| (&entry.symbol, entry.decisions.as_slice()))
            .collect();
        let hashes: HashMap<&SymbolKey, &SymbolHash> = facts
            .get::<NormTree>()?
            .symbols
            .iter()
            .map(|entry| (&entry.symbol, &entry.hash))
            .collect();
        let bands = Bands::of(&limits);
        let mut worst: f64 = 0.0;
        let mut findings = 0;
        let mut flagged = false;
        for symbol in &in_scope {
            ctx.cancel.checkpoint()?;
            let key = symbol.key();
            let cyclomatic = cyclomatic(decisions.get(&key).copied().unwrap_or_default());
            let crap = crap(cyclomatic, None);
            worst = worst.max(crap);
            // A symbol without a structural hash cannot be identified, so it is scored but not
            // reported; every language producing NormTree hashes every symbol.
            let Some(hash) = hashes.get(&key) else {
                continue;
            };
            let score = CrapScore {
                crap,
                cyclomatic,
                coverage: None,
                watch: bands.watch,
                high: bands.high,
            };
            let finding = if crap >= f64::from(bands.high) {
                flagged = true;
                Some(self.high(Subject { symbol, hash }, score, conventions))
            } else if crap >= f64::from(bands.watch) {
                Some(self.watch(Subject { symbol, hash }, score))
            } else {
                None
            };
            if let Some(finding) = finding {
                findings += 1;
                sink.emit(Event::Finding(Box::new(finding)));
            }
        }
        Ok(CheckResult {
            id: self.manifest.id.clone(),
            status: if flagged {
                CheckStatus::Fail
            } else {
                CheckStatus::Pass
            },
            tier: self.manifest.tier,
            cost: self.manifest.cost,
            score: Some(Score {
                value: worst,
                unit: "max".to_owned(),
                n: in_scope.len() as u64,
            }),
            delta: None,
            findings,
            duration_ms: 0,
            skipped: None,
            oracle: None,
        })
    }

    /// The largest value, over the symbols of every language (ADR-048).
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        let first = scores.first()?;
        Some(Score {
            value: scores.iter().map(|score| score.value).fold(0.0, f64::max),
            unit: first.unit.clone(),
            n: scores.iter().map(|score| score.n).sum(),
        })
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

/// `cyclomatic² × (1 − coverage)³ + cyclomatic`, with unmeasured coverage counted as 0.
fn crap(cyclomatic: u32, coverage: Option<f64>) -> f64 {
    let cyclomatic = f64::from(cyclomatic);
    let uncovered = 1.0 - coverage.unwrap_or(0.0);
    cyclomatic * cyclomatic * uncovered.powi(3) + cyclomatic
}

impl StaticCrap {
    /// The result for a language whose symbols cannot be scored without coverage.
    fn skipped(&self, lang: &LangId) -> CheckResult {
        CheckResult {
            id: self.manifest.id.clone(),
            status: CheckStatus::Skipped,
            tier: self.manifest.tier,
            cost: self.manifest.cost,
            score: None,
            delta: None,
            findings: 0,
            duration_ms: 0,
            skipped: Some(Skipped {
                reason: SkipReason::Unconfigured,
                fact: Some(FactKind::CoverageMap),
                lang: Some(lang.clone()),
                tool: None,
                see: Some(format!("cupel describe {ID}")),
            }),
            oracle: None,
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
        let mut finding = self.report(subject, "crap.high", Severity::Warn, title, score, summary);
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
        self.report(subject, "crap.watch", Severity::Info, title, score, summary)
    }

    /// A finding of `kind` about the subject, with the action its manifest maps it to.
    fn report(
        &self,
        subject: Subject<'_>,
        kind: &'static str,
        severity: Severity,
        title: String,
        evidence: CrapScore,
        summary: String,
    ) -> Finding {
        let action = self
            .manifest
            .findings
            .iter()
            .find(|spec| spec.kind == kind)
            .map_or(ActionKind::Review, |spec| spec.action);
        SymbolFinding {
            check: &self.manifest.id,
            kind,
            symbol: subject.symbol,
            hash: subject.hash,
            severity,
            tier: self.manifest.tier,
            title,
            evidence,
            action,
            summary,
        }
        .finding()
    }
}

fn coverage_text(coverage: Option<f64>) -> String {
    coverage.map_or_else(
        || "coverage not measured".to_owned(),
        |coverage| format!("{:.0}% coverage", coverage * 100.0),
    )
}

/// A finding kind whose evidence is `T`'s JSON Schema (ADR-012).
fn kind<T: JsonSchema>(kind: &str, action: ActionKind, summary: &str) -> FindingKindSpec {
    let schema = match schemars::schema_for!(T).to_value() {
        Value::Object(schema) => schema,
        _ => Map::new(),
    };
    FindingKindSpec {
        kind: kind.to_owned(),
        action,
        summary: summary.to_owned(),
        evidence: schema,
    }
}

fn metric(name: &str, summary: &str) -> MetricSpec {
    MetricSpec {
        name: name.to_owned(),
        summary: summary.to_owned(),
    }
}
