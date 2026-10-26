// ABOUTME: `tests.quality`: SCRAP-style test-code quality from TestFrames, Symbols, NormTree, and Decisions:
// ABOUTME: weak or missing assertions, mock-heavy tests, logic in tests, and duplicated setup (ADR-054/055).
use std::collections::{BTreeMap, BTreeSet, HashMap};

use cupel_core::syntax::{
    AssertionStrength, Decision, DecisionKind, Decisions, NormNode, NormTree, SymbolDef, SymbolKey,
    Symbols, TestCase, TestFile, TestFrames,
};
use cupel_core::{
    ActionKind, Check, CheckCtx, CheckId, CheckResult, CostClass, Error, Event, EventSink,
    FactKind, Facts, Finding, FindingKindSpec, Manifest, Measurement, MetricPer, MetricSpec, Plan,
    Result, Scope, Score, Stability, SymbolHash, Tier,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::finding::SymbolFinding;
use crate::manifest::{evidence, kind, metric, ranked};
use crate::metrics::{
    case_key, decisions_by_symbol, scored, sum, symbol_target, trees_by_symbol, weighted_mean,
};
use crate::outcome;
use crate::settings::Limits;

const ID: &str = "tests.quality";

/// The settings and their defaults (ADR-054). A finding fires when a value exceeds its maximum.
const LIMITS: Limits = Limits::new(&[
    (
        "mock_density_max",
        50,
        "The highest share, in percent, of doubles among a test's doubles and assertions before \
         test.mock-heavy.",
    ),
    (
        "mocks_min",
        2,
        "The fewest test doubles that can make a test test.mock-heavy.",
    ),
    (
        "logic_max",
        0,
        "The most loops, alternatives, and unguarded branches a test may have before test.logic.",
    ),
    (
        "setup_statements_min",
        3,
        "The fewest leading statements tests in one file must share to be test.duplicate-setup.",
    ),
]);

/// How a finding is fixed, after SCRAP's remediation modes.
#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(inline)]
enum RemediationMode {
    Strengthen,
    AddAssertion,
    Decouple,
    Simplify,
    ExtractFixture,
}

/// How much understanding the fix needs, after SCRAP's actionability classes for an assistant.
#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(inline)]
enum Actionability {
    Mechanical,
    Guided,
    Judgment,
}

/// Evidence of `test.weak-assertion`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WeakAssertion {
    remediation_mode: RemediationMode,
    actionability: Actionability,
    /// Assertions in the test.
    assertions: u32,
    /// Of those, the weak ones: every one.
    weak: u32,
    /// The distinct semantic kinds of its assertions.
    kinds: Vec<String>,
}

/// Evidence of `test.no-assertion`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NoAssertion {
    remediation_mode: RemediationMode,
    actionability: Actionability,
    /// Test doubles in the test.
    mocks: u32,
    /// Whether the test is marked skipped.
    skipped: bool,
}

/// Evidence of `test.mock-heavy`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MockHeavy {
    remediation_mode: RemediationMode,
    actionability: Actionability,
    mocks: u32,
    assertions: u32,
    /// Doubles as a percentage of doubles plus assertions, rounded down.
    density: u32,
    mock_density_max: u32,
    mocks_min: u32,
    /// What each double replaces, as written.
    targets: Vec<String>,
}

/// Evidence of `test.logic`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TestLogic {
    remediation_mode: RemediationMode,
    actionability: Actionability,
    /// Loops, plus alternatives, plus branches beyond the guards.
    logic: u32,
    logic_max: u32,
    /// Loops other than tables.
    loops: u32,
    /// Loops over a table of cases written in place: parametrization, not logic (ADR-069).
    tables: u32,
    /// `if` and `else if` branches and switch or match arms.
    branches: u32,
    /// Conditional expressions and exception handlers.
    alternatives: u32,
    /// Branches that may be guarded failure calls or skips: one per assertion and skip marker.
    guards: u32,
}

