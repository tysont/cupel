// ABOUTME: A narrowed mutant's tests run one at a time (ADR-160): fastest first by baseline duration, each under its own
// ABOUTME: cap within the mutant's overall cap; what each run says (failed, hung, slow), and the slowest a timeout names.
use std::collections::BTreeMap;
use std::time::Duration;

use cupel_core::UnfinishedTest;

use crate::settings::TIMEOUT_FLOOR;

/// One selected test and its duration in the baseline, when the baseline timed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Test {
    pub id: String,
    pub baseline: Option<Duration>,
}

/// `ids` in the order a mutant runs them: ascending by `baseline` duration, those it did not time last, ties by ID.
pub fn order(ids: &[String], baseline: &BTreeMap<String, Duration>) -> Vec<Test> {
    let mut tests: Vec<Test> = ids
        .iter()
        .map(|id| Test {
            id: id.clone(),
            baseline: baseline.get(id).copied(),
        })
        .collect();
    tests.sort_by(|a, b| {
        (a.baseline.is_none(), a.baseline, &a.id).cmp(&(b.baseline.is_none(), b.baseline, &b.id))
    });
    tests
}

/// The timing of one mutant's sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    /// A test's own cap is this many times its baseline duration (`timeout_factor`).
    pub factor: u32,
    /// The runner's start-up for a run of one test, measured in the slot.
    pub startup: Duration,
    /// The longest the whole sequence may take (`max_mutant_timeout`).
    pub overall: Duration,
}

/// The timeout of one test's run, and whether it is the test's own cap rather than what was left of the overall cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    pub timeout: Duration,
    pub own: bool,
}

impl Caps {
    /// A test's own cap, start-up aside: `factor` times its baseline duration, at least the floor.
    fn own(&self, baseline: Duration) -> Duration {
        TIMEOUT_FLOOR.max(baseline * self.factor)
    }

    /// The timeout of `test`'s run after `spent` of the sequence, allowing `build` more for a run that rebuilds:
    /// the start-up and its own cap, or what is left of the overall cap when that is less or its duration unknown.
    pub fn limit(&self, test: &Test, spent: Duration, build: Duration) -> Limit {
        let left = self.overall.saturating_sub(spent);
        match test.baseline {
            Some(baseline) => {
                let own = self.startup + self.own(baseline) + build;
                Limit {
                    timeout: own.min(left),
                    own: own <= left,
                }
            }
            None => Limit {
                timeout: left,
                own: false,
            },
        }
    }
}

/// What one test's run reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    /// The test failed.
    pub failed: bool,
    /// The run completed and reported the test.
    pub complete: bool,
    /// The run reached its timeout.
    pub reached: bool,
}

/// What one test's run says about the mutant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// It passed: the next test runs.
    Passed,
    /// It failed: the mutant is killed.
    Failed,
    /// It ran past its own cap though it normally finishes in under a quarter of it: the mutant is killed.
    Hung,
    /// Its time ran out for slowness: the mutant's outcome is unknown.
    Slow,
    /// It ended incomplete before its timeout: the mutant did not build or load.
    Unviable,
}

/// What `run` of `test`, under `limit`, says.
pub fn verdict(run: &Run, limit: Limit, test: &Test, caps: &Caps) -> Verdict {
    if run.failed {
        return Verdict::Failed;
    }
    if run.reached {
        let hung = limit.own
            && test
                .baseline
                .is_some_and(|baseline| baseline * 4 < caps.own(baseline));
        return if hung { Verdict::Hung } else { Verdict::Slow };
    }
    if run.complete {
        Verdict::Passed
    } else {
        Verdict::Unviable
    }
}

/// How many of the tests left a timeout names.
const NAMED: usize = 3;

/// Of the tests `left` uncompleted, the slowest a timeout names: by baseline duration, longest first, those of
/// unknown duration after.
pub fn slowest(left: &[Test]) -> Vec<UnfinishedTest> {
    let mut left: Vec<&Test> = left.iter().collect();
    left.sort_by(|a, b| {
        (b.baseline.is_some(), b.baseline, &a.id).cmp(&(a.baseline.is_some(), a.baseline, &b.id))
    });
    left.into_iter()
        .take(NAMED)
        .map(|test| UnfinishedTest {
            id: test.id.clone(),
            baseline_ms: test
                .baseline
                .map(|baseline| u64::try_from(baseline.as_millis()).unwrap_or(u64::MAX)),
        })
        .collect()
}
