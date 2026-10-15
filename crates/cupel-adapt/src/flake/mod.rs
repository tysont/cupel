// ABOUTME: The flake engine, `engine.flake` (ADR-078): reruns the baseline's tests with the baseline's registered
// ABOUTME: runner, varying order, seed, and parallelism by a schedule from the run's seed, into `TestReruns`.
//! The flake engine.
//!
//! `engine.flake` is a [`Producer`] of [`TestReruns`] that needs [`TestResults`]: the baseline
//! must be green (ADR-076) before any rerun, and it names the runner to rerun. Each rerun's
//! order, seed, and parallelism come from [`options::schedule`]; the engine hands them to the
//! baseline's runner, which it finds among the registered runners (`ProduceCtx::runners`), as
//! [`RunOptions`](cupel_core::RunOptions). The runner runs every process against the run's cancellation, so
//! `--timeout` stops a rerun (exit 5).

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
            });
        };
        let runner = ctx
            .runners
            .get(&baseline.runner)
            .filter(|runner| runner.ordering() != RunOrdering::Fixed)
            .ok_or_else(|| Error::InvalidValue {
                what: "runner for engine.flake",
                value: baseline.runner.to_string(),
                expected: "a runner the flake engine reruns: tool.cargo-test, tool.go-test, \
                           tool.pytest, tool.unittest, tool.vitest, or tool.jest",
            })?;
        let explicit = runner.ordering() == RunOrdering::Explicit;
        let mut runs: Vec<TestRerun> = Vec::new();
        for planned in schedule(ctx.seed, rerun_count(&ctx.config)?) {
            let first = runs.first().and_then(|run| run.executed.as_deref());
            let planned = planned.resolved(explicit, first);
            runs.push(rerun(ctx, runner, &planned, scope)?);
        }
        Ok(TestReruns {
            runner: baseline.runner.clone(),
            seed: ctx.seed,
            runs,
        })
    }
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

/// One rerun as `planned`, its tests narrowed to those in scope (or with no file). Its order is
/// kept when the runner reports one and ran one test at a time or in the order given.
fn rerun(
    ctx: &ProduceCtx,
    runner: &dyn Runner,
    planned: &Planned,
    scope: &Scope,
) -> Result<TestRerun> {
    let selection = if planned.explicit.is_empty() {
        TestSelection::All
    } else {
        TestSelection::Tests(planned.explicit.clone())
    };
    let explicit = runner.ordering() == RunOrdering::Explicit;
    let ran = runner.run(ctx, &selection, &planned.options())?;
    let files = &scope.files;
    let mut tests = ran.results.tests;
    tests.retain(|test: &TestResult| {
        files.is_empty() || test.file.as_ref().is_none_or(|file| files.contains(file))
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
