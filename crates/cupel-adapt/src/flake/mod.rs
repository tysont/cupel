// ABOUTME: The flake engine, `engine.flake` (ADR-078): reruns the baseline's tests with the baseline's registered
// ABOUTME: runner, varying order, seed, and parallelism by a schedule from the run's seed, into `TestReruns`.
//! The flake engine.
//!
//! `engine.flake` is a [`Producer`] of [`TestReruns`] that needs [`TestResults`]: the baseline
//! must be green (ADR-076) before any rerun, and it names the runner to rerun. Each rerun's
//! order, seed, and parallelism come from [`options::schedule`]; the engine hands them to the
//! baseline's runner, which it finds among the registered runners (`ProduceCtx::runners`), as
//! [`RunOptions`](cupel_core::RunOptions). The runner runs every process against the run's cancellation, so
//! `--timeout` stops a rerun (exit 5). A run over a range reruns only the tests the range
//! impacts, which the run chose before it started as `tests --impacted` chooses them
//! ([`Focus::impacted`](cupel_core::Focus), ADR-186, ADR-190).

mod options;

use cupel_core::{
    Availability, CostClass, Ctx, Error, EventSink, FactKind, Langs, ProduceCtx, Producer,
    ProducerId, Result, RunOrdering, Runner, Scope, TestRerun, TestReruns, TestResult, TestResults,
    TestSelection,
};
use serde_json::Value;

pub use crate::report::test2json::order as go_test_order;
pub use options::{DEFAULT_RUNS, Planned, rerun_seed, schedule};

const ID: &str = "engine.flake";

/// The `[lang.<id>]` key that sets how many reruns the engine makes.
pub const RUNS_KEY: &str = "flake_runs";

/// Reruns the baseline's tests (ADR-078).
pub struct FlakeEngine;

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<TestReruns, _>(FlakeEngine),
});

impl Producer<TestReruns> for FlakeEngine {
    fn id(&self) -> ProducerId {
        ProducerId::from(ID)
    }

    fn languages(&self) -> Langs {
        Langs::Any
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::TestResults]
    }

    /// The project roots the runner reruns in, as the baseline ran.
    fn wants(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    fn cost(&self) -> CostClass {
        CostClass::Standard
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        _: &mut dyn EventSink,
    ) -> Result<TestReruns> {
        let Some(baseline) = ctx.upstream.get::<TestResults>() else {
            return Ok(TestReruns {
                runner: self.id(),
                seed: ctx.seed,
                runs: Vec::new(),
                impacted: None,
            });
        };
        let runner = ctx
            .runners
            .get(&baseline.runner)
            .ok_or_else(|| Error::InvalidValue {
                what: "runner for engine.flake",
                value: baseline.runner.to_string(),
                expected: "a runner the flake engine reruns: tool.cargo-test, tool.go-test, \
                           tool.pytest, tool.unittest, tool.vitest, tool.jest, or tool.node-test",
            })?;
        // One process per test: no order of the tests can change an outcome (ADR-158).
        if runner.ordering() == RunOrdering::Fixed {
            return Err(Error::Unsupported {
                fact: FactKind::TestReruns,
                tool: runner.tool(&ctx.ctx).map(|probe| probe.name.to_string()),
                message: format!(
                    "{ID} does not rerun {}: it runs each test in its own process, so the \
                     order of the tests cannot change an outcome",
                    baseline.runner
                ),
            });
        }
        let impacted = impacted(ctx, &baseline.runner);
        let explicit = runner.ordering() == RunOrdering::Explicit;
        let mut runs: Vec<TestRerun> = Vec::new();
        if impacted
            .as_ref()
            .is_none_or(|impacted| !impacted.is_empty())
        {
            for planned in schedule(ctx.seed, rerun_count(&ctx.config)?) {
                let first = runs.first().and_then(|run| run.executed.as_deref());
                let planned = planned.resolved(explicit, first);
                runs.push(rerun(ctx, runner, &planned, scope, impacted.as_ref())?);
            }
        }
        Ok(TestReruns {
            runner: baseline.runner.clone(),
            seed: ctx.seed,
            runs,
            impacted: impacted.map(Impacted::names),
        })
    }
}

