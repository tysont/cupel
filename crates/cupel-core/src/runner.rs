// ABOUTME: The `Runner` trait (design §8.7, ADR-077): a `TestResults` producer that can also list its tests,
// ABOUTME: run a selection of them with extra environment and a timeout, and name the commands that run one.
use std::time::Duration;

use crate::error::Result;
use crate::producer::{ProduceCtx, Producer};
use crate::test_results::TestResults;

/// Which tests a runner runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestSelection {
    /// The whole suite.
    All,
    /// These tests, by the IDs the runner's `TestResults` use (ADR-075).
    Tests(Vec<String>),
    /// The tests in these files, relative to the repository root.
    Files(Vec<String>),
}

/// A command a person or agent runs to run a selection in one project root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerCommand {
    /// The project root it runs in, relative to the repository root; `.` for the root.
    pub dir: String,
    /// Shell words, ready to paste in `dir`.
    pub command: String,
}

/// A test runner: produces `TestResults` for the whole suite, and runs selections of it for the
/// consumers that choose tests (`tests --impacted`, the mutation engine).
pub trait Runner: Producer<TestResults> {
    /// The IDs of the suite's tests, without running them.
    fn list(&self, ctx: &ProduceCtx) -> Result<Vec<String>>;

    /// Runs `selection` with `env` added to the runner's environment. The run's cancellation
    /// still applies (`Error::Cancelled`); `timeout` ends only this run, which then reports an
    /// incomplete invocation.
    fn run(
        &self,
        ctx: &ProduceCtx,
        selection: &TestSelection,
        env: &[(String, String)],
        timeout: Option<Duration>,
    ) -> Result<TestResults>;

    /// The command lines that run `selection`, one per project root it reaches; `None` when the
    /// runner cannot narrow its suite to it.
    fn commands(&self, ctx: &ProduceCtx, selection: &TestSelection) -> Option<Vec<RunnerCommand>>;
}
