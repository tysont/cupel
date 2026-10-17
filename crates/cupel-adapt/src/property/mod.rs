// ABOUTME: The property engine, `engine.property` (ADR-095): runs the property tests `TestFrames` finds in scope once,
// ABOUTME: with the baseline's runner, the run's seed, and a case budget, into `PropertyResults` with counterexamples.
//! The property engine.
//!
//! `engine.property` is a [`Producer`] of [`PropertyResults`] that needs [`TestResults`] (the
//! baseline must be green, ADR-076, and names the runner) and [`TestFrames`] (which cases are
//! properties, `TestCase::property`). It hands each framework present the run's seed and the
//! case budget ([`Focus::case_budget`](cupel_core::Focus)) the framework's own way
//! ([`frameworks`]), runs the property tests through the baseline's runner as narrowly as the
//! runner allows, and reads each failure's shrunk counterexample back ([`counterexample`]).

mod counterexample;
mod frameworks;

use std::collections::BTreeSet;

use cupel_core::syntax::{TestCase, TestFrames};
use cupel_core::{
    Availability, CostClass, Ctx, Error, EventSink, FactKind, LangId, Langs, ProduceCtx, Producer,
    ProducerId, PropertyOutcome, PropertyResult, PropertyResults, Result, RunOptions, Runner,
    Scope, TestOutcome, TestResult, TestResults, TestSelection, names_case,
};

use crate::toolchain;
use frameworks::{Budget, FRAMEWORKS, Framework};

const ID: &str = "engine.property";

/// Runs the property tests in scope with a seed and a case budget (ADR-095).
pub struct PropertyEngine;

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<PropertyResults, _>(PropertyEngine),
});

impl Producer<PropertyResults> for PropertyEngine {
    fn id(&self) -> ProducerId {
        ProducerId::from(ID)
    }

    /// The languages whose toolchain rows name a framework the engine budgets.
    fn languages(&self) -> Langs {
        let budgeted = |row: &&toolchain::Row| {
            row.property
                .iter()
                .any(|tool| frameworks::find(tool.as_str()).is_some())
        };
        let langs: BTreeSet<LangId> = toolchain::rows()
            .iter()
            .filter(budgeted)
            .flat_map(|row| row.langs.iter().cloned())
            .collect();
        Langs::Only(langs.into_iter().collect())
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::TestResults, FactKind::TestFrames]
    }

    /// The project roots the runner runs in, as the baseline ran.
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
    ) -> Result<PropertyResults> {
        let budget = Budget {
            cases: ctx.focus.case_budget(),
            seed: ctx.seed,
        };
        let Some(baseline) = ctx.upstream.get::<TestResults>() else {
            return Ok(results(self.id(), &budget, Vec::new()));
        };
        let mut produced = results(baseline.runner.clone(), &budget, Vec::new());
        let cases = property_cases(ctx.upstream.get::<TestFrames>(), scope);
        if cases.is_empty() {
            return Ok(produced);
        }
        let runner = ctx
            .runners
            .get(&baseline.runner)
            .ok_or_else(|| Error::InvalidValue {
                what: "runner for engine.property",
                value: baseline.runner.to_string(),
                expected: "a registered runner",
            })?;
        let options = options(ctx, &baseline.runner, &cases, &budget)?;
        let ids: Vec<Option<String>> = cases
            .iter()
            .map(|case| reported(case, &baseline.tests).map(|test| test.id.clone()))
            .collect();
        let selection = selection(ctx, runner, &cases, &ids);
        let ran = runner.run(ctx, &selection, &options)?;
        produced.properties = cases
            .iter()
            .zip(ids)
            .map(|(case, id)| result(case, id, &ran.results.tests))
            .collect();
        Ok(produced)
    }
}

/// The results of a run with `budget` and no properties yet.
fn results(
    runner: ProducerId,
    budget: &Budget,
    properties: Vec<PropertyResult>,
) -> PropertyResults {
    PropertyResults {
        frameworks: frameworks_of(&runner),
        runner,
        seed: budget.seed,
        cases: budget.cases,
        properties,
    }
}

