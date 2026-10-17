// ABOUTME: `gen.property` (ADR-096): properties the property engine falsified under its seed and case budget, with
// ABOUTME: the frameworks' shrunk counterexamples, and public functions whose tests hold no property test.

use cupel_core::syntax::{
    ApiSurface, Span, SymbolDef, SymbolKind, Symbols, TestConventions, Visibility,
};
use cupel_core::{
    Action, ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink,
    Evidence, FactKind, Facts, Finding, FindingId, Location, Manifest, Plan, PropertyResult,
    PropertyResults, Result, Scope, Score, Severity, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::finding::with_nearest_test;
use crate::manifest::{evidence, kind, metric};
use crate::metrics::sum;
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "gen.property";

/// The check takes no settings; the case budget is `--cases`.
const LIMITS: Limits = Limits::new(&[]);

/// Evidence of `property.falsified`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Falsified {
    /// The test's ID, as its runner names it; null when no test the runner reported is the case.
    test: Option<String>,
    /// The framework that generated its inputs, such as `proptest`.
    framework: String,
    /// The runner that ran it.
    runner: String,
    /// The framework's shrunk failing input, as it printed it; null when it failed without one.
    counterexample: Option<String>,
    /// The seed the framework was given; null for a framework that takes none.
    seed: Option<u64>,
    /// The generated cases it was asked to pass.
    cases: u32,
}

/// Evidence of `property.missing`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Missing {
    /// The function, as `namespace.name` or the language's spelling of it.
    function: String,
    name: String,
    namespace: String,
    parameters: u32,
    /// Its normalized signature, when the language reports one.
    signature: Option<String>,
    /// The language's property frameworks, in preference order.
    frameworks: Vec<String>,
}

pub struct GenProperty {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(GenProperty::new())));

impl GenProperty {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(4).expect("tier 4 exists"),
                cost: CostClass::Standard,
                needs: vec![
                    FactKind::PropertyResults,
                    FactKind::Symbols,
                    FactKind::TestConventions,
                ],
                wants: vec![FactKind::ApiSurface],
                findings: vec![
                    kind::<Falsified>(
                        "property.falsified",
                        ActionKind::FixBug,
                        "A property test failed under the run's seed and case budget; the \
                         counterexample is the framework's shrunk failing input.",
                    ),
                    kind::<Missing>(
                        "property.missing",
                        ActionKind::AddProperty,
                        "A public function that takes arguments, in a file whose tests hold no \
                         property test.",
                    ),
                ],
                metrics: ["properties", "falsified", "missing"]
                    .into_iter()
                    .zip([
                        "Property tests in scope.",
                        "Property tests falsified, or failed, under the case budget.",
                        "Public functions taking arguments whose tests hold no property test.",
                    ])
                    .map(|(name, summary)| metric(name, summary))
                    .collect(),
                stability: Stability::Experimental,
                summary: "Runs the existing property tests (proptest, quickcheck, Hypothesis, \
                          fast-check) with the run's seed and a case budget; reports the shrunk \
                          counterexamples, and functions with no property test."
                    .to_owned(),
            },
        }
    }
}

impl Check for GenProperty {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// One target per file in scope; the property run's size is the engine's to know.
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
        let properties = facts.get::<PropertyResults>()?;
        let symbols = facts.get::<Symbols>()?;
        let conventions = facts.get::<TestConventions>()?;
        let api = facts.get::<ApiSurface>().ok();
        let falsified: Vec<Finding> = properties
            .properties
            .iter()
            .filter(|property| property.outcome.is_failure())
            .map(|property| self.falsified(properties, property))
            .collect();
        let missing: Vec<Finding> = symbols
            .symbols
            .iter()
            .filter(|symbol| lacks_property(symbol, properties, conventions, &ctx.scope))
            .map(|symbol| {
                with_nearest_test(
                    self.missing(symbol, properties, api),
                    symbol,
                    Some(conventions),
                )
            })
            .collect();
        let counts = [
            ("properties", properties.properties.len()),
            ("falsified", falsified.len()),
            ("missing", missing.len()),
        ];
        let reported = (falsified.len() + missing.len()) as u64;
        let failed = !falsified.is_empty();
        for finding in falsified.into_iter().chain(missing) {
            sink.emit(Event::Finding(Box::new(finding)));
        }
        Ok(outcome::result(
            &self.manifest,
            outcome::status(failed),
            Some(outcome::counted(counts[1].1, "falsified", counts[0].1)),
            outcome::counts(&counts),
            reported,
        ))
    }

    /// Properties and findings add up across languages.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::summed(scores)
    }

    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        sum(name, results)
    }
}

