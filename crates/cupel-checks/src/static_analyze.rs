// ABOUTME: `static.analyze`: every type-checker and linter result in AnalyzerReport as an `analyze.<tool>.<rule>`
// ABOUTME: finding, severity by level, with a note for each analyzer that did not run (ADR-102).
use std::collections::{BTreeMap, HashMap};

use cupel_core::{
    Action, ActionKind, AnalyzerLevel, AnalyzerReport, AnalyzerResult, AnalyzerRun, AnalyzerStatus,
    Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, Evidence, FactKind, Facts,
    Finding, FindingId, Manifest, Plan, Result, Scope, Score, Severity, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::manifest::{evidence, kind, metric, spelling};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "static.analyze";

/// The check has no settings; any key is a configuration error (ADR-102).
const SETTINGS: Limits = Limits::new(&[]);

/// Evidence of `analyze.<tool>.<rule>`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Diagnostic {
    /// The analyzer that reported it, such as `mypy`.
    tool: String,
    /// The analyzer's rule or error code, as it reported it.
    rule: String,
    /// `error`, `warning`, or `note`.
    level: String,
    message: String,
}

pub struct StaticAnalyze {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(StaticAnalyze::new())));

impl StaticAnalyze {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(1).expect("tier 1 exists"),
                cost: CostClass::Standard,
                needs: vec![FactKind::AnalyzerReport],
                wants: vec![],
                findings: vec![kind::<Diagnostic>(
                    "analyze.<tool>.<rule>",
                    ActionKind::FixBug,
                    "A type checker or linter reported a problem: `<tool>` is the analyzer and \
                     `<rule>` its rule or error code. Errors fail, warnings warn, notes inform.",
                )],
                metrics: vec![
                    metric(
                        "errors",
                        "Results the analyzers reported as errors (type errors, error-level lints).",
                    ),
                    metric("diagnostics", "Every result the analyzers reported."),
                ],
                settings: SETTINGS.specs(),
                stability: Stability::Experimental,
                summary: "Runs each language's type checker at its strictest and its linters \
                          with the project's configuration, and reports what they find."
                    .to_owned(),
            },
        }
    }
}

impl Check for StaticAnalyze {
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
        let report = facts.get::<AnalyzerReport>()?;
        let mut tally = Tally::default();
        for analyzer in &report.analyzers {
            ctx.cancel.checkpoint()?;
            if analyzer.status == AnalyzerStatus::Ran {
                tally.ran += 1;
                for found in &analyzer.results {
                    sink.emit(Event::Finding(Box::new(
                        tally.finding(self, analyzer, found),
                    )));
                }
            } else {
                tally.notes.push(format!(
                    "{} {}: {}",
                    analyzer.tool,
                    spelling(&analyzer.status),
                    analyzer.detail.as_deref().unwrap_or("no detail")
                ));
            }
        }
        Ok(tally.result(&self.manifest))
    }

    /// Results add up across languages.
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }

    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }
}

/// What the analyzers reported so far.
#[derive(Default)]
struct Tally {
    /// Analyzers that ran.
    ran: u64,
    errors: u64,
    diagnostics: u64,
    /// Results that are notes.
    informational: u64,
    /// How many findings of each kind, file, and message were reported, which tells repeats
    /// apart.
    repeats: HashMap<(String, String, String), usize>,
    notes: Vec<String>,
}

impl Tally {
    /// The finding for `found`, counted.
    fn finding(
        &mut self,
        check: &StaticAnalyze,
        analyzer: &AnalyzerRun,
        found: &AnalyzerResult,
    ) -> Finding {
        let kind = format!("analyze.{}.{}", analyzer.tool, segment(&found.rule));
        let repeat = self
            .repeats
            .entry((
                kind.clone(),
                found.location.file.clone(),
                found.message.clone(),
            ))
            .or_default();
        let finding = check.finding(analyzer.tool.as_str(), &kind, found, *repeat);
        *repeat += 1;
        self.diagnostics += 1;
        self.errors += u64::from(found.level == AnalyzerLevel::Error);
        self.informational += u64::from(found.level == AnalyzerLevel::Note);
        finding
    }

    /// The language's result: a score and metrics only when some analyzer ran.
    fn result(self, manifest: &Manifest) -> CheckResult {
        #[allow(
            clippy::cast_precision_loss,
            reason = "result counts stay far below 2^52, where f64 is exact"
        )]
        let (errors, diagnostics) = (self.errors as f64, self.diagnostics as f64);
        let measured = self.ran > 0;
        let mut result = outcome::result(
            manifest,
            outcome::status(self.diagnostics > self.informational),
            measured.then(|| Score {
                value: diagnostics,
                unit: "diagnostics".to_owned(),
                n: self.ran,
            }),
            if measured {
                BTreeMap::from([
                    ("errors".to_owned(), errors),
                    ("diagnostics".to_owned(), diagnostics),
                ])
            } else {
                BTreeMap::new()
            },
            self.diagnostics,
        );
        result.notes = self.notes;
        result
    }
}

impl StaticAnalyze {
    /// The finding for one result: the `repeat`th of its kind, file, and message.
    fn finding(&self, tool: &str, kind: &str, found: &AnalyzerResult, repeat: usize) -> Finding {
        let file = &found.location.file;
        let subject = format!("{file}|{}|{repeat}", found.message);
        let id = FindingId::for_subject(&self.manifest.id, kind, &subject);
        let level = spelling(&found.level);
        let fields = match evidence(&Diagnostic {
            tool: tool.to_owned(),
            rule: found.rule.clone(),
            level,
            message: found.message.clone(),
        }) {
            Value::Object(fields) => fields,
            _ => Map::new(),
        };
        Finding {
            suppress: format!("cupel suppress {id} --reason '…'"),
            repro: format!("cupel check --only {} --paths {file}", self.manifest.id),
            id,
            check: self.manifest.id.clone(),
            kind: kind.to_owned(),
            severity: match found.level {
                AnalyzerLevel::Error => Severity::Fail,
                AnalyzerLevel::Warning => Severity::Warn,
                AnalyzerLevel::Note => Severity::Info,
            },
            confidence: 1.0,
            tier: self.manifest.tier,
            symbol: None,
            location: Some(found.location.clone()),
            title: format!("{tool} {}: {}", found.rule, found.message),
            detail: None,
            evidence: Evidence(fields),
            action: Action {
                kind: ActionKind::FixBug,
                priority: if found.level == AnalyzerLevel::Error {
                    1
                } else {
                    2
                },
                summary: format!(
                    "Fix what {tool} reports at {file}:{} ({}), or configure {tool} if the \
                     rule does not apply to this project.",
                    found.location.line, found.rule
                ),
                hints: Map::new(),
            },
            new_since_baseline: false,
            suppressed: false,
        }
    }
}

/// `rule` as kind segments: characters outside `[A-Za-z0-9_.-]` become `-`, and the result has
/// no leading, trailing, or doubled dot or dash at its ends.
fn segment(rule: &str) -> String {
    let replaced: String = rule
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "_.-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = replaced.trim_matches(|c| c == '-' || c == '.');
    let joined: Vec<&str> = trimmed.split('.').filter(|part| !part.is_empty()).collect();
    if joined.is_empty() {
        "unnamed".to_owned()
    } else {
        joined.join(".")
    }
}
