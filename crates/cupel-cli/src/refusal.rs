// ABOUTME: The findings of a refused run (exit 4, ADR-076 §6): one `baseline.test-failed` per failing or
// ABOUTME: errored baseline test and one `baseline.run-incomplete` per runner run that did not finish.
use cupel_core::{ActionKind, RunnerInvocation, Severity, TestResult, Unevaluable};
use serde_json::json;

use crate::problem::Problem;

/// Why scores are refused, for the detail of every refusal finding.
const DETAIL: &str = "Cupel scores nothing that depends on test execution while the baseline \
                      suite is red (exit 4). Fix the suite, then rerun.";

/// One finding per failing test, then one per incomplete run.
pub fn findings(red: &Unevaluable) -> Vec<Problem> {
    red.results
        .failures()
        .map(|test| failed(red, test))
        .chain(red.results.incomplete().map(|run| incomplete(red, run)))
        .collect()
}

/// The first of [`findings`], or a summary when there is none (which a red suite never has).
pub fn summary(red: &Unevaluable) -> Problem {
    findings(red).into_iter().next().unwrap_or_else(|| Problem {
        check: "baseline",
        kind: "baseline.run-incomplete",
        subject: red.lang.to_string(),
        severity: Severity::Fail,
        title: format!("The baseline test suite for {} is red", red.lang),
        detail: Some(DETAIL.to_owned()),
        evidence: json!({"lang": red.lang, "runner": red.results.runner}),
        action: ActionKind::FixBug,
        fix: "Make the test suite pass, then rerun.".to_owned(),
    })
}

fn failed(red: &Unevaluable, test: &TestResult) -> Problem {
    Problem {
        check: "baseline",
        kind: "baseline.test-failed",
        subject: format!("{}:{}", red.lang, test.id),
        severity: Severity::Fail,
        title: format!(
            "Baseline test `{}` {} ({}, {})",
            test.name, test.outcome, red.lang, red.results.runner
        ),
        detail: Some(DETAIL.to_owned()),
        evidence: json!({
            "lang": red.lang,
            "runner": red.results.runner,
            "test": test.id,
            "file": test.file,
            "outcome": test.outcome,
            "output": test.output,
        }),
        action: ActionKind::FixBug,
        fix: format!("Make `{}` pass, or skip it, then rerun.", test.id),
    }
}

fn incomplete(red: &Unevaluable, run: &RunnerInvocation) -> Problem {
    let code = run
        .exit_code
        .map_or_else(|| "a signal".to_owned(), |code| format!("exit code {code}"));
    Problem {
        check: "baseline",
        kind: "baseline.run-incomplete",
        subject: format!("{}:{}", red.lang, run.dir),
        severity: Severity::Fail,
        title: format!(
            "{} did not finish the {} suite in `{}` ({code})",
            red.results.runner, red.lang, run.dir
        ),
        detail: Some(DETAIL.to_owned()),
        evidence: json!({
            "lang": red.lang,
            "runner": red.results.runner,
            "dir": run.dir,
            "command": run.command,
            "exit_code": run.exit_code,
            "output": run.output,
        }),
        action: ActionKind::FixBug,
        fix: format!(
            "Run `{}` in `{}` and make it build and finish, then rerun.",
            run.command.join(" "),
            run.dir
        ),
    }
}
