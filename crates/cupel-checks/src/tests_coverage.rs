// ABOUTME: `tests.coverage`: per-symbol line and branch coverage from Symbols and a CoverageMap: symbols no test
// ABOUTME: ran, files no test loaded, branches no test took (where measured), and line/branch ratios (ADR-079, ADR-152).
use std::collections::{BTreeMap, HashMap};

use cupel_core::coverage::{CoverageMap, Listing};
use cupel_core::syntax::{NormTree, SymbolDef, SymbolKey, Symbols, TestConventions};
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, FactKind,
    Facts, Finding, Manifest, Measurement, MetricPer, MetricSpec, Plan, ProtectionMetric, Result,
    Scope, Score, Severity, Stability, SymbolHash, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::finding::{SymbolFinding, with_nearest_test};
use crate::manifest::{kind, metric, protection, ranked};
use crate::metrics::{hashes_by_symbol, sum, symbol_lines, symbol_target, weighted_mean};
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "tests.coverage";

/// The check has no settings; any key is a configuration error.
const SETTINGS: Limits = Limits::new(&[]);

/// What the result notes when the coverage map measured no branches (design §8.3.2).
const BRANCHES_UNAVAILABLE: &str = "branches: unavailable";

/// Evidence of `coverage.uncovered-symbol`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UncoveredSymbol {
    /// The symbol's instrumented lines.
    lines: u32,
    /// Of those, the ones that ran: none.
    covered: u32,
}

/// Evidence of `coverage.unloaded-file`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UnloadedFile {
    /// The symbol's lines, which stand in for the instrumented lines the tool never listed.
    lines: u32,
    /// Of those, the ones that ran: none.
    covered: u32,
}

/// Evidence of `coverage.uncovered-branch`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UncoveredBranch {
    /// The branches the coverage map measured in the symbol.
    branches: u32,
    /// Of those, the ones no test took.
    uncovered: u32,
    /// The line of each branch no test took, in order, each once.
    lines: Vec<u32>,
}

pub struct TestsCoverage {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(TestsCoverage::new())));

impl TestsCoverage {
    pub fn new() -> Self {
        Self {
            manifest: manifest(),
        }
    }
}

/// The manifest: needs, wants, finding kinds with their evidence, and metrics (ADR-079).
fn manifest() -> Manifest {
    Manifest {
        id: CheckId::from(ID),
        version: 1,
        tier: Tier::try_from(2).expect("tier 2 exists"),
        cost: CostClass::Standard,
        needs: vec![FactKind::Symbols, FactKind::CoverageMap],
        wants: vec![FactKind::NormTree, FactKind::TestConventions],
        findings: vec![
            kind::<UncoveredSymbol>(
                "coverage.uncovered-symbol",
                ActionKind::WriteTest,
                "No test ran any instrumented line of the function.",
            ),
            kind::<UncoveredBranch>(
                "coverage.uncovered-branch",
                ActionKind::WriteTest,
                "Tests ran the function but never took some of its branches.",
            ),
            kind::<UnloadedFile>(
                "coverage.unloaded-file",
                ActionKind::WriteTest,
                "No test loaded the function's file, so the coverage tool never listed it.",
            ),
        ],
        metrics: metrics(),
        settings: SETTINGS.specs(),
        stability: Stability::Experimental,
        summary: "Line and branch coverage per function, from the coverage tool the \
                  language's `[lang.<id>] coverage` command runs (or a fresh report it \
                  left). Branch findings only where the tool measured branches; \
                  otherwise the result notes `branches: unavailable`. A function with \
                  no instrumented line is not scored, unless the tool lists only the files \
                  the suite loaded: then a function in a file it left out never ran."
            .to_owned(),
    }
}

/// The metrics this check reports: gate keys, then what `survey` ranks.
fn metrics() -> Vec<MetricSpec> {
    vec![
        metric(
            "line",
            "The share of the scored functions' instrumented lines that ran; the score.",
        ),
        metric(
            "branch",
            "The share of the scored functions' branches that were taken; only when measured.",
        ),
        metric("lines", "Instrumented lines in the scored functions."),
        metric("branches", "Measured branches in the scored functions."),
        metric("uncovered", "Functions in scope no test ran."),
        ranked(
            "uncovered_lines",
            MetricPer::Symbol,
            "Instrumented lines of the function that never ran.",
        ),
        protection(
            "line_coverage",
            ProtectionMetric::Coverage,
            "Instrumented lines of the function that ran, as a ratio of all of them.",
        ),
    ]
}