/// Evidence of `test.duplicate-setup`.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DuplicateSetup {
    remediation_mode: RemediationMode,
    actionability: Actionability,
    /// Leading statements every test in the group repeats.
    statements: u32,
    setup_statements_min: u32,
    /// Each test in the group, in file order.
    tests: Vec<String>,
    /// The first line of each test in the group.
    lines: Vec<u32>,
}

pub struct TestsQuality {
    manifest: Manifest,
}

inventory::submit!(crate::NativeCheck(|| Box::new(TestsQuality::new())));

/// The facts the check reads: each test file's cases and assertions, and the symbols and trees
/// that identify a case and hold its statements.
const NEEDS: [FactKind; 3] = [FactKind::TestFrames, FactKind::Symbols, FactKind::NormTree];

/// The decisions, by which a case's loops and branches are counted as logic.
const WANTS: [FactKind; 1] = [FactKind::Decisions];

impl TestsQuality {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                id: CheckId::from(ID),
                version: 1,
                tier: Tier::try_from(2).expect("tier 2 exists"),
                cost: CostClass::Quick,
                needs: NEEDS.to_vec(),
                wants: WANTS.to_vec(),
                findings: finding_kinds(),
                metrics: metrics(),
                settings: LIMITS.specs(),
                stability: Stability::Experimental,
                summary: "SCRAP-style test quality: assertion density, weak and missing \
                          assertions, mock density, logic in tests, and duplicated setup."
                    .to_owned(),
            },
        }
    }
}

/// The finding kinds this check reports, with their actions and evidence.
fn finding_kinds() -> Vec<FindingKindSpec> {
    vec![
        kind::<WeakAssertion>(
            "test.weak-assertion",
            ActionKind::StrengthenAssertion,
            "Every assertion in the test is weak: truthiness, presence, inequality, or \
             a partial match.",
        ),
        kind::<NoAssertion>(
            "test.no-assertion",
            ActionKind::WriteTest,
            "The test asserts nothing; an expected panic, raise, or throw counts as an \
             assertion.",
        ),
        kind::<MockHeavy>(
            "test.mock-heavy",
            ActionKind::Review,
            "At least mocks_min test doubles, making up more than mock_density_max \
             percent of the test's doubles and assertions.",
        ),
        kind::<TestLogic>(
            "test.logic",
            ActionKind::RefactorExtract,
            "More than logic_max loops, alternatives, and branches beyond guarded \
             assertions inside a test body.",
        ),
        kind::<DuplicateSetup>(
            "test.duplicate-setup",
            ActionKind::RefactorExtract,
            "Tests in one file repeat the same setup_statements_min or more leading \
             statements.",
        ),
    ]
}

/// The metrics this check reports: gate keys, then what `survey` ranks.
fn metrics() -> Vec<MetricSpec> {
    vec![
        metric(
            "assertion_density",
            "Assertions per test in scope; the score.",
        ),
        metric(
            "weak_ratio",
            "Weak assertions as a share of all assertions in scope.",
        ),
        metric("no_assertion", "Tests in scope that assert nothing."),
        ranked(
            "mock_density",
            MetricPer::Symbol,
            "Test doubles per test as a percentage of its doubles plus assertions.",
        ),
    ]
}

