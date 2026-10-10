// ABOUTME: Integration tests for the renderers through the real binary: JSON lines, text, color, and
// ABOUTME: `--limit`/`--offset` paging with `truncated` and `total_findings`.
mod support;

use serde_json::{Value, json};
use support::{cli_finding_id, run};

/// Parses JSON-lines stdout into its header and finding lines.
fn jsonl(stdout: &str) -> (Value, Vec<Value>) {
    assert!(stdout.ends_with('\n'), "trailing newline");
    let mut lines = stdout
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("each line is JSON"));
    let header = lines.next().expect("a header line");
    (header, lines.collect())
}

#[test]
fn jsonl_writes_the_envelope_header_then_each_finding_on_its_own_line() {
    let outcome = run(["check", "--format", "jsonl"]);
    assert_eq!(outcome.code, 2);
    let (header, findings) = jsonl(&outcome.stdout);
    assert_eq!(header["findings"], json!([]));
    assert_eq!(header["total_findings"], json!(1));
    assert_eq!(header["truncated"], json!(false));
    assert_eq!(header["exit_code"], json!(2));
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0]["id"],
        json!(cli_finding_id("cli.not-implemented", "check"))
    );
}

#[test]
fn jsonl_reassembles_into_the_json_envelope() {
    let (mut header, findings) = jsonl(&run(["check", "--format", "jsonl", "--seed", "1"]).stdout);
    let mut whole = run(["check", "--format", "json", "--seed", "1"]).envelope();
    header["findings"] = Value::Array(findings);
    for envelope in [&mut header, &mut whole] {
        envelope["run"]["id"] = json!("[run id]");
        envelope["run"]["started"] = json!("[started]");
        envelope["run"]["duration_ms"] = json!(0);
        envelope["findings"][0]["repro"] = json!("[repro]");
        envelope["run"]["config"]["effective"]["format"] = json!("[format]");
    }
    assert_eq!(header, whole);
}

#[test]
fn jsonl_of_describe_is_a_single_header_line() {
    let outcome = run(["describe", "--format", "jsonl"]);
    assert_eq!(outcome.code, 0);
    assert_eq!(outcome.stdout.lines().count(), 1);
    let (header, _) = jsonl(&outcome.stdout);
    assert_eq!(header["description"]["subject"], json!("cli"));
}

#[test]
fn text_describe_reads_as_sections() {
    let outcome = run(["describe", "check", "--format", "text"]);
    assert_eq!(outcome.code, 0);
    let out = &outcome.stdout;
    assert!(out.starts_with("cupel describe: pass (exit 0)\n"), "{out}");
    for section in [
        "cupel check · ",
        "  arguments\n",
        "  examples\n",
        "summary ",
        "run ",
    ] {
        assert!(out.contains(section), "{section:?} in:\n{out}");
    }
}

#[test]
fn text_without_a_terminal_or_with_no_color_has_no_escape_codes() {
    for args in [
        &["check", "--format", "text"][..],
        &["check", "--format", "text", "--no-color"][..],
    ] {
        let outcome = run(args);
        assert!(!outcome.stdout.contains('\u{1b}'), "{args:?}");
    }
}

#[test]
fn limit_zero_keeps_no_findings_but_reports_them() {
    let outcome = run(["check", "--limit", "0"]);
    assert_eq!(outcome.code, 2, "paging never changes the exit code");
    let envelope = outcome.envelope();
    assert_eq!(envelope["findings"], json!([]));
    assert_eq!(envelope["total_findings"], json!(1));
    assert_eq!(envelope["truncated"], json!(true));
    assert_eq!(envelope["summary"]["findings"]["fail"], json!(1));
}

#[test]
fn offset_past_the_end_is_an_empty_truncated_page() {
    let envelope = run(["check", "--offset", "5"]).envelope();
    assert_eq!(envelope["findings"], json!([]));
    assert_eq!(envelope["total_findings"], json!(1));
    assert_eq!(envelope["truncated"], json!(true));
}

#[test]
fn a_page_holding_every_finding_is_not_truncated() {
    let envelope = run(["check", "--limit", "1", "--offset", "0"]).envelope();
    assert_eq!(envelope["findings"].as_array().unwrap().len(), 1);
    assert_eq!(envelope["total_findings"], json!(1));
    assert_eq!(envelope["truncated"], json!(false));
}

#[test]
fn paging_applies_to_jsonl_and_text_too() {
    let (header, findings) = jsonl(&run(["check", "--format", "jsonl", "--limit", "0"]).stdout);
    assert!(findings.is_empty());
    assert_eq!(header["truncated"], json!(true));
    let text = run(["check", "--format", "text", "--limit", "0"]).stdout;
    assert!(
        text.contains("showing 0 of 1 findings (truncated)"),
        "{text}"
    );
    assert!(!text.contains("fail (1)"), "{text}");
}
