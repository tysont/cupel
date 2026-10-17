// ABOUTME: `tests.assertion-density` (ADR-104): assertions per test and per tested public function, from TestFrames,
// ABOUTME: Symbols, and TestConventions; long tests that assert little, and test files far below the repository.
use std::collections::BTreeMap;

use cupel_core::syntax::{
    Span, SymbolKey, SymbolKind, Symbols, TestCase, TestConventions, TestFile, TestFrames,
    Visibility,
};
use cupel_core::{
    Action, ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Event, EventSink,
    Evidence, FactKind, Facts, Finding, FindingId, Location, Manifest, Plan, Result, Scope, Score,
    Severity, Stability, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::manifest::{evidence, kind, metric};
use crate::metrics::{scored, sum, weighted_mean};
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "tests.assertion-density";

const SPARSE_TEST: &str = "density.sparse-test";
const SPARSE_FILE: &str = "density.sparse-file";

/// The settings and their defaults (ADR-104).
const LIMITS: Limits = Limits::new(&[
    ("lines_per_assertion_max", 20),
    ("file_density_percent_min", 50),
    ("file_tests_min", 3),
]);

/// Evidence of `density.sparse-test`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SparseTest {
    /// The test's name, as its framework names it.
    test: String,
    assertions: u32,
    /// Lines the test spans, from its first line to its last.
    lines: u32,
    /// Lines per assertion, rounded down.
    lines_per_assertion: u32,
    lines_per_assertion_max: u32,
}

/// Evidence of `density.sparse-file`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SparseFile {
    /// Tests in the file.
    tests: u64,
    /// Assertions those tests make.
    assertions: u64,
    /// Tests in every test file the run read.
    repository_tests: u64,
    /// Assertions those tests make.
    repository_assertions: u64,
    /// The file's assertions per test as a percentage of the repository's, rounded down.
    percent: u64,
    file_density_percent_min: u32,
    file_tests_min: u32,
}

pub struct TestsAssertionDensity {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(
    || Box::new(TestsAssertionDensity::new())
));

impl TestsAssertionDensity {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(2).expect("tier 2 exists"),
                cost: CostClass::Quick,
                needs: vec![
                    FactKind::TestFrames,
                    FactKind::Symbols,
                    FactKind::TestConventions,
                ],
                wants: Vec::new(),
                findings: vec![
                    kind::<SparseTest>(
                        SPARSE_TEST,
                        ActionKind::StrengthenAssertion,
                        "A test that asserts something spans more than lines_per_assertion_max \
                         lines per assertion it makes.",
                    ),
                    kind::<SparseFile>(
                        SPARSE_FILE,
                        ActionKind::StrengthenAssertion,
                        "A test file of at least file_tests_min tests whose assertions per test \
                         are below file_density_percent_min percent of the repository's.",
                    ),
                ],
                metrics: [
                    (
                        "assertions_per_test",
                        "Assertions per test in scope; the score.",
                    ),
                    (
                        "assertions_per_public_function",
                        "Assertions in the tests paired with a source file, per public function \
                         of it in scope.",
                    ),
                    (
                        "tested_public_functions",
                        "Public functions in scope whose source file has tests.",
                    ),
                    ("sparse_tests", "Tests in scope reported as sparse."),
                    ("sparse_files", "Test files in scope reported as sparse."),
                ]
                .into_iter()
                .map(|(name, summary)| metric(name, summary))
                .collect(),
                stability: Stability::Experimental,
                summary: "Assertion density: assertions per test and per tested public \
                          function, long tests that assert little, and test files far below \
                          the repository's density."
                    .to_owned(),
            },
        }
    }
}