impl Check for TestsCoverage {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        SETTINGS.plan(
            &self.manifest,
            ctx,
            outcome::plan_symbols(&self.manifest, scope),
        )
    }

    fn run(
        &self,
        ctx: &CheckCtx,
        facts: &Facts,
        _: &Plan,
        sink: &mut dyn EventSink,
    ) -> Result<CheckResult> {
        SETTINGS.read(&self.manifest.id, &ctx.config)?;
        let inputs = Inputs {
            coverage: facts.get::<CoverageMap>()?,
            conventions: facts.get::<TestConventions>().ok(),
            hashes: facts
                .get::<NormTree>()
                .map(hashes_by_symbol)
                .unwrap_or_default(),
        };
        let mut tally = Tally::default();
        for symbol in facts
            .get::<Symbols>()?
            .symbols
            .iter()
            .filter(|symbol| ctx.scope.includes(&symbol.key()))
            .filter(|symbol| !inputs.conventions.is_some_and(|c| c.is_test(&symbol.file)))
        {
            ctx.cancel.checkpoint()?;
            if let Some(finding) = self.score(symbol, &inputs, &mut tally, sink) {
                tally.findings += 1;
                tally.failed |= finding.severity != Severity::Info;
                sink.emit(Event::Finding(Box::new(finding)));
            }
        }
        Ok(tally.result(&self.manifest, inputs.coverage.branches_measured()))
    }

    /// The line ratio over every language's instrumented lines together.
    fn combine(&self, scores: &[Score]) -> Option<Score> {
        outcome::weighted(scores)
    }

    /// Ratios weighted by what they are ratios of; counts add up (ADR-063).
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        let weight = |of: &'static str| {
            move |result: &CheckResult| result.metrics.get(of).copied().unwrap_or(0.0)
        };
        match name {
            "line" => weighted_mean(name, results, weight("lines")),
            "branch" => weighted_mean(name, results, weight("branches")),
            _ => sum(name, results),
        }
    }
}

/// What scoring reads besides the symbols.
struct Inputs<'a> {
    coverage: &'a CoverageMap,
    conventions: Option<&'a TestConventions>,
    hashes: HashMap<&'a SymbolKey, &'a SymbolHash>,
}

/// What the scored symbols added up to.
#[derive(Default)]
struct Tally {
    lines: u64,
    covered_lines: u64,
    branches: u64,
    taken: u64,
    uncovered: u64,
    findings: u64,
    /// Some symbol is uncovered.
    failed: bool,
}

impl Tally {
    /// The language's result: `fail` with an uncovered symbol, scored by the line ratio;
    /// branch metrics only when the map measured branches, else the note that it did not.
    fn result(&self, manifest: &Manifest, branches_measured: bool) -> CheckResult {
        let line = ratio(self.covered_lines, self.lines);
        let mut metrics = BTreeMap::from([
            ("line".to_owned(), line),
            ("lines".to_owned(), count(self.lines)),
            ("uncovered".to_owned(), count(self.uncovered)),
        ]);
        if branches_measured {
            metrics.insert("branch".to_owned(), ratio(self.taken, self.branches));
            metrics.insert("branches".to_owned(), count(self.branches));
        }
        let score = Score {
            value: line,
            unit: "line_ratio".to_owned(),
            n: self.lines,
        };
        let mut result = outcome::result(
            manifest,
            outcome::status(self.failed),
            Some(score),
            metrics,
            self.findings,
        );
        if !branches_measured {
            result.notes.push(BRANCHES_UNAVAILABLE.to_owned());
        }
        result
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "line and branch counts stay far below 2^52, where f64 is exact"
)]
fn count(value: u64) -> f64 {
    value as f64
}

/// `part` of `whole`; 0 when `whole` is 0.
fn ratio(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        count(part) / count(whole)
    }
}

/// One symbol's coverage.
struct Measured {
    lines: u32,
    covered: u32,
    /// `(line, hits)` of each branch in the symbol; `None` when the file's branches were not
    /// measured.
    branches: Option<Vec<(u32, u64)>>,
    /// Whether the suite loaded the symbol's file; when it did not, `lines` are the symbol's
    /// own, since the tool listed none.
    loaded: bool,
}

/// The lines of `symbol` that stand in for its instrumented lines when its file was never
/// loaded: its body's, or its declaration line alone, as `symbol_lines` reads a listed file.
fn span_lines(symbol: &SymbolDef) -> u32 {
    let (first, last) = (symbol.span.line, symbol.span.end_line);
    if last > first { last - first } else { 1 }
}

/// The coverage of `symbol`: its instrumented lines and branches; or, when the map lists only
/// the files the suite loaded and leaves its file out, its span's lines, none covered (ADR-152);
/// `None` when it has nothing to measure.
fn measure(symbol: &SymbolDef, coverage: &CoverageMap) -> Option<Measured> {
    let lines = symbol_lines(coverage, symbol);
    if lines.is_empty() {
        let unloaded = coverage.listing == Listing::Loaded
            && !coverage.files.is_empty()
            && !coverage.instruments(&symbol.file);
        return unloaded.then(|| Measured {
            lines: span_lines(symbol),
            covered: 0,
            branches: None,
            loaded: false,
        });
    }
    Some(Measured {
        lines: u32::try_from(lines.len()).unwrap_or(u32::MAX),
        covered: u32::try_from(lines.iter().filter(|hits| hits.hits > 0).count())
            .unwrap_or(u32::MAX),
        branches: coverage
            .branch_hits(&symbol.file, symbol.span.line, symbol.span.end_line)
            .map(|branches| branches.iter().map(|b| (b.line, b.hits)).collect()),
        loaded: true,
    })
}