impl Check for TestsQuality {
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
        let limits = Settings::read(&self.manifest.id, &ctx.config)?;
        let index = Index {
            limits,
            symbols: facts
                .get::<Symbols>()?
                .symbols
                .iter()
                .map(|symbol| (symbol.key(), symbol))
                .collect(),
            trees: trees_by_symbol([facts.get::<NormTree>()?]),
            // Decisions are wanted: without them every kind but `test.logic` still reports.
            decisions: match facts.get::<Decisions>() {
                Ok(decisions) => Some(decisions_by_symbol(decisions)),
                Err(Error::AbsentFact { .. }) => None,
                Err(error) => return Err(error),
            },
        };
        let mut counts = Counts::default();
        let mut reported = Vec::new();
        for file in &facts.get::<TestFrames>()?.files {
            reported.extend(self.file_findings(ctx, file, &index, &mut counts, sink)?);
        }
        let findings = reported.len() as u64;
        for finding in reported {
            sink.emit(Event::Finding(Box::new(finding)));
        }
        Ok(outcome::result(
            &self.manifest,
            outcome::status(findings > 0),
            Some(counts.score()),
            counts.metrics(),
            findings,
        ))
    }

    /// Ratios over every language's tests together: density weighted by tests, the weak share
    /// by assertions (density × tests); `no_assertion` adds up (ADR-063).
    fn combine_metric(&self, name: &str, results: &[&CheckResult]) -> Option<f64> {
        match name {
            "assertion_density" => weighted_mean(name, results, scored),
            "weak_ratio" => weighted_mean(name, results, |result| {
                result.metrics.get("assertion_density").unwrap_or(&0.0) * scored(result)
            }),
            _ => sum(name, results),
        }
    }

    /// Mock density is a percentage: a matched set of tests compares its weighted mean
    /// (ADR-131).
    fn averages(&self, name: &str) -> bool {
        name == "mock_density"
    }
}

/// The thresholds in force and the per-symbol facts a case joins by its key (ADR-054).
struct Index<'a> {
    limits: Settings,
    symbols: HashMap<SymbolKey, &'a SymbolDef>,
    trees: HashMap<&'a SymbolKey, (&'a SymbolHash, &'a NormNode)>,
    decisions: Option<HashMap<&'a SymbolKey, &'a [Decision]>>,
}

/// What the measured cases add up to.
#[derive(Default)]
struct Counts {
    tests: u64,
    assertions: u64,
    weak: u64,
    no_assertion: u64,
}

impl Counts {
    fn add(&mut self, case: &TestCase) {
        self.tests += 1;
        self.assertions += case.assertions.len() as u64;
        self.weak +=
            u64::from(count(case.assertions.iter().filter(|assertion| {
                assertion.strength == AssertionStrength::Weak
            })));
        self.no_assertion += u64::from(case.assertions.is_empty());
    }

    fn score(&self) -> Score {
        Score {
            value: ratio(self.assertions, self.tests),
            unit: "assertion_density".to_owned(),
            n: self.tests,
        }
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "test counts stay far below 2^52, where f64 is exact"
    )]
    fn metrics(&self) -> BTreeMap<String, f64> {
        BTreeMap::from([
            (
                "assertion_density".to_owned(),
                ratio(self.assertions, self.tests),
            ),
            ("weak_ratio".to_owned(), ratio(self.weak, self.assertions)),
            ("no_assertion".to_owned(), self.no_assertion as f64),
        ])
    }
}

/// The configured thresholds (ADR-054).
struct Settings {
    mock_density_max: u32,
    mocks_min: u32,
    logic_max: u32,
    setup_statements_min: u32,
}

impl Settings {
    fn read(check: &CheckId, config: &Map<String, Value>) -> Result<Self> {
        let values = LIMITS.read(check, config)?;
        let value = |name: &str| values.get(name).copied().unwrap_or(u32::MAX);
        Ok(Self {
            mock_density_max: value("mock_density_max"),
            mocks_min: value("mocks_min"),
            logic_max: value("logic_max"),
            setup_statements_min: value("setup_statements_min"),
        })
    }
}

/// The test symbol a finding is about.
struct Subject<'a> {
    symbol: &'a SymbolDef,
    hash: &'a SymbolHash,
}

/// A test's control flow, from its decisions (ADR-054).
struct LogicCount {
    loops: u32,
    tables: u32,
    branches: u32,
    alternatives: u32,
    guards: u32,
}