/// The property frameworks of the language `runner` runs: its toolchain row's `property` column.
fn frameworks_of(runner: &ProducerId) -> Vec<String> {
    toolchain::rows()
        .iter()
        .find(|row| row.runners.iter().any(|tool| tool.id == *runner))
        .map(|row| row.property.iter().map(ToString::to_string).collect())
        .unwrap_or_default()
}

/// A property case in scope: its file, the case, and its framework.
struct Case<'a> {
    file: &'a str,
    test: &'a TestCase,
    framework: &'static Framework,
}

/// Every case in scope a framework the engine budgets drives, by file, then line.
fn property_cases<'a>(frames: Option<&'a TestFrames>, scope: &Scope) -> Vec<Case<'a>> {
    let mut cases: Vec<Case<'a>> = frames
        .into_iter()
        .flat_map(|frames| &frames.files)
        .filter(|file| scope.files.is_empty() || scope.files.contains(&file.file))
        .flat_map(|file| {
            file.cases.iter().filter_map(move |case| {
                let framework = frameworks::find(case.property.as_deref()?)?;
                Some(Case {
                    file: &file.file,
                    test: case,
                    framework,
                })
            })
        })
        .collect();
    cases.sort_by_key(|case| (case.file, case.test.span.line));
    cases
}

/// The options that hand every framework among `cases` the budget, merged.
fn options(
    ctx: &ProduceCtx,
    runner: &ProducerId,
    cases: &[Case<'_>],
    budget: &Budget,
) -> Result<RunOptions> {
    let mut merged = RunOptions::default();
    for framework in &FRAMEWORKS {
        if cases
            .iter()
            .any(|case| case.framework.name == framework.name)
        {
            let options = framework.options(ctx, runner, budget)?;
            merged.env.extend(options.env);
            merged.args.extend(options.args);
        }
    }
    Ok(merged)
}

/// The test the baseline reported for `case`: the one in its file with its name, else the one
/// anywhere with its name (a runner that names a test's target rather than its module file).
fn reported<'t>(case: &Case<'_>, tests: &'t [TestResult]) -> Option<&'t TestResult> {
    let named: Vec<&TestResult> = tests
        .iter()
        .filter(|test| names_case(&test.name, &case.test.name))
        .collect();
    let in_file: Vec<&TestResult> = named
        .iter()
        .copied()
        .filter(|test| test.file.as_deref() == Some(case.file))
        .collect();
    match (in_file.as_slice(), named.as_slice()) {
        ([test], _) | ([], [test]) => Some(test),
        _ => None,
    }
}

/// The narrowest selection `runner` takes: the cases' test IDs, else their files, else the suite.
fn selection(
    ctx: &ProduceCtx,
    runner: &dyn Runner,
    cases: &[Case<'_>],
    ids: &[Option<String>],
) -> TestSelection {
    let tests = TestSelection::Tests(ids.iter().flatten().cloned().collect());
    let mut files: Vec<String> = cases.iter().map(|case| case.file.to_owned()).collect();
    files.dedup();
    let files = TestSelection::Files(files);
    [tests, files]
        .into_iter()
        .find(|selection| runner.commands(ctx, selection).is_some())
        .unwrap_or(TestSelection::All)
}

/// What the run said about `case`, whose test is `id`.
fn result(case: &Case<'_>, id: Option<String>, tests: &[TestResult]) -> PropertyResult {
    let ran = id
        .as_ref()
        .and_then(|id| tests.iter().find(|test| test.id == *id));
    let counterexample = ran
        .filter(|test| test.outcome.is_failure())
        .and_then(|test| test.output.as_deref())
        .and_then(case.framework.counterexample);
    let outcome = match (ran.map(|test| test.outcome), &counterexample) {
        (None, _) => PropertyOutcome::NotRun,
        (Some(TestOutcome::Passed), _) => PropertyOutcome::Passed,
        (Some(TestOutcome::Skipped), _) => PropertyOutcome::Skipped,
        (Some(_), Some(_)) => PropertyOutcome::Falsified,
        (Some(_), None) => PropertyOutcome::Failed,
    };
    PropertyResult {
        file: case.file.to_owned(),
        name: case.test.name.clone(),
        span: case.test.span,
        framework: case.framework.name.to_owned(),
        test: id,
        outcome,
        counterexample,
        seeded: case.framework.seeded,
    }
}
