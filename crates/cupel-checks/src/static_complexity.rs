// ABOUTME: `static.complexity`: per-symbol cyclomatic and cognitive complexity, nesting depth, parameter
// ABOUTME: count, and length from Symbols, Decisions, and NormTree, against configurable limits (ADR-042).
use std::collections::{BTreeMap, HashMap};

use cupel_core::syntax::{
    Decision, DecisionKind, Decisions, NormTree, SymbolDef, SymbolKey, Symbols,
};
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CheckStatus, CostClass, Event, EventSink,
    FactKind, Facts, Finding, FindingKindSpec, Manifest, MetricSpec, Plan, Result, Scope, Score,
    Severity, Stability, SymbolHash, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::finding::SymbolFinding;
use crate::metrics::cyclomatic;
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
    /// 1 plus the cyclomatic weight of every decision.
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
                version: 1,
                tier: Tier::try_from(0).expect("tier 0 exists"),
                cost: CostClass::Quick,
                needs: vec![FactKind::Symbols, FactKind::Decisions, FactKind::NormTree],
                wants: vec![],
                findings: vec![
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
                ],
                metrics: vec![
                    metric(
                        "cyclomatic_max",
                        "The largest cyclomatic complexity in scope.",
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
                ],
                stability: Stability::Experimental,
                summary: "Cyclomatic and cognitive complexity, nesting depth, parameter count, \
                          and length per function, against configurable limits."
                    .to_owned(),
            },
        }
    }
}

impl Check for StaticComplexity {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        LIMITS.read(&self.manifest.id, &ctx.config)?;
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
        let limits = LIMITS.read(&self.manifest.id, &ctx.config)?;
        let symbols = facts.get::<Symbols>()?;
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
        let mut measured = 0;
        let mut worst = 0;
        let mut findings = 0;
        for symbol in &symbols.symbols {
            let key = symbol.key();
            if !ctx.scope.includes(&key) {
                continue;
            }
            ctx.cancel.checkpoint()?;
            let metrics = Metrics::of(symbol, decisions.get(&key).copied().unwrap_or_default());
            measured += 1;
            worst = worst.max(metrics.cyclomatic);
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
        Ok(CheckResult {
            id: self.manifest.id.clone(),
            status: if findings == 0 {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            tier: self.manifest.tier,
            cost: self.manifest.cost,
            score: Some(Score {
                value: f64::from(worst),
                unit: "cyclomatic_max".to_owned(),
                n: measured,
            }),
            delta: None,
            findings,
            duration_ms: 0,
            skipped: None,
            oracle: None,
        })
    }

    /// The largest value, over the symbols of every language (ADR-042).
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        let first = scores.first()?;
        Some(Score {
            value: scores.iter().map(|score| score.value).fold(0.0, f64::max),
            unit: first.unit.clone(),
            n: scores.iter().map(|score| score.n).sum(),
        })
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
            severity: Severity::Warn,
            tier: self.manifest.tier,
            title,
            evidence,
            action,
            summary,
        }
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
        let cyclomatic = cyclomatic(decisions);
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

fn evidence<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
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