impl LogicCount {
    fn of(case: &TestCase, decisions: &[Decision]) -> Self {
        let of_kinds = |kinds: &[DecisionKind]| {
            count(
                decisions
                    .iter()
                    .filter(|decision| kinds.contains(&decision.kind)),
            )
        };
        let loops =
            |table: bool| {
                count(decisions.iter().filter(|decision| {
                    decision.kind == DecisionKind::Loop && decision.table == table
                }))
            };
        Self {
            loops: loops(false),
            tables: loops(true),
            // A case arm, like an `if`, may be the guard of one failure call
            // (`case got != want: t.Errorf(…)`).
            branches: of_kinds(&[DecisionKind::Branch, DecisionKind::Case]),
            alternatives: of_kinds(&[DecisionKind::Ternary, DecisionKind::Catch]),
            guards: count(case.assertions.iter()) + u32::from(case.skip.is_some()),
        }
    }

    /// Loops and alternatives, plus the branches that cannot all be guarded failure calls or
    /// guarded skips.
    fn logic(&self) -> u32 {
        self.loops + self.alternatives + self.branches.saturating_sub(self.guards)
    }
}

impl TestsQuality {
    /// Counts the in-scope cases of `file` and returns their findings, duplicate setup last.
    fn file_findings(
        &self,
        ctx: &CheckCtx,
        file: &TestFile,
        index: &Index<'_>,
        counts: &mut Counts,
        sink: &mut dyn EventSink,
    ) -> Result<Vec<Finding>> {
        let mut reported = Vec::new();
        let mut candidates = Vec::new();
        for case in &file.cases {
            let key = case_key(file, case);
            if !ctx.scope.includes(&key) {
                continue;
            }
            ctx.cancel.checkpoint()?;
            counts.add(case);
            // A case without a symbol or structural hash cannot be identified, so it is
            // measured but not reported; every language's test cases are symbols (ADR-054).
            let (Some(symbol), Some((hash, root))) =
                (index.symbols.get(&key), index.trees.get(&key))
            else {
                continue;
            };
            sink.measure(Measurement {
                metric: "mock_density".to_owned(),
                value: f64::from(mock_density(case)),
                target: symbol_target(symbol, Some(hash)),
            });
            let subject = Subject { symbol, hash };
            let logic = index.decisions.as_ref().map(|by_symbol| {
                LogicCount::of(case, by_symbol.get(&key).copied().unwrap_or_default())
            });
            reported.extend(self.case_findings(&subject, case, logic.as_ref(), &index.limits));
            let body = statements(root);
            candidates.push(Candidate {
                subject,
                body,
                setup: setup_length(body, case),
            });
        }
        reported.extend(self.duplicate_setup(&candidates, &index.limits));
        Ok(reported)
    }

