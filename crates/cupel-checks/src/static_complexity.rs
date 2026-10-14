// ABOUTME: `static.complexity`: per-symbol cyclomatic and cognitive complexity, nesting depth, parameter
// ABOUTME: count, and length from Symbols, Decisions, and NormTree, against configurable limits (ADR-042).
use std::collections::BTreeMap;

use cupel_core::syntax::{Decision, DecisionKind, Decisions, NormTree, SymbolDef, Symbols};
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, FactKind,
    Facts, Finding, FindingKindSpec, Manifest, Measurement, MetricPer, MetricSpec, Plan, Result,
    Scope, Score, Stability, SymbolHash, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::finding::SymbolFinding;
use crate::manifest::{evidence, kind, metric, ranked};
use crate::metrics::{decisions_by_symbol, hashes_by_symbol, symbol_target};
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "static.complexity";

/// The settings and their defaults (ADR-042). A finding fires when a metric exceeds its limit.
const LIMITS: Limits = Limits::new(&[
    ("cyclomatic_max", 10),
    ("cognitive_max", 15),
    ("nesting_max", 4),
    ("params_max", 5),
    ("length_max", 60),
]);

/// Evidence of `complexity.high`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ComplexityHigh {
    /// Modified cyclomatic complexity: 1 plus the cyclomatic weight of every decision, a match or
    /// switch counting once (ADR-072).
    cyclomatic: u32,
    /// The cognitive weight of every decision, structural ones increased by their nesting.
    cognitive: u32,
    cyclomatic_max: u32,
    cognitive_max: u32,
}

/// Evidence of `nesting.deep`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NestingDeep {
    /// The deepest level of nested branches, loops, cases, handlers, and conditionals.
    depth: u32,
    nesting_max: u32,
}

/// Evidence of `params.many`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ParamsMany {
    /// Declared parameters, not counting a receiver.
    parameters: u32,
    params_max: u32,
}

/// Evidence of `function.long`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FunctionLong {
    /// Lines the symbol spans, from its first line to its last.
    lines: u32,
    length_max: u32,
}

pub struct StaticComplexity {
    manifest: Manifest,
}

impl StaticComplexity {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 2,
                tier: Tier::try_from(0).expect("tier 0 exists"),
                cost: CostClass::Quick,
                needs: vec![FactKind::Symbols, FactKind::Decisions, FactKind::NormTree],
                wants: vec![],
                findings: finding_kinds(),
                metrics: metrics(),
                stability: Stability::Experimental,
                summary: "Cyclomatic and cognitive complexity, nesting depth, parameter count, \
                          and length per function, against configurable limits. Cyclomatic \
                          complexity is the modified form: a match or switch counts as one \
                          decision however many arms it has."
                    .to_owned(),
            },
        }
    }
}

/// The finding kinds this check reports, with their actions and evidence.
fn finding_kinds() -> Vec<FindingKindSpec> {
    vec![
        kind::<ComplexityHigh>(
            "complexity.high",
            ActionKind::ReduceComplexity,
            "Cyclomatic or cognitive complexity exceeds its limit \
             (cyclomatic_max, cognitive_max).",
        ),
        kind::<NestingDeep>(
            "nesting.deep",
            ActionKind::ReduceComplexity,
            "Control flow nests deeper than nesting_max.",
        ),
        kind::<ParamsMany>(
            "params.many",
            ActionKind::RefactorExtract,
            "More declared parameters than params_max, not counting a receiver.",
        ),
        kind::<FunctionLong>(
            "function.long",
            ActionKind::RefactorExtract,
            "The symbol spans more lines than length_max.",
        ),
    ]
}

/// The metrics this check reports: gate keys, then what `survey` ranks.
fn metrics() -> Vec<MetricSpec> {
    vec![
        metric(
            "cyclomatic_max",
            "The largest modified cyclomatic complexity in scope (a match counts once).",
        ),
        metric(
            "cognitive_max",
            "The largest cognitive complexity in scope.",
        ),
        metric("nesting_max", "The deepest nesting in scope."),
        metric(
            "params_max",
            "The most parameters any symbol in scope declares.",
        ),
        metric("length_max", "The most lines any symbol in scope spans."),
        ranked(
            "cyclomatic",
            MetricPer::Symbol,
            "Modified cyclomatic complexity per function (a match counts once).",
        ),
        ranked(
            "cognitive",
            MetricPer::Symbol,
            "Cognitive complexity per function.",
        ),
    ]
}

