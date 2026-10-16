// ABOUTME: Integration tests proving each exit code reachable so far via a deliberate trigger,
// ABOUTME: and that every error path still writes one valid envelope to stdout.
mod support;

use serde_json::{Value, json};
use support::{cli_finding_id, cupel, run, run_with};

#[test]
fn exit_0_pass_for_a_successful_verb() {
    let envelope = run(["describe"]).envelope();
    assert_eq!(envelope["status"], json!("pass"));
    assert_eq!(envelope["exit_code"], json!(0));
    assert_eq!(envelope["findings"], json!([]));
}

#[test]
fn exit_2_for_an_unknown_flag_names_the_flag_and_a_fix() {
    let outcome = run(["describe", "--frobnicate"]);
    assert_eq!(outcome.code, 2);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("error"));
    let finding = outcome.finding();
    assert_eq!(finding["check"], json!("cli"));
    assert_eq!(finding["kind"], json!("cli.usage"));
    assert_eq!(finding["severity"], json!("fail"));
    assert_eq!(finding["tier"], json!(0));
    assert_eq!(finding["symbol"], Value::Null);
    assert_eq!(finding["location"], Value::Null);
    assert_eq!(finding["evidence"]["argument"], json!("--frobnicate"));
    assert_eq!(
        finding["id"],
        json!(cli_finding_id("cli.usage", "--frobnicate"))
    );
    assert_eq!(finding["repro"], json!("cupel describe --frobnicate"));
    assert!(!finding["action"]["summary"].as_str().unwrap().is_empty());
}

#[test]
fn exit_2_for_a_misspelled_flag_suggests_the_right_one() {
    let finding = run(["describe", "--formt", "json"]).finding();
    assert_eq!(
        finding["action"]["summary"],
        json!("Did you mean --format?")
    );
}

#[test]
fn exit_2_for_an_unknown_verb_has_no_verb_and_lists_the_verbs() {
    let outcome = run(["frobnicate"]);
    let finding = outcome.finding();
    assert_eq!(
        (outcome.code, &outcome.envelope()["run"]["verb"]),
        (2, &Value::Null)
    );
    assert_eq!(finding["evidence"]["value"], json!("frobnicate"));
    assert_eq!(
        finding["action"]["summary"],
        json!(
            "Use one of: init, doctor, describe, check, survey, explain, baseline, oracle, tests, \
             suppress, serve."
        )
    );
}

#[test]
fn exit_2_for_no_arguments_writes_help_to_stderr_and_an_envelope_to_stdout() {
    let outcome = run::<[&str; 0], &str>([]);
    assert_eq!(outcome.code, 2);
    assert!(
        outcome.stderr.contains("Usage: cupel"),
        "{}",
        outcome.stderr
    );
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    assert_eq!(finding["id"], json!(cli_finding_id("cli.usage", "verb")));
}

#[test]
fn exit_2_for_a_missing_required_argument_names_it() {
    let finding = run(["suppress", "fnd_7c2e1a"]).finding();
    assert_eq!(finding["evidence"]["argument"], json!("--reason"));
    let fix = finding["action"]["summary"].as_str().unwrap();
    assert!(fix.contains("--reason"), "{fix}");
}

#[test]
fn exit_2_for_conflicting_flags_names_both() {
    let finding = run(["describe", "--quiet", "--verbose"]).finding();
    assert_eq!(
        finding["action"]["summary"],
        json!("Pass only one of --quiet and --verbose.")
    );
}

#[test]
fn exit_2_for_a_cwd_that_is_not_a_directory() {
    let outcome = run(["describe", "--cwd", "/definitely/not/a/dir"]);
    assert_eq!(outcome.code, 2);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    assert_eq!(finding["evidence"]["argument"], json!("--cwd"));
    assert_eq!(finding["evidence"]["value"], json!("/definitely/not/a/dir"));
}

/// The arguments of an example command line, without its leading `cupel`.
fn arguments(line: &str) -> Vec<&str> {
    line.split_whitespace().skip(1).collect()
}

/// Each pending verb with its first example and the plan step that builds it, in registry order.
fn pending_examples() -> Vec<(String, String, String)> {
    let cli = run(["describe"]).envelope()["description"].clone();
    let all = run(["describe", "--all"]).envelope()["description"].clone();
    let verbs = cli["verbs"].as_array().unwrap();
    let commands = all["commands"].as_array().unwrap();
    verbs
        .iter()
        .zip(commands)
        .filter_map(|(summary, command)| {
            let step = summary["pending_step"].as_str()?;
            let example = command["examples"][0]["command"].as_str().unwrap();
            let name = command["name"].as_str().unwrap();
            Some((name.to_owned(), example.to_owned(), step.to_owned()))
        })
        .collect()
}

/// Every example of `command` and of its sub-verbs, with the exit code it declares.
fn examples(command: &Value) -> Vec<(String, i64)> {
    let own = command["examples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|example| {
            (
                example["command"].as_str().unwrap().to_owned(),
                example["exit_code"].as_i64().unwrap(),
            )
        });
    let subs = command["subcommands"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(examples);
    own.chain(subs).collect()
}