/// The tests a range impacts: by the runner's test IDs, or by test file.
enum Impacted {
    Tests(Vec<String>),
    Files(Vec<String>),
}

impl Impacted {
    fn is_empty(&self) -> bool {
        match self {
            Self::Tests(names) | Self::Files(names) => names.is_empty(),
        }
    }

    fn names(self) -> Vec<String> {
        match self {
            Self::Tests(names) | Self::Files(names) => names,
        }
    }

    fn selection(&self) -> TestSelection {
        match self {
            Self::Tests(ids) => TestSelection::Tests(ids.clone()),
            Self::Files(files) => TestSelection::Files(files.clone()),
        }
    }

    /// Whether `test` is one of them.
    fn holds(&self, test: &TestResult) -> bool {
        match self {
            Self::Tests(ids) => ids.contains(&test.id),
            Self::Files(files) => test.file.as_ref().is_some_and(|file| files.contains(file)),
        }
    }
}

/// The tests the run's range impacts, as the run chose them (ADR-190): by test ID when they are
/// `runner`'s IDs, or by test file; `None` without a range, or when the run could not choose
/// them in a way `runner` can rerun, when every test reruns.
fn impacted(ctx: &ProduceCtx, runner: &ProducerId) -> Option<Impacted> {
    let impacted = ctx.focus.impacted.as_ref()?;
    match &impacted.selection {
        TestSelection::Tests(ids) if impacted.runner.as_ref() == Some(runner) => {
            Some(Impacted::Tests(ids.clone()))
        }
        TestSelection::Files(files) => Some(Impacted::Files(files.clone())),
        _ => None,
    }
}

/// The `impacted` tests as a selection, when there are some and the runner can narrow its suite
/// to them; else the whole suite.
fn narrowest(ctx: &ProduceCtx, runner: &dyn Runner, impacted: Option<&Impacted>) -> TestSelection {
    impacted
        .map(Impacted::selection)
        .filter(|selection| runner.commands(ctx, selection).is_some())
        .unwrap_or(TestSelection::All)
}

/// `[lang.<id>] flake_runs`, else [`DEFAULT_RUNS`].
fn rerun_count(config: &serde_json::Map<String, Value>) -> Result<u32> {
    match config.get(RUNS_KEY) {
        None => Ok(DEFAULT_RUNS),
        Some(value) => value
            .as_u64()
            .and_then(|runs| u32::try_from(runs).ok())
            .ok_or_else(|| Error::InvalidValue {
                what: "flake_runs",
                value: value.to_string(),
                expected: "a non-negative integer",
            }),
    }
}

/// One rerun as `planned`, its tests narrowed to the `impacted` ones, else to those in scope (or
/// with no file). Its order is kept when the runner reports one and ran one test at a time or
/// in the order given.
fn rerun(
    ctx: &ProduceCtx,
    runner: &dyn Runner,
    planned: &Planned,
    scope: &Scope,
    impacted: Option<&Impacted>,
) -> Result<TestRerun> {
    let selection = if planned.explicit.is_empty() {
        narrowest(ctx, runner, impacted)
    } else {
        TestSelection::Tests(planned.explicit.clone())
    };
    let explicit = runner.ordering() == RunOrdering::Explicit;
    let ran = runner.run(ctx, &selection, &planned.options())?;
    let files = &scope.files;
    let mut tests = ran.results.tests;
    tests.retain(|test: &TestResult| match impacted {
        Some(impacted) => impacted.holds(test),
        None => files.is_empty() || test.file.as_ref().is_none_or(|file| files.contains(file)),
    });
    Ok(TestRerun {
        order: planned.order,
        seed: planned.seed,
        parallelism: if explicit {
            Some(1)
        } else {
            planned.parallelism
        },
        executed: ran.executed.filter(|_| planned.serial() || explicit),
        invocations: ran.results.invocations,
        tests,
    })
}
