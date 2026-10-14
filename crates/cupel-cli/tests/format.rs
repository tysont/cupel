// ABOUTME: Integration tests for output-format selection: piped stdout gets JSON, `--format` and
// ABOUTME: `CUPEL_FORMAT` override it, and invalid or unbuilt formats produce exit-2 envelopes.
mod support;

use serde_json::{Value, json};
use support::{cli_finding_id, cupel, run, run_with};

#[test]
fn piped_stdout_gets_the_complete_json_envelope() {
    let outcome = run(["describe"]);
    assert_eq!(outcome.code, 0);
    assert!(outcome.stdout.ends_with("}\n"), "trailing newline");
    let envelope = outcome.envelope();
    for key in [
        "cupel",
        "run",
        "status",
        "exit_code",
        "checks",
        "findings",
        "summary",
        "next_actions",
        "description",
        "truncated",
        "total_findings",
    ] {
        assert!(envelope.get(key).is_some(), "{key}");
    }
}

#[test]
fn format_text_renders_a_status_line_instead_of_json() {
    let outcome = run(["describe", "--format", "text"]);
    assert_eq!(outcome.code, 0);
    assert!(serde_json::from_str::<Value>(&outcome.stdout).is_err());
    assert!(
        outcome
            .stdout
            .starts_with("cupel describe: pass (exit 0)\n"),
        "{}",
        outcome.stdout
    );
    insta::assert_snapshot!(support::redacted_text(&outcome.stdout));
}

#[test]
fn format_text_shows_findings_with_their_fix() {
    let outcome = run(["survey", "--format", "text"]);
    assert_eq!(outcome.code, 2);
    let id = cli_finding_id("cli.not-implemented", "survey");
    assert!(outcome.stdout.starts_with("cupel survey: error (exit 2)\n"));
    let findings = format!(
        "fail (1)\n  cli\n    `cupel survey` is not implemented yet\n      \
         {id} · cli.not-implemented · tier 0 · confidence 1\n      \
         Implemented in plan step P2.9.\n      \
         evidence: step \"P2.9\" (full: cupel explain {id})\n      \
         action review (priority 1): Use a verb that is implemented; `cupel describe` marks \
         pending ones with pending_step.\n      \
         repro: cupel survey --format text\n      \
         suppress: cupel suppress {id} --reason '…'\n"
    );
    assert!(
        outcome.stdout.contains(&findings),
        "{findings}\nin:\n{}",
        outcome.stdout
    );
}

#[test]
fn cupel_format_env_selects_the_format() {
    let outcome = run_with(cupel().env("CUPEL_FORMAT", "text"), ["describe"]);
    assert!(outcome.stdout.starts_with("cupel describe: pass"));
}

#[test]
fn format_flag_overrides_cupel_format_env() {
    let outcome = run_with(
        cupel().env("CUPEL_FORMAT", "text"),
        ["describe", "--format", "json"],
    );
    assert_eq!(outcome.envelope()["status"], json!("pass"));
}

#[test]
fn format_flag_may_come_before_or_after_the_verb() {
    for args in [["--format=text", "describe"], ["describe", "--format=text"]] {
        let outcome = run(args);
        assert!(
            outcome.stdout.starts_with("cupel describe: pass"),
            "{args:?}"
        );
    }
}

#[test]
fn invalid_format_is_a_usage_error_naming_the_valid_values() {
    let outcome = run(["describe", "--format", "xml"]);
    assert_eq!(outcome.code, 2);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("error"));
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    assert_eq!(
        finding["id"],
        json!(cli_finding_id("cli.usage", "--format"))
    );
    assert_eq!(finding["evidence"]["argument"], json!("--format"));
    assert_eq!(finding["evidence"]["value"], json!("xml"));
    assert_eq!(
        finding["evidence"]["valid"],
        json!(["json", "jsonl", "text", "sarif"])
    );
    assert_eq!(finding["action"]["kind"], json!("configure"));
    let fix = finding["action"]["summary"].as_str().unwrap();
    assert!(fix.contains("json, jsonl, text, sarif"), "{fix}");
    assert!(outcome.stderr.contains("xml"), "diagnostic on stderr");
}

#[test]
fn invalid_cupel_format_env_is_a_usage_error_naming_the_variable() {
    let outcome = run_with(cupel().env("CUPEL_FORMAT", "xml"), ["describe"]);
    assert_eq!(outcome.code, 2);
    let finding = outcome.finding();
    assert_eq!(finding["evidence"]["argument"], json!("CUPEL_FORMAT"));
    assert_eq!(finding["evidence"]["value"], json!("xml"));
}

#[test]
fn usage_errors_render_in_the_requested_format_when_it_is_valid() {
    let outcome = run(["describe", "--format", "text", "--bogus"]);
    assert_eq!(outcome.code, 2);
    assert!(
        outcome
            .stdout
            .starts_with("cupel describe: error (exit 2)\n"),
        "{}",
        outcome.stdout
    );
}

#[test]
fn sarif_is_not_implemented_and_renders_as_json() {
    let outcome = run(["describe", "--format", "sarif"]);
    assert_eq!(outcome.code, 2);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.not-implemented"));
    assert_eq!(
        finding["id"],
        json!(cli_finding_id("cli.not-implemented", "--format sarif"))
    );
    assert!(finding["detail"].as_str().unwrap().contains("P6.1"));
}

#[test]
fn quiet_silences_stderr_diagnostics_without_changing_stdout() {
    let loud = run(["describe", "--bogus"]);
    let quiet = run(["describe", "--bogus", "--quiet"]);
    assert!(!loud.stderr.is_empty());
    assert_eq!(quiet.stderr, "");
    assert_eq!(quiet.finding()["kind"], loud.finding()["kind"]);
}

#[test]
fn verbose_reports_the_repository_root_and_format_on_stderr() {
    let dir = std::env::temp_dir().canonicalize().unwrap();
    let outcome = run(["describe", "--verbose", "--cwd", dir.to_str().unwrap()]);
    assert_eq!(outcome.code, 0);
    assert!(
        outcome.stderr.contains(&format!("root: {}", dir.display())),
        "{}",
        outcome.stderr
    );
    assert!(
        outcome.stderr.contains("format: json"),
        "{}",
        outcome.stderr
    );
    outcome.envelope();
}
