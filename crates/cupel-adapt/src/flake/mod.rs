// ABOUTME: The flake engine, `engine.flake` (ADR-078): reruns the baseline's tests with the baseline's runner,
// ABOUTME: varying order, seed, and parallelism by a schedule from the run's seed, into `TestReruns`.
//! The flake engine.
//!
//! `engine.flake` is a [`Producer`] of [`TestReruns`] that needs [`TestResults`]: the baseline
//! must be green (ADR-076) before any rerun, and it names the runner and the project roots to
//! rerun. Each rerun's [`RunOptions`] come from [`options::schedule`]; `reruns` turns them
//! into each runner's arguments and environment and reads the report with the runner's own
//! parser. The engine runs every process through [`Process`] against the run's cancellation,
//! so `--timeout` stops a rerun (exit 5).

mod options;
mod order;
mod reruns;

use std::path::Path;

use cupel_core::{
    Availability, CostClass, Ctx, Error, EventSink, FactKind, Langs, Process, ProduceCtx, Producer,
    ProducerId, Result, RunnerInvocation, Scope, TestRerun, TestReruns, TestResult, TestResults,
    find_program, output_tail,
};
use serde_json::Value;

pub use options::{DEFAULT_RUNS, RunOptions, rerun_seed, schedule};
pub use order::go_test_order;
pub use reruns::{RerunSpec, runners};

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
        let spec = reruns::spec(baseline.runner.as_str()).ok_or_else(|| Error::InvalidValue {
            what: "runner for engine.flake",
            value: baseline.runner.to_string(),
            expected: "a runner the flake engine reruns: tool.cargo-test, tool.go-test, \
                       tool.pytest, tool.unittest, tool.vitest, or tool.jest",
        })?;
        let mut runs: Vec<TestRerun> = Vec::new();
        for (index, planned) in schedule(ctx.seed, rerun_count(&ctx.config)?)
            .into_iter()
            .enumerate()
        {
            let first = runs.first().and_then(|run| run.executed.as_deref());
            let options = planned.resolved(spec.explicit_order, first);
            runs.push(rerun(ctx, spec, baseline, &options, index, scope)?);
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

/// Rerun `index` with `options` in every project root the baseline ran in; its tests narrowed
/// to those in `scope` (or with no file), its order the whole run's.
fn rerun(
    ctx: &ProduceCtx,
    spec: &RerunSpec,
    baseline: &TestResults,
    options: &RunOptions,
    index: usize,
    scope: &Scope,
) -> Result<TestRerun> {
    let mut invocations = Vec::new();
    let mut tests = Vec::new();
    let mut executed = Vec::new();
    for (root, ran) in baseline.invocations.iter().enumerate() {
        let area = ctx.writer.create_dir(
            &ctx.writer
                .cache_dir()
                .join("flake")
                .join(index.to_string())
                .join(root.to_string()),
        )?;
        let (invocation, found, order) = rerun_root(ctx, spec, ran, options, &area)?;
        invocations.push(invocation);
        executed.extend(order.into_iter().map(|id| placed_id(id, &ran.dir)));
        tests.extend(found.into_iter().map(|test| placed(test, &ran.dir)));
    }
    tests.retain(|test: &TestResult| {
        scope.files.is_empty()
            || test
                .file
                .as_ref()
                .is_none_or(|file| scope.files.contains(file))
    });
    tests.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(TestRerun {
        order: options.order,
        seed: options.seed,
        parallelism: if spec.explicit_order {
            Some(1)
        } else {
            options.parallelism
        },
        executed: (spec.reports_order && (options.serial() || spec.explicit_order))
            .then_some(executed),
        invocations,
        tests,
    })
}

/// One rerun in the root the baseline invocation `ran` ran in: what ran, the tests it reported
/// relative to that root, and their IDs in the order they ran.
fn rerun_root(
    ctx: &ProduceCtx,
    spec: &RerunSpec,
    ran: &RunnerInvocation,
    options: &RunOptions,
    area: &Path,
) -> Result<(RunnerInvocation, Vec<TestResult>, Vec<String>)> {
    let dir = ctx.ctx.root.join(&ran.dir);
    let dir = dir.canonicalize().unwrap_or(dir);
    let prefix = format!("{}: ", ran.dir);
    let in_root = RunOptions {
        explicit: options
            .explicit
            .iter()
            .filter_map(|id| {
                if ran.dir == "." {
                    Some(id.clone())
                } else {
                    id.strip_prefix(&prefix).map(str::to_owned)
                }
            })
            .collect(),
        ..options.clone()
    };
    let rerun = reruns::Rerun {
        program: program(spec, &dir).unwrap_or_else(|| spec.tool.to_owned()),
        report: area.join("report"),
        options: &in_root,
        baseline: &ran.command,
    };
    let _ = std::fs::remove_file(&rerun.report);
    let command = (spec.command)(&rerun);
    let (program, args) = command.argv.split_first().unwrap_or((&rerun.program, &[]));
    let mut process = Process::new(program, &dir).args(args.iter().cloned());
    for (key, value) in &command.env {
        process = process.env(key, value);
    }
    if command.merged {
        process = process.merged();
    }
    let captured = process.run(&ctx.cancel)?;
    let found = (spec.read)(&reruns::Ran {
        dir: &dir,
        captured: &captured,
        report: std::fs::read_to_string(&rerun.report).ok(),
    });
    let completed = found.is_some();
    let order = match spec.ran_order {
        Some(ran_order) => ran_order(&captured),
        None => found.iter().flatten().map(|test| test.id.clone()).collect(),
    };
    let invocation = RunnerInvocation {
        dir: ran.dir.clone(),
        command: process.command_line(),
        exit_code: captured.exit_code,
        completed,
        output: (!completed)
            .then(|| output_tail(&format!("{}\n{}", captured.stdout, captured.stderr)))
            .flatten(),
    };
    Ok((invocation, found.unwrap_or_default(), order))
}

/// The program to run in `dir`: the root's `node_modules/.bin` copy when the runner may use
/// one, else the tool on `PATH`.
fn program(spec: &RerunSpec, dir: &Path) -> Option<String> {
    let local = dir.join("node_modules").join(".bin").join(spec.tool);
    if spec.node_bin && local.is_file() {
        return Some(local.to_string_lossy().into_owned());
    }
    find_program(spec.tool).map(|_| spec.tool.to_owned())
}

/// A test ID reported relative to the root at `dir`, as the baseline names it (ADR-075).
fn placed_id(id: String, dir: &str) -> String {
    if dir == "." {
        id
    } else {
        format!("{dir}: {id}")
    }
}

/// `test`, reported relative to the root at `dir`, made relative to the repository root.
fn placed(mut test: TestResult, dir: &str) -> TestResult {
    test.id = placed_id(test.id, dir);
    if dir != "." {
        test.file = test.file.map(|file| format!("{dir}/{file}"));
    }
    test
}