impl Check for StaticComplexity {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        LIMITS.read(&self.manifest.id, &ctx.config)?;
        Ok(outcome::plan_symbols(&self.manifest, scope))
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        let limits = LIMITS.read(&self.manifest.id, &ctx.config)?;
        let symbols = facts.get::<Symbols>()?;
        let decisions = decisions_by_symbol(facts.get::<Decisions>()?);
        let hashes = hashes_by_symbol(facts.get::<NormTree>()?);
        let mut measured = 0;
        let mut worst = Worst::default();
        let mut findings = 0;
        for symbol in &symbols.symbols {
            let key = symbol.key();
            if !ctx.scope.includes(&key) {
                continue;
            }
            ctx.cancel.checkpoint()?;
            let metrics = Metrics::of(symbol, decisions.get(&key).copied().unwrap_or_default());
            measured += 1;
            worst.include(symbol, &metrics);
            measure(sink, symbol, hashes.get(&key).copied(), &metrics);
            // A symbol without a structural hash cannot be identified, so it is measured but
            // not reported; every language producing NormTree hashes every symbol.
            let Some(hash) = hashes.get(&key) else {
                continue;
            };
            let subject = Subject {
                symbol,
                hash,
                limits: &limits,
            };
            for finding in self.findings(&subject, &metrics) {
                findings += 1;
                sink.emit(Event::Finding(Box::new(finding)));
            }
        }
        Ok(outcome::result(
            &self.manifest,
            outcome::status(findings > 0),
            Some(Score {
                value: f64::from(worst.cyclomatic),
                unit: "cyclomatic_max".to_owned(),
                n: measured,
            }),
            worst.metrics(),
            findings,
        ))
    }

    /// The largest value, over the symbols of every language (ADR-042).
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::maximum(scores)
    }
}

/// The symbol a finding is about and the limits it is measured against.
struct Subject<'a> {
    symbol: &'a SymbolDef,
    hash: &'a SymbolHash,
    limits: &'a BTreeMap<&'static str, u32>,
}

impl Subject<'_> {
    fn limit(&self, name: &str) -> u32 {
        self.limits.get(name).copied().unwrap_or(u32::MAX)
    }
}