    fn case_findings(
        &self,
        subject: &Subject<'_>,
        case: &TestCase,
        logic: Option<&LogicCount>,
        limits: &Settings,
    ) -> Vec<Finding> {
        [
            self.weak_assertion(subject, case),
            self.no_assertion(subject, case),
            self.mock_heavy(subject, case, limits),
            logic.and_then(|logic| self.logic(subject, logic, limits)),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    fn weak_assertion(&self, subject: &Subject<'_>, case: &TestCase) -> Option<Finding> {
        let weak = case
            .assertions
            .iter()
            .all(|assertion| assertion.strength == AssertionStrength::Weak);
        (weak && !case.assertions.is_empty()).then(|| {
            let kinds: BTreeSet<String> = case
                .assertions
                .iter()
                .filter_map(|assertion| match serde_json::to_value(assertion.kind) {
                    Ok(Value::String(kind)) => Some(kind),
                    _ => None,
                })
                .collect();
            let assertions = count(case.assertions.iter());
            self.report(
                subject,
                "test.weak-assertion",
                format!(
                    "`{}` asserts only weakly: {assertions} weak assertion(s), none pinning a value",
                    subject.symbol.display
                ),
                evidence(&WeakAssertion {
                    remediation_mode: RemediationMode::Strengthen,
                    actionability: Actionability::Guided,
                    assertions,
                    weak: assertions,
                    kinds: kinds.into_iter().collect(),
                }),
                format!(
                    "Replace the truthiness, presence, or inequality checks in `{}` with an \
                     assertion that pins the expected value.",
                    subject.symbol.name
                ),
            )
        })
    }

    fn no_assertion(&self, subject: &Subject<'_>, case: &TestCase) -> Option<Finding> {
        case.assertions.is_empty().then(|| {
            self.report(
                subject,
                "test.no-assertion",
                format!("`{}` asserts nothing", subject.symbol.display),
                evidence(&NoAssertion {
                    remediation_mode: RemediationMode::AddAssertion,
                    actionability: Actionability::Guided,
                    mocks: count(case.mocks.iter()),
                    skipped: case.skip.is_some(),
                }),
                format!(
                    "Add an assertion to `{}` that pins the outcome it exercises.",
                    subject.symbol.name
                ),
            )
        })
    }

    fn mock_heavy(
        &self,
        subject: &Subject<'_>,
        case: &TestCase,
        limits: &Settings,
    ) -> Option<Finding> {
        let mocks = count(case.mocks.iter());
        let assertions = count(case.assertions.iter());
        let density = mock_density(case);
        (mocks >= limits.mocks_min && density > limits.mock_density_max).then(|| {
            self.report(
                subject,
                "test.mock-heavy",
                format!(
                    "`{}` is mock-heavy: {mocks} doubles and {assertions} assertion(s), {density}% \
                     doubles (max {}%)",
                    subject.symbol.display, limits.mock_density_max
                ),
                evidence(&MockHeavy {
                    remediation_mode: RemediationMode::Decouple,
                    actionability: Actionability::Judgment,
                    mocks,
                    assertions,
                    density,
                    mock_density_max: limits.mock_density_max,
                    mocks_min: limits.mocks_min,
                    targets: case.mocks.iter().map(|mock| mock.target.clone()).collect(),
                }),
                format!(
                    "Use real collaborators or simple fakes for some of the doubles in `{}`, or \
                     split it, so it checks behavior rather than wiring.",
                    subject.symbol.name
                ),
            )
        })
    }

    fn logic(
        &self,
        subject: &Subject<'_>,
        logic: &LogicCount,
        limits: &Settings,
    ) -> Option<Finding> {
        let total = logic.logic();
        (total > limits.logic_max).then(|| {
            self.report(
                subject,
                "test.logic",
                format!(
                    "`{}` contains logic: {total} loop(s) or branch(es) (max {})",
                    subject.symbol.display, limits.logic_max
                ),
                evidence(&TestLogic {
                    remediation_mode: RemediationMode::Simplify,
                    actionability: Actionability::Guided,
                    logic: total,
                    logic_max: limits.logic_max,
                    loops: logic.loops,
                    tables: logic.tables,
                    branches: logic.branches,
                    alternatives: logic.alternatives,
                    guards: logic.guards,
                }),
                format!(
                    "Split `{}` into one test per path, or parametrize it, so the test body \
                     neither loops nor branches.",
                    subject.symbol.name
                ),
            )
        })
    }

    /// One finding per group of a file's tests that repeat the same leading statements, about
    /// the group's first test (ADR-055).
    fn duplicate_setup(&self, candidates: &[Candidate<'_>], limits: &Settings) -> Vec<Finding> {
        setup_groups(candidates, limits.setup_statements_min as usize)
            .into_iter()
            .map(|members| self.duplicate_group(&members, limits))
            .collect()
    }

    /// The finding for one group of tests sharing their leading statements, about its first.
    fn duplicate_group(&self, members: &[&Candidate<'_>], limits: &Settings) -> Finding {
        let min = limits.setup_statements_min as usize;
        let shortest = members
            .iter()
            .map(|member| member.shared_at_most())
            .min()
            .unwrap_or(0);
        let first = members[0].body;
        let shared = (min..shortest)
            .take_while(|&at| members.iter().all(|member| member.body[at] == first[at]))
            .count()
            + min;
        let statements = u32::try_from(shared).unwrap_or(u32::MAX);
        let subject = &members[0].subject;
        let others = members.len() - 1;
        self.report(
            subject,
            "test.duplicate-setup",
            format!(
                "`{}` and {others} other test(s) repeat the same {statements} leading statements",
                subject.symbol.display
            ),
            evidence(&DuplicateSetup {
                remediation_mode: RemediationMode::ExtractFixture,
                actionability: Actionability::Mechanical,
                statements,
                setup_statements_min: limits.setup_statements_min,
                tests: members
                    .iter()
                    .map(|member| member.subject.symbol.display.clone())
                    .collect(),
                lines: members
                    .iter()
                    .map(|member| member.subject.symbol.span.line)
                    .collect(),
            }),
            format!(
                "Extract the {statements} leading statements these tests share into a fixture or \
                 setup helper, starting from `{}`.",
                subject.symbol.name
            ),
        )
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

/// A test case that may share its setup with others of its file: its body's statements and how
/// many of them lead up to its first assertion.
struct Candidate<'s> {
    subject: Subject<'s>,
    body: &'s [NormNode],
    setup: usize,
}

impl Candidate<'_> {
    /// The most leading statements this case can share as setup: those before its first
    /// assertion, and never its last statement, since something must follow the repeated part.
    fn shared_at_most(&self) -> usize {
        self.setup.min(self.body.len().saturating_sub(1))
    }
}

/// How many of `body`'s leading statements come before the first that holds one of `case`'s
/// assertions: an assertion checks what the test did, so it is never setup a fixture could share,
/// however alike two tests' assertions normalize. A statement without a span holds none.
fn setup_length(body: &[NormNode], case: &TestCase) -> usize {
    body.iter()
        .position(|statement| {
            statement.span.as_ref().is_some_and(|span| {
                case.assertions
                    .iter()
                    .any(|assertion| span.contains(&assertion.span))
            })
        })
        .unwrap_or(body.len())
}

/// The groups of two or more candidates whose first `min` statements are equal setup (they come
/// before each case's first assertion, and more statements follow them), each sorted by line,
/// the groups by their first line.
fn setup_groups<'c, 's>(
    candidates: &'c [Candidate<'s>],
    min: usize,
) -> Vec<Vec<&'c Candidate<'s>>> {
    let mut groups: BTreeMap<Vec<u8>, Vec<&Candidate<'_>>> = BTreeMap::new();
    for candidate in candidates {
        // An empty prefix repeats trivially.
        if min == 0 || candidate.shared_at_most() < min {
            continue;
        }
        let prefix: Vec<u8> = candidate.body[..min]
            .iter()
            .flat_map(NormNode::shape_json)
            .collect();
        groups.entry(prefix).or_default().push(candidate);
    }
    let mut groups: Vec<_> = groups
        .into_values()
        .filter(|members| members.len() > 1)
        .map(|mut members| {
            members.sort_by_key(|member| member.subject.symbol.span.line);
            members
        })
        .collect();
    groups.sort_by_key(|members| members[0].subject.symbol.span.line);
    groups
}

/// The top-level statements of a symbol's body (ADR-055): the children of the tree's last child,
/// or of that child's only child when the body wraps its statements in one list node. A list
/// holding a leaf is a single statement's parts, not statements, and yields none.
fn statements(root: &NormNode) -> &[NormNode] {
    let Some(body) = root.children.last() else {
        return &[];
    };
    let list = match body.children.as_slice() {
        [only] => only.children.as_slice(),
        all => all,
    };
    if list.iter().any(|node| node.children.is_empty()) {
        &[]
    } else {
        list
    }
}

/// Doubles as a percentage of doubles plus assertions, rounded down; 0 for neither.
fn mock_density(case: &TestCase) -> u32 {
    let mocks = count(case.mocks.iter());
    let assertions = count(case.assertions.iter());
    (100 * mocks).checked_div(mocks + assertions).unwrap_or(0)
}

/// `part` over `whole`; 0 when `whole` is.
#[allow(
    clippy::cast_precision_loss,
    reason = "assertion and test counts stay far below 2^52, where f64 is exact"
)]
fn ratio(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

fn count<T>(items: impl Iterator<Item = T>) -> u32 {
    u32::try_from(items.count()).unwrap_or(u32::MAX)
}