impl Check for TestsAssertionDensity {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

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
        let limits = Settings::read(&self.manifest.id, &ctx.config)?;
        let frames = facts.get::<TestFrames>()?;
        let repository = Tally::of(frames.files.iter().flat_map(|file| &file.cases));
        let mut tests = Tally::default();
        let mut reported = Vec::new();
        for file in &frames.files {
            ctx.cancel.checkpoint()?;
            for case in &file.cases {
                if ctx.scope.includes(&key_of(file, case)) {
                    tests.add(case);
                    reported.extend(self.sparse_test(file, case, &limits));
                }
            }
            if ctx.scope.files.contains(&file.file) {
                reported.extend(self.sparse_file(file, &repository, &limits));
            }
        }
        let functions = tested_functions(
            facts.get::<Symbols>()?,
            facts.get::<TestConventions>()?,
            frames,
            &ctx.scope,
        );
        let sparse = |kind: &str| count(reported.iter().filter(|f| f.kind == kind));
        let metrics = BTreeMap::from([
            ("assertions_per_test".to_owned(), tests.density()),
            (
                "assertions_per_public_function".to_owned(),
                functions.density(),
            ),
            (
                "tested_public_functions".to_owned(),
                as_f64(functions.tests),
            ),
            ("sparse_tests".to_owned(), as_f64(sparse(SPARSE_TEST))),
            ("sparse_files".to_owned(), as_f64(sparse(SPARSE_FILE))),
        ]);
        let findings = reported.len() as u64;
        for finding in reported {
            sink.emit(Event::Finding(Box::new(finding)));
        }
        Ok(outcome::result(
            &self.manifest,
            outcome::status(findings > 0),
            Some(Score {
                value: tests.density(),
                unit: "assertions_per_test".to_owned(),
                n: tests.tests,
            }),
            metrics,
            findings,
        ))
    }

    /// Densities over every language's tests or functions together; the counts add up (ADR-063).
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        match name {
            "assertions_per_test" => weighted_mean(name, results, scored),
            "assertions_per_public_function" => weighted_mean(name, results, |result| {
                result
                    .metrics
                    .get("tested_public_functions")
                    .copied()
                    .unwrap_or(0.0)
            }),
            _ => sum(name, results),
        }
    }
}

/// The configured limits (ADR-104).
struct Settings {
    lines_per_assertion_max: u32,
    file_density_percent_min: u32,
    file_tests_min: u32,
}

impl Settings {
    fn read(check: &CheckId, config: &Map<String, Value>) -> Result<Self> {
        let values = LIMITS.read(check, config)?;
        let value = |name: &str| values.get(name).copied().unwrap_or(u32::MAX);
        Ok(Self {
            lines_per_assertion_max: value("lines_per_assertion_max"),
            file_density_percent_min: value("file_density_percent_min"),
            file_tests_min: value("file_tests_min"),
        })
    }
}

/// Assertions over what they are counted per: tests, or tested public functions.
#[derive(Default)]
struct Tally {
    tests: u64,
    assertions: u64,
}

impl Tally {
    fn of<'a>(cases: impl IntoIterator<Item = &'a TestCase>) -> Self {
        let mut tally = Self::default();
        for case in cases {
            tally.add(case);
        }
        tally
    }

    fn add(&mut self, case: &TestCase) {
        self.tests += 1;
        self.assertions += case.assertions.len() as u64;
    }

    /// Assertions per test; 0 without tests.
    fn density(&self) -> f64 {
        if self.tests == 0 {
            0.0
        } else {
            as_f64(self.assertions) / as_f64(self.tests)
        }
    }
}

/// The public functions in scope outside the tests whose source file is paired with a test file
/// the run read, as `tests`, and the assertions of those test files (ADR-104).
fn tested_functions(
    symbols: &Symbols,
    conventions: &TestConventions,
    frames: &TestFrames,
    scope: &Scope,
) -> Tally {
    let mut by_file: BTreeMap<&str, u64> = BTreeMap::new();
    for symbol in &symbols.symbols {
        if symbol.visibility == Visibility::Public
            && symbol.kind != SymbolKind::Closure
            && scope.includes(&symbol.key())
            && !conventions.is_test(&symbol.file)
        {
            *by_file.entry(symbol.file.as_str()).or_default() += 1;
        }
    }
    let mut tally = Tally::default();
    for (source, functions) in by_file {
        let tests = conventions.test_files_for(source);
        let paired = Tally::of(
            frames
                .files
                .iter()
                .filter(|file| tests.contains(&file.file))
                .flat_map(|file| &file.cases),
        );
        if paired.tests > 0 {
            tally.tests += functions;
            tally.assertions += paired.assertions;
        }
    }
    tally
}