#[test]
fn exit_2_for_every_unimplemented_verb_names_its_plan_step() {
    // Every verb but describe, init, doctor, check, survey, explain, baseline, tests, suppress, and
    // serve is pending: (verb, its first example, its plan step).
    let pending = [("oracle", "cupel oracle list", "P5.2")];
    let owned =
        pending.map(|(verb, example, step)| (verb.to_owned(), example.to_owned(), step.to_owned()));
    assert_eq!(pending_examples(), owned);
    let outcomes: Vec<(i32, Value, Value, bool)> = pending
        .iter()
        .map(|(_, example, step)| {
            let outcome = run(arguments(example));
            let finding = outcome.finding();
            let detail = finding["detail"].as_str().unwrap();
            (
                outcome.code,
                finding["kind"].clone(),
                outcome.envelope()["run"]["verb"].clone(),
                detail.contains(step),
            )
        })
        .collect();
    let expected: Vec<(i32, Value, Value, bool)> = pending
        .iter()
        .map(|(verb, _, _)| (2, json!("cli.not-implemented"), json!(verb), true))
        .collect();
    assert_eq!(outcomes, expected);
}

#[test]
fn every_registered_example_exits_with_its_declared_code() {
    let all = run(["describe", "--all"]).envelope()["description"].clone();
    let declared: Vec<(String, i64)> = all["commands"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(examples)
        .collect();
    let exited: Vec<(String, i64)> = declared
        .iter()
        .map(|(line, _)| (line.clone(), i64::from(run(arguments(line)).code)))
        .collect();
    assert_eq!(exited, declared);
    assert!(declared.len() >= 11, "{} examples", declared.len());
}

#[test]
fn seed_is_echoed_in_the_envelope() {
    let envelope = run(["describe", "--seed", "42"]).envelope();
    assert_eq!(envelope["run"]["seed"], json!(42));
}

#[test]
fn flags_that_do_not_apply_to_describe_change_nothing() {
    let baseline = support::redacted(run(["describe", "--seed", "1"]).envelope());
    let outcome = run([
        "describe",
        "--seed",
        "1",
        "--timeout",
        "10s",
        "--limit",
        "5",
        "--offset",
        "1",
        "--no-color",
        "--no-install",
        "--write",
    ]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(support::redacted(outcome.envelope()), baseline);
}

#[test]
fn exit_1_when_the_gate_fails() {
    let repo = support::Repo::new();
    repo.write(
        "src/branchy.rs",
        "pub fn branchy(a: i32) -> i32 {\n    if a > 1 { 1 } else if a > 2 { 2 } else { 3 }\n}\n",
    );
    repo.write(
        "cupel.toml",
        "[gate.tree]\n\"static.complexity.cyclomatic_max\" = \"< 3\"\n",
    );
    let outcome = repo.run(&["check", "--only", "static.complexity"]);
    assert_eq!(outcome.code, 1, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("fail"));
    assert_eq!(
        envelope["summary"]["gate"],
        json!({
            "result": "fail", "evaluated": "gate.tree",
            "rules": [{"rule": "static.complexity.cyclomatic_max < 3", "actual": 3.0,
                       "result": "fail"}]
        })
    );
}

#[test]
fn exit_3_when_doctor_finds_something_required_missing() {
    let repo = support::Repo::new();
    repo.write("cupel.toml", "required_version = \">=99.0\"\n");
    let outcome = repo.run(&["doctor"]);
    assert_eq!(outcome.code, 3, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("error"));
    assert_eq!(envelope["exit_code"], json!(3));
    assert_eq!(envelope["findings"][0]["check"], json!("doctor"));
}

// Exit 4 needs a check that depends on test execution, and none ships before P3.4; its trigger
// runs the CLI's entry point in-process with a test-only check in `runner_baseline.rs`
// (ADR-076 §8).

#[test]
fn exit_5_when_the_timeout_cuts_work_short_keeps_the_partial_results() {
    let started = std::time::Instant::now();
    let outcome = run_with(
        cupel().env("CUPEL_DEBUG_SLEEP_MS", "60000"),
        ["describe", "--timeout", "200ms"],
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "the slow task was cancelled, not waited out: {:?}",
        started.elapsed()
    );
    assert_eq!(outcome.code, 5, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("partial"));
    assert_eq!(envelope["exit_code"], json!(5));
    assert_eq!(envelope["findings"], json!([]));
    assert_eq!(
        envelope["description"]["subject"],
        json!("cli"),
        "results completed before the timeout are included"
    );
}

#[test]
fn exit_6_for_an_internal_error_carries_a_bug_report() {
    let outcome = run_with(cupel().env("CUPEL_DEBUG_PANIC", "1"), ["describe"]);
    assert_eq!(outcome.code, 6);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("error"));
    assert_eq!(envelope["run"]["verb"], json!("describe"));
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.internal"));
    assert_eq!(finding["action"]["kind"], json!("review"));
    let report = &finding["evidence"]["bug_report"];
    assert_eq!(report["version"], json!(env!("CARGO_PKG_VERSION")));
    assert_eq!(report["os"], json!(std::env::consts::OS));
    assert_eq!(report["arch"], json!(std::env::consts::ARCH));
    assert!(
        report["message"]
            .as_str()
            .unwrap()
            .contains("CUPEL_DEBUG_PANIC"),
        "{report}"
    );
    assert!(
        report["location"].as_str().unwrap().contains(".rs:"),
        "{report}"
    );
    assert_eq!(report["args"], json!(["describe"]));
    assert!(report["report"].as_str().unwrap().starts_with("https://"));
    assert!(!outcome.stderr.contains("panicked"), "{}", outcome.stderr);
}
