// ABOUTME: `tests.coverage`: per-symbol line and branch coverage from Symbols and a CoverageMap: symbols no test
// ABOUTME: ran, branches no test took (only where the map measured branches), and line/branch ratios (ADR-079).
use std::collections::{BTreeMap, HashMap};

use cupel_core::coverage::CoverageMap;
use cupel_core::syntax::{NormTree, SymbolDef, SymbolKey, Symbols, TestConventions};
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink, FactKind,
    Facts, Finding, Manifest, Measurement, MetricPer, MetricSpec, Plan, Result, Scope, Score,
    Severity, Stability, SymbolHash, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::finding::{SymbolFinding, with_nearest_test};
use crate::manifest::{kind, metric, ranked};
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
        ],
        metrics: metrics(),
        stability: Stability::Experimental,
        summary: "Line and branch coverage per function, from the coverage tool the \
                  language's `[lang.<id>] coverage` command runs (or a fresh report it \
                  left). Branch findings only where the tool measured branches; \
                  otherwise the result notes `branches: unavailable`. A function with \
                  no instrumented line is not scored."
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
    ]
}

impl Check for TestsCoverage {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn plan(&self, ctx: &CheckCtx, scope: &Scope) -> Result<Plan> {
        SETTINGS
            .read(&self.manifest.id, &ctx.config)
            .map(|_| outcome::plan_symbols(&self.manifest, scope))
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
        let first = scores.first()?;
        let lines: u64 = scores.iter().map(|score| score.n).sum();
        #[allow(
            clippy::cast_precision_loss,
            reason = "line counts stay far below 2^52, where f64 is exact"
        )]
        let covered: f64 = scores
            .iter()
            .map(|score| score.value * score.n as f64)
            .sum();
        #[allow(
            clippy::cast_precision_loss,
            reason = "line counts stay far below 2^52, where f64 is exact"
        )]
        let value = if lines == 0 {
            0.0
        } else {
            covered / lines as f64
        };
        Some(Score {
            value,
            unit: first.unit.clone(),
            n: lines,
        })
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
}

impl TestsCoverage {
    /// Scores `symbol` when coverage instrumented a line of it, measuring its uncovered lines;
    /// its finding, if any.
    fn score(
        &self,
        symbol: &SymbolDef,
        inputs: &Inputs<'_>,
        tally: &mut Tally,
        sink: &mut dyn EventSink,
    ) -> Option<Finding> {
        let (start, end) = (symbol.span.line, symbol.span.end_line);
        let lines = symbol_lines(inputs.coverage, symbol);
        if lines.is_empty() {
            return None;
        }
        let measured = Measured {
            lines: u32::try_from(lines.len()).unwrap_or(u32::MAX),
            covered: u32::try_from(lines.iter().filter(|hits| hits.hits > 0).count())
                .unwrap_or(u32::MAX),
            branches: inputs
                .coverage
                .branch_hits(&symbol.file, start, end)
                .map(|branches| branches.iter().map(|b| (b.line, b.hits)).collect()),
        };
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
        if measured.covered == 0 {
            tally.uncovered += 1;
        }
        // A symbol without a structural hash cannot be identified, so it is scored but not
        // reported; every language producing NormTree hashes every symbol.
        self.finding(symbol, hash?, &measured, inputs.conventions)
    }

    /// `coverage.uncovered-symbol` when nothing ran, else `coverage.uncovered-branch` when a
    /// measured branch was never taken.
    fn finding(
        &self,
        symbol: &SymbolDef,
        hash: &SymbolHash,
        measured: &Measured,
        conventions: Option<&TestConventions>,
    ) -> Option<Finding> {
        let finding = if measured.covered == 0 {
            self.uncovered_symbol(symbol, hash, measured.lines)
        } else {
            self.uncovered_branch(symbol, hash, measured.branches.as_ref()?)?
        };
        Some(with_nearest_test(finding, symbol, conventions))
    }

    fn uncovered_symbol(&self, symbol: &SymbolDef, hash: &SymbolHash, lines: u32) -> Finding {
        let title = format!(
            "`{}` never ran under test: none of its {lines} instrumented lines",
            symbol.display
        );
        let summary = format!("Write a test that calls `{}`.", symbol.name);
        let evidence = UncoveredSymbol { lines, covered: 0 };
        SymbolFinding::new(
            &self.manifest,
            "coverage.uncovered-symbol",
            symbol,
            hash,
            evidence,
        )
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
