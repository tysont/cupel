// ABOUTME: The `TestResults` fact payload (ADR-075): every test a runner reported with its outcome, and the
// ABOUTME: invocations that ran them. Whether the suite is green decides the baseline's exit 4 (ADR-076).
use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactEntry, FactKind};
use crate::id::{LangId, ProducerId};

/// The most lines an output tail keeps.
pub const OUTPUT_TAIL_LINES: usize = 40;

/// What one run of a test suite reported, normalized across runners (ADR-075).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestResults {
    /// The producer that ran the suite, such as `tool.pytest`.
    pub runner: ProducerId,
    /// One per project root the runner ran in, in root order.
    pub invocations: Vec<RunnerInvocation>,
    /// Every test the runner reported, sorted by ID.
    pub tests: Vec<TestResult>,
    /// Identifies the suite that ran: a blake3 hex digest of the sorted test IDs and the content
    /// of every file they name.
    pub suite_hash: String,
    /// How a joint suite run obtained these results from its cache (ADR-149 §5); null for a
    /// runner that runs its whole suite every time.
    pub reuse: Option<SuiteReuse>,
}

impl Fact for TestResults {
    const KIND: FactKind = FactKind::TestResults;
    const VERSION: u32 = 2;
}

/// How many of a joint suite run's tests ran and how many were kept from its cache (ADR-149).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SuiteReuse {
    pub mode: SuiteMode,
    /// The tests that ran under instrumentation in this production.
    pub rerun: u64,
    /// The tests whose results and coverage came from the cache without running.
    pub reused: u64,
}

/// What a joint suite run did.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum SuiteMode {
    /// Every test ran.
    Full,
    /// The stale and new tests ran; the rest were kept from the cache (a range scope).
    Incremental,
    /// Nothing ran: the tree is as the cache recorded it.
    Cached,
}

inventory::submit!(FactEntry::execution::<TestResults>("test-results").pinned_by("runner"));

/// One run of a runner in one project root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunnerInvocation {
    /// The directory it ran in, relative to the repository root, `/`-separated; `.` for the root.
    pub dir: String,
    /// The program and its arguments, as run.
    pub command: Vec<String>,
    /// `None` when the process was ended by a signal.
    pub exit_code: Option<i32>,
    /// Whether the runner reached the end of the suite and reported it: false for a build that
    /// failed, a crash, or a report that could not be read.
    pub completed: bool,
    /// The tail of the runner's output, when it did not complete.
    pub output: Option<String>,
}

/// One test as its runner reported it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestResult {
    /// The runner's identity of the test, unique within the fact and stable across runs.
    pub id: String,
    /// The file the runner attributes the test to, relative to the repository root.
    pub file: Option<String>,
    /// The runner's display name of the test.
    pub name: String,
    pub outcome: TestOutcome,
    /// `None` when the runner does not measure it.
    pub duration_ms: Option<u64>,
    /// For `failed` and `errored` tests, the tail of the failure message and captured output.
    pub output: Option<String>,
}

/// How a test ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TestOutcome {
    Passed,
    /// An assertion or panic in the test.
    Failed,
    /// Ignored, pending, todo, or skipped.
    Skipped,
    /// The test or its suite could not run, such as a file that does not load or build.
    Errored,
}

impl TestOutcome {
    /// The outcome's name as it serializes.
    pub fn as_str(self) -> &'static str {
        OUTCOMES[self as usize]
    }

    /// Whether this outcome makes the suite red.
    pub fn is_failure(self) -> bool {
        matches!(self, Self::Failed | Self::Errored)
    }
}

/// Each outcome's name, in declaration order (so `OUTCOMES[outcome as usize]` names it).
const OUTCOMES: [&str; 4] = ["passed", "failed", "skipped", "errored"];

impl fmt::Display for TestOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TestResults {
    /// The results of `runner`'s `invocations`, with `tests` sorted by ID and the suite hash
    /// computed over the files they name under `root`, the repository root.
    pub fn new(
        runner: ProducerId,
        invocations: Vec<RunnerInvocation>,
        mut tests: Vec<TestResult>,
        root: &Path,
    ) -> Self {
        tests.sort_by(|a, b| a.id.cmp(&b.id));
        let suite_hash = suite_hash(root, &tests);
        Self {
            runner,
            invocations,
            tests,
            suite_hash,
            reuse: None,
        }
    }

    /// These results, obtained as `reuse` says.
    #[must_use]
    pub fn with_reuse(mut self, reuse: SuiteReuse) -> Self {
        self.reuse = Some(reuse);
        self
    }

    /// Whether every invocation completed and no test failed or errored (ADR-075).
    pub fn is_green(&self) -> bool {
        self.failures().next().is_none() && self.incomplete().next().is_none()
    }

    /// Whether no test ran: there was no suite (no project to run it in), or every test it
    /// reported was skipped. Nothing that depends on test execution can be measured.
    pub fn ran_none(&self) -> bool {
        self.tests
            .iter()
            .all(|test| test.outcome == TestOutcome::Skipped)
    }

    /// The tests that make the suite red.
    pub fn failures(&self) -> impl Iterator<Item = &TestResult> {
        self.tests.iter().filter(|test| test.outcome.is_failure())
    }

    /// The invocations that did not reach the end of the suite.
    pub fn incomplete(&self) -> impl Iterator<Item = &RunnerInvocation> {
        self.invocations
            .iter()
            .filter(|invocation| !invocation.completed)
    }
}

/// A red baseline: the test run of `lang` that refused the run (`Outcome::refused`, ADR-076);
/// the CLI reports it with exit 4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unevaluable {
    pub lang: LangId,
    pub results: TestResults,
}

/// Whether `reported`, a runner's name of a test, names the `TestFrames` case `case`: equal, or
/// ending in it after a separator (`tests::case`, `Suite > case`, `test_case (module.Class.test_case)`
/// excepted).
pub fn names_case(reported: &str, case: &str) -> bool {
    reported == case
        || reported
            .strip_suffix(case)
            .is_some_and(|head| head.ends_with(|c: char| !(c.is_alphanumeric() || c == '_')))
}

/// The last [`OUTPUT_TAIL_LINES`] lines of `text`, without surrounding blank lines; `None` when
/// nothing is left.
pub fn output_tail(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.trim().lines().collect();
    let start = lines.len().saturating_sub(OUTPUT_TAIL_LINES);
    let tail = lines[start..].join("\n");
    (!tail.trim().is_empty()).then_some(tail)
}

/// The blake3 hex digest of the sorted test IDs and the content of each distinct file the tests
/// name under `root` (a file that cannot be read contributes its name only).
fn suite_hash(root: &Path, tests: &[TestResult]) -> String {
    let mut hasher = blake3::Hasher::new();
    for test in tests {
        hasher.update(test.id.as_bytes());
        hasher.update(&[0]);
    }
    let files: BTreeSet<&str> = tests
        .iter()
        .filter_map(|test| test.file.as_deref())
        .collect();
    for file in files {
        hasher.update(file.as_bytes());
        hasher.update(&[0]);
        if let Ok(content) = std::fs::read(root.join(file)) {
            hasher.update(&content);
        }
        hasher.update(&[0]);
    }
    hasher.finalize().to_hex().to_string()
}