impl StaticComplexity {
    fn findings(&self, subject: &Subject<'_>, metrics: &Metrics) -> Vec<cupel_core::Finding> {
        [
            self.complexity_high(subject, metrics),
            self.nesting_deep(subject, metrics),
            self.params_many(subject),
            self.function_long(subject, metrics),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    fn complexity_high(&self, subject: &Subject<'_>, metrics: &Metrics) -> Option<Finding> {
        let (cyclomatic_max, cognitive_max) = (
            subject.limit("cyclomatic_max"),
            subject.limit("cognitive_max"),
        );
        (metrics.cyclomatic > cyclomatic_max || metrics.cognitive > cognitive_max).then(|| {
            self.report(
                subject,
                "complexity.high",
                format!(
                    "`{}` is too complex: cyclomatic {} (max {cyclomatic_max}), cognitive {} \
                     (max {cognitive_max})",
                    subject.symbol.display, metrics.cyclomatic, metrics.cognitive
                ),
                evidence(&ComplexityHigh {
                    cyclomatic: metrics.cyclomatic,
                    cognitive: metrics.cognitive,
                    cyclomatic_max,
                    cognitive_max,
                }),
                format!(
                    "Split `{}` into smaller functions or replace conditional chains with data, \
                     until cyclomatic is at most {cyclomatic_max} and cognitive at most \
                     {cognitive_max}.",
                    subject.symbol.name
                ),
            )
        })
    }

    fn nesting_deep(&self, subject: &Subject<'_>, metrics: &Metrics) -> Option<Finding> {
        let nesting_max = subject.limit("nesting_max");
        (metrics.depth > nesting_max).then(|| {
            self.report(
                subject,
                "nesting.deep",
                format!(
                    "`{}` nests {} levels deep (max {nesting_max})",
                    subject.symbol.display, metrics.depth
                ),
                evidence(&NestingDeep {
                    depth: metrics.depth,
                    nesting_max,
                }),
                format!(
                    "Flatten `{}` with guard clauses and early exits, or extract the inner levels \
                     into named functions, until it nests at most {nesting_max} deep.",
                    subject.symbol.name
                ),
            )
        })
    }

    fn params_many(&self, subject: &Subject<'_>) -> Option<Finding> {
        let params_max = subject.limit("params_max");
        let parameters = subject.symbol.parameters;
        (parameters > params_max).then(|| {
            self.report(
                subject,
                "params.many",
                format!(
                    "`{}` takes {parameters} parameters (max {params_max})",
                    subject.symbol.display
                ),
                evidence(&ParamsMany {
                    parameters,
                    params_max,
                }),
                format!(
                    "Group related parameters of `{}` into a type, or split it so each part needs \
                     at most {params_max}.",
                    subject.symbol.name
                ),
            )
        })
    }

    fn function_long(&self, subject: &Subject<'_>, metrics: &Metrics) -> Option<Finding> {
        let length_max = subject.limit("length_max");
        (metrics.lines > length_max).then(|| {
            self.report(
                subject,
                "function.long",
                format!(
                    "`{}` is {} lines long (max {length_max})",
                    subject.symbol.display, metrics.lines
                ),
                evidence(&FunctionLong {
                    lines: metrics.lines,
                    length_max,
                }),
                format!(
                    "Extract cohesive blocks of `{}` into named functions until it is at most \
                     {length_max} lines.",
                    subject.symbol.name
                ),
            )
        })
    }

    /// A `warn` finding of `kind` about the subject, with the action its manifest maps it to.
    fn report(
        &self,
        subject: &Subject<'_>,
        kind: &'static str,
        title: String,
        evidence: Value,
        summary: String,
    ) -> Finding {
        SymbolFinding::new(&self.manifest, kind, subject.symbol, subject.hash, evidence)
            .described(title, summary)
            .finding()
    }
}

/// One symbol's measurements (ADR-042).
struct Metrics {
    cyclomatic: u32,
    cognitive: u32,
    depth: u32,
    lines: u32,
}

impl Metrics {
    fn of(symbol: &SymbolDef, decisions: &[Decision]) -> Self {
        // A match or switch is one multiway decision however many arms it has (ADR-072).
        let cyclomatic = 1 + decisions
            .iter()
            .filter(|d| !d.continues)
            .map(|d| d.cyclomatic)
            .sum::<u32>();
        let cognitive = decisions
            .iter()
            .map(|d| {
                let increment = structural(d.kind) && d.cognitive > 0;
                d.cognitive + if increment { d.nesting } else { 0 }
            })
            .sum();
        let depth = decisions
            .iter()
            .filter(|d| structural(d.kind))
            .map(|d| d.nesting + 1)
            .max()
            .unwrap_or(0);
        Self {
            cyclomatic,
            cognitive,
            depth,
            lines: symbol.span.end_line.saturating_sub(symbol.span.line) + 1,
        }
    }
}

/// The largest value of each metric over the symbols in scope: the check's `scope` metrics
/// (ADR-042, ADR-060).
#[derive(Default)]
struct Worst {
    cyclomatic: u32,
    cognitive: u32,
    depth: u32,
    parameters: u32,
    lines: u32,
}

impl Worst {
    fn include(&mut self, symbol: &SymbolDef, metrics: &Metrics) {
        self.cyclomatic = self.cyclomatic.max(metrics.cyclomatic);
        self.cognitive = self.cognitive.max(metrics.cognitive);
        self.depth = self.depth.max(metrics.depth);
        self.parameters = self.parameters.max(symbol.parameters);
        self.lines = self.lines.max(metrics.lines);
    }

    fn metrics(&self) -> BTreeMap<String, f64> {
        [
            ("cyclomatic_max", self.cyclomatic),
            ("cognitive_max", self.cognitive),
            ("nesting_max", self.depth),
            ("params_max", self.parameters),
            ("length_max", self.lines),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), f64::from(value)))
        .collect()
    }
}

/// Reports `symbol`'s cyclomatic and cognitive complexity for `survey`'s rankings (ADR-062).
fn measure(
    sink: &mut dyn EventSink,
    symbol: &SymbolDef,
    hash: Option<&SymbolHash>,
    metrics: &Metrics,
) {
    for (metric, value) in [
        ("cyclomatic", metrics.cyclomatic),
        ("cognitive", metrics.cognitive),
    ] {
        sink.measure(Measurement {
            metric: metric.to_owned(),
            value: f64::from(value),
            target: symbol_target(symbol, hash),
        });
    }
}

/// Kinds that structure control flow: they nest what they contain and carry the nesting
/// increment of cognitive complexity (ADR-024, ADR-042).
fn structural(kind: DecisionKind) -> bool {
    matches!(
        kind,
        DecisionKind::Branch
            | DecisionKind::Loop
            | DecisionKind::Case
            | DecisionKind::Catch
            | DecisionKind::Ternary
    )
}