/// Whether `symbol` is a public function taking arguments, outside the tests, whose file's
/// tests are all in scope and hold no property test (ADR-096 §3).
fn lacks_property(
    symbol: &SymbolDef,
    properties: &PropertyResults,
    conventions: &TestConventions,
    scope: &Scope,
) -> bool {
    let candidate = symbol.kind == SymbolKind::Function
        && symbol.visibility == Visibility::Public
        && symbol.parameters > 0
        && scope.includes(&symbol.key())
        && !conventions.is_test(&symbol.file);
    let tests = conventions.test_files_for(&symbol.file);
    candidate
        && tests.iter().all(|test| scope.files.contains(test))
        && !properties
            .properties
            .iter()
            .any(|property| tests.contains(&property.file))
}

impl GenProperty {
    fn falsified(&self, run: &PropertyResults, property: &PropertyResult) -> Finding {
        let name = &property.name;
        let (title, confidence) = match &property.counterexample {
            Some(counterexample) => (format!("`{name}` is falsified by `{counterexample}`"), 0.95),
            None => (
                format!("`{name}` failed under the case budget without a counterexample"),
                0.6,
            ),
        };
        let said = Said {
            kind: "property.falsified",
            subject: format!("{}::{name}", property.file),
            severity: Severity::Fail,
            confidence,
            title,
            summary: format!(
                "Fix the code under test so `{name}` holds for the counterexample, then rerun \
                 with the same seed; if the property is wrong, correct the property instead."
            ),
            evidence: evidence(&Falsified {
                test: property.test.clone(),
                framework: property.framework.clone(),
                runner: run.runner.to_string(),
                counterexample: property.counterexample.clone(),
                seed: property.seeded.then_some(run.seed),
                cases: run.cases,
            }),
            repro: format!(
                "cupel check --only {ID} --paths {} --seed {} --cases {}",
                property.file, run.seed, run.cases
            ),
        };
        self.finding(said, &property.file, &property.span)
    }

    fn missing(
        &self,
        symbol: &SymbolDef,
        run: &PropertyResults,
        api: Option<&ApiSurface>,
    ) -> Finding {
        let display = &symbol.display;
        let signature = api.and_then(|api| {
            api.items
                .iter()
                .find(|item| item.file == symbol.file && item.span.line == symbol.span.line)
                .map(|item| item.signature.clone())
        });
        let said = Said {
            kind: "property.missing",
            subject: format!("{}::{display}", symbol.file),
            severity: Severity::Info,
            confidence: 0.5,
            title: format!("`{display}` has no property test"),
            summary: format!(
                "State a property `{display}` keeps for every input; `cupel check --only {ID} \
                 --scaffold --write` writes a stub to start from."
            ),
            evidence: evidence(&Missing {
                function: display.clone(),
                name: symbol.name.clone(),
                namespace: symbol.namespace.clone(),
                parameters: symbol.parameters,
                signature,
                frameworks: run.frameworks.clone(),
            }),
            repro: format!("cupel check --only {ID} --paths {}", symbol.file),
        };
        self.finding(said, &symbol.file, &symbol.span)
    }

    fn finding(&self, said: Said, file: &str, span: &Span) -> Finding {
        let id = FindingId::for_subject(&self.manifest.id, said.kind, &said.subject);
        let evidence = match said.evidence {
            Value::Object(fields) => fields,
            _ => serde_json::Map::new(),
        };
        let action = self
            .manifest
            .findings
            .iter()
            .find(|spec| spec.kind == said.kind)
            .map_or(ActionKind::Review, |spec| spec.action);
        Finding {
            suppress: format!("cupel suppress {id} --reason '…'"),
            repro: said.repro,
            id,
            check: self.manifest.id.clone(),
            kind: said.kind.to_owned(),
            severity: said.severity,
            confidence: said.confidence,
            tier: self.manifest.tier,
            symbol: None,
            location: Some(Location {
                file: file.to_owned(),
                line: span.line,
                col: Some(span.col),
                end_line: span.end_line,
                end_col: Some(span.end_col),
            }),
            title: said.title,
            detail: None,
            evidence: Evidence(evidence),
            action: Action {
                kind: action,
                priority: 2,
                summary: said.summary,
                hints: serde_json::Map::new(),
            },
            new_since_baseline: false,
            suppressed: false,
        }
    }
}

/// What a finding says, before identity and location.
struct Said {
    kind: &'static str,
    /// What the finding is about, for its ID: the file and the case or function.
    subject: String,
    severity: Severity,
    confidence: f64,
    title: String,
    summary: String,
    evidence: Value,
    repro: String,
}