impl TestsAssertionDensity {
    /// `density.sparse-test` when `case` asserts something but spans more lines per assertion
    /// than the limit; a test asserting nothing is `tests.quality`'s `test.no-assertion`.
    fn sparse_test(&self, file: &TestFile, case: &TestCase, limits: &Settings) -> Option<Finding> {
        let assertions = u32::try_from(case.assertions.len()).unwrap_or(u32::MAX);
        let lines = case.span.end_line.saturating_sub(case.span.line) + 1;
        let per = lines.checked_div(assertions)?;
        let max = limits.lines_per_assertion_max;
        (lines > max.saturating_mul(assertions)).then(|| {
            let name = &case.name;
            self.finding(Said {
                kind: SPARSE_TEST,
                subject: format!("{}::{name}", file.file),
                file: &file.file,
                span: case.span,
                title: format!(
                    "`{name}` makes {assertions} assertion(s) in {lines} lines (max {max} lines \
                     per assertion)"
                ),
                summary: format!(
                    "Assert what `{name}` exercises as it goes, or split it into tests that \
                     each pin one outcome."
                ),
                evidence: evidence(&SparseTest {
                    test: name.clone(),
                    assertions,
                    lines,
                    lines_per_assertion: per,
                    lines_per_assertion_max: max,
                }),
            })
        })
    }

    /// `density.sparse-file` when `file` has enough tests and their assertions per test fall
    /// below the limit's share of `repository`'s.
    fn sparse_file(
        &self,
        file: &TestFile,
        repository: &Tally,
        limits: &Settings,
    ) -> Option<Finding> {
        let own = Tally::of(&file.cases);
        let (first, last) = (file.cases.first()?, file.cases.last()?);
        // The file's density over the repository's, as a percentage: own.a / own.t ÷ repo.a / repo.t.
        let percent = (100 * own.assertions * repository.tests)
            .checked_div(own.tests * repository.assertions)?;
        let min = limits.file_density_percent_min;
        (own.tests >= u64::from(limits.file_tests_min) && percent < u64::from(min)).then(|| {
            let path = &file.file;
            self.finding(Said {
                kind: SPARSE_FILE,
                subject: path.clone(),
                file: path,
                span: Span {
                    end_line: last.span.end_line,
                    end_col: last.span.end_col,
                    ..first.span
                },
                title: format!(
                    "`{path}` makes {} assertion(s) per test, {percent}% of the repository's {} \
                     (min {min}%)",
                    rounded(own.density()),
                    rounded(repository.density()),
                ),
                summary: format!(
                    "Pin more of what the tests in `{path}` exercise: they assert less than \
                     the rest of the suite."
                ),
                evidence: evidence(&SparseFile {
                    tests: own.tests,
                    assertions: own.assertions,
                    repository_tests: repository.tests,
                    repository_assertions: repository.assertions,
                    percent,
                    file_density_percent_min: min,
                    file_tests_min: limits.file_tests_min,
                }),
            })
        })
    }

    /// A `warn` finding identified by its subject (ADR-016), reproduced over its file.
    fn finding(&self, said: Said<'_>) -> Finding {
        let id = FindingId::for_subject(&self.manifest.id, said.kind, &said.subject);
        let evidence = match said.evidence {
            Value::Object(fields) => fields,
            _ => Map::new(),
        };
        Finding {
            suppress: format!("cupel suppress {id} --reason '…'"),
            repro: format!("cupel check --only {ID} --paths {}", said.file),
            id,
            check: self.manifest.id.clone(),
            kind: said.kind.to_owned(),
            severity: Severity::Warn,
            confidence: 1.0,
            tier: self.manifest.tier,
            symbol: None,
            location: Some(Location {
                file: said.file.to_owned(),
                line: said.span.line,
                col: Some(said.span.col),
                end_line: said.span.end_line,
                end_col: Some(said.span.end_col),
            }),
            title: said.title,
            detail: None,
            evidence: Evidence(evidence),
            action: Action {
                kind: ActionKind::StrengthenAssertion,
                priority: 2,
                summary: said.summary,
                hints: Map::new(),
            },
            new_since_baseline: false,
            suppressed: false,
        }
    }
}

/// What a finding says, before identity and the common fields.
struct Said<'a> {
    kind: &'static str,
    /// The structural name the finding ID is derived from.
    subject: String,
    file: &'a str,
    span: Span,
    title: String,
    summary: String,
    evidence: Value,
}

/// The key of the symbol a test case is (ADR-030, ADR-054).
fn key_of(file: &TestFile, case: &TestCase) -> SymbolKey {
    SymbolKey {
        file: file.file.clone(),
        line: case.span.line,
        col: case.span.col,
    }
}

/// `value` to two decimal places, for a title.
fn rounded(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn count<T>(items: impl Iterator<Item = T>) -> u64 {
    items.count() as u64
}

#[expect(
    clippy::cast_precision_loss,
    reason = "test, assertion, and function counts stay far below 2^52, where f64 is exact"
)]
fn as_f64(count: u64) -> f64 {
    count as f64
}
