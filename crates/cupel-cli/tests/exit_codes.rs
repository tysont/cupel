// ABOUTME: Integration tests proving each exit code reachable so far via a deliberate trigger,
// ABOUTME: and that every error path still writes one valid envelope to stdout.
mod support;

use serde_json::{Value, json};
use support::{cli_finding_id, cupel, run, run_with};

#[test]
fn exit_0_pass_for_a_successful_verb() {
    let outcome = run(["describe"]);
    assert_eq!(outcome.code, 0);
    let envelope = outcome.envelope();
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
    let fix = finding["action"]["summary"].as_str().unwrap();
    assert!(fix.contains("--format"), "{fix}");
}

#[test]
fn exit_2_for_an_unknown_verb_has_no_verb_and_lists_the_verbs() {
    let outcome = run(["frobnicate"]);
    assert_eq!(outcome.code, 2);
    let envelope = outcome.envelope();
    assert_eq!(envelope["run"]["verb"], Value::Null);
    let finding = outcome.finding();
    assert_eq!(finding["evidence"]["value"], json!("frobnicate"));
    let fix = finding["action"]["summary"].as_str().unwrap();
    assert!(fix.contains("init, doctor, describe, check"), "{fix}");
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
    let fix = finding["action"]["summary"].as_str().unwrap();
    assert!(
        fix.contains("--quiet") && fix.contains("--verbose"),
        "{fix}"
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

#[test]
fn exit_2_for_every_unimplemented_verb_names_its_plan_step() {
    let cli = run(["describe"]).envelope()["description"].clone();
    let all = run(["describe", "--all"]).envelope()["description"].clone();
    let mut pending = 0;
    for (summary, command) in cli["verbs"]
        .as_array()
        .unwrap()
        .iter()
        .zip(all["commands"].as_array().unwrap())
    {
        let Some(step) = summary["pending_step"].as_str() else {
            continue;
        };
        pending += 1;
        let name = command["name"].as_str().unwrap();
        let example = command["examples"][0]["command"].as_str().unwrap();
        let args: Vec<&str> = example.split_whitespace().skip(1).collect();
        let outcome = run(&args);
        assert_eq!(outcome.code, 2, "{example}");
        let finding = outcome.finding();
        assert_eq!(finding["kind"], json!("cli.not-implemented"), "{example}");
        assert_eq!(outcome.envelope()["run"]["verb"], json!(name));
        let detail = finding["detail"].as_str().unwrap();
        assert!(detail.contains(step), "{example}: {detail}");
    }
    assert_eq!(
        pending, 4,
        "every verb but describe, init, doctor, check, explain, baseline, and suppress is pending"
    );
}

#[test]
fn every_registered_example_exits_with_its_declared_code() {
    fn visit(command: &Value, checked: &mut usize) {
        for example in command["examples"].as_array().unwrap() {
            let line = example["command"].as_str().unwrap();
            let args: Vec<&str> = line.split_whitespace().skip(1).collect();
            assert_eq!(
                Some(i64::from(run(&args).code)),
                example["exit_code"].as_i64(),
                "{line}"
            );
            *checked += 1;
        }
        for sub in command["subcommands"].as_array().unwrap() {
            visit(sub, checked);
        }
    }
    let all = run(["describe", "--all"]).envelope()["description"].clone();
    let mut checked = 0;
    for command in all["commands"].as_array().unwrap() {
        visit(command, &mut checked);
    }
    assert!(checked >= 11, "{checked} examples");
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