impl TestsCoverage {
    /// Scores `symbol` when it has lines to measure ([`measure`]), measuring its uncovered
    /// lines; its finding, if any.
    fn score(
        &self,
        symbol: &SymbolDef,
        inputs: &Inputs<'_>,
        tally: &mut Tally,
        sink: &mut dyn EventSink,
    ) -> Option<Finding> {
        let measured = measure(symbol, inputs.coverage)?;
        tally.lines += u64::from(measured.lines);
        tally.covered_lines += u64::from(measured.covered);
        for (_, hits) in measured.branches.iter().flatten() {
            tally.branches += 1;
            tally.taken += u64::from(*hits > 0);
        }
        let key = symbol.key();
        let hash = inputs.hashes.get(&key).copied();
        sink.measure(Measurement {
            metric: "uncovered_lines".to_owned(),
            value: f64::from(measured.lines - measured.covered),
            target: symbol_target(symbol, hash),
        });
        sink.measure(Measurement {
            metric: "line_coverage".to_owned(),
            value: f64::from(measured.covered) / f64::from(measured.lines),
            target: symbol_target(symbol, hash),
        });
        if measured.covered == 0 {
            tally.uncovered += 1;
        }
        // A symbol without a structural hash cannot be identified, so it is scored but not
        // reported; every language producing NormTree hashes every symbol.
        self.finding(symbol, hash?, &measured, inputs.conventions)
    }

    /// `coverage.unloaded-file` when the suite never loaded the file, `coverage.uncovered-symbol`
    /// when nothing ran, else `coverage.uncovered-branch` when a measured branch was never taken.
    fn finding(
        &self,
        symbol: &SymbolDef,
        hash: &SymbolHash,
        measured: &Measured,
        conventions: Option<&TestConventions>,
    ) -> Option<Finding> {
        let finding = if measured.covered == 0 {
            self.never_ran(symbol, hash, measured)
        } else {
            self.uncovered_branch(symbol, hash, measured.branches.as_ref()?)?
        };
        Some(with_nearest_test(finding, symbol, conventions))
    }

    /// The finding about a symbol none of whose lines ran: `coverage.uncovered-symbol`, or
    /// `coverage.unloaded-file` when the suite never loaded its file. Both kinds' evidence is
    /// `{lines, covered}` (`UncoveredSymbol`, `UnloadedFile`).
    fn never_ran(&self, symbol: &SymbolDef, hash: &SymbolHash, measured: &Measured) -> Finding {
        let lines = measured.lines;
        let (kind, title, summary) = if measured.loaded {
            (
                "coverage.uncovered-symbol",
                format!(
                    "`{}` never ran under test: none of its {lines} instrumented lines",
                    symbol.display
                ),
                format!("Write a test that calls `{}`.", symbol.name),
            )
        } else {
            (
                "coverage.unloaded-file",
                format!(
                    "`{}` is in a file no test loaded: none of its {lines} lines ran",
                    symbol.display
                ),
                format!(
                    "Write a test that imports `{}` and calls `{}`.",
                    symbol.namespace, symbol.name
                ),
            )
        };
        let evidence = UncoveredSymbol { lines, covered: 0 };
        SymbolFinding::new(&self.manifest, kind, symbol, hash, evidence)
            .described(title, summary)
            .finding()
    }

    /// The finding about `branches` (`(line, hits)`) of a symbol that ran, when one was never
    /// taken.
    fn uncovered_branch(
        &self,
        symbol: &SymbolDef,
        hash: &SymbolHash,
        branches: &[(u32, u64)],
    ) -> Option<Finding> {
        let mut lines: Vec<u32> = branches
            .iter()
            .filter(|(_, hits)| *hits == 0)
            .map(|(line, _)| *line)
            .collect();
        let uncovered = u32::try_from(lines.len()).unwrap_or(u32::MAX);
        if uncovered == 0 {
            return None;
        }
        lines.dedup();
        let named: Vec<String> = lines.iter().map(ToString::to_string).collect();
        let title = format!(
            "`{}` ran, but {uncovered} of its {} branches never did",
            symbol.display,
            branches.len()
        );
        let summary = format!(
            "Write a test that takes the untaken branches of `{}` (lines {}).",
            symbol.name,
            named.join(", ")
        );
        let evidence = UncoveredBranch {
            branches: u32::try_from(branches.len()).unwrap_or(u32::MAX),
            uncovered,
            lines,
        };
        let finding = SymbolFinding::new(
            &self.manifest,
            "coverage.uncovered-branch",
            symbol,
            hash,
            evidence,
        );
        Some(
            finding
                .described(title, summary)
                .at(Severity::Info)
                .finding(),
        )
    }
}
