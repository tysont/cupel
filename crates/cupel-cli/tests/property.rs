// ABOUTME: `gen.property` through the binary on the Python gen.property fixture (ADR-095, ADR-096): the falsified and
// ABOUTME: missing properties, `--cases`, and `--scaffold` previewed without `--write` and written with it.
mod support;

use serde_json::{Value, json};
use support::Repo;

/// A copy of the Python gen.property fixture; `None` (reported as skipped) without pytest or
/// Hypothesis.
fn fixture() -> Option<Repo> {
    let hypothesis = std::process::Command::new("python3")
        .args(["-c", "import hypothesis"])
        .output()
        .is_ok_and(|output| output.status.success());
    if !hypothesis {
        eprintln!("SKIPPED: hypothesis not installed; gen.property was not run");
        return None;
    }
    Repo::python_fixture("gen.property")
}

/// Each finding's kind and subject (its test, else its function), in envelope order.
fn found(envelope: &Value) -> Vec<(String, String)> {
    envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            let evidence = &finding["evidence"];
            let subject = evidence["test"].as_str().or(evidence["function"].as_str());
            (
                finding["kind"].as_str().unwrap().to_owned(),
                subject.unwrap().to_owned(),
            )
        })
        .collect()
}

fn pair(kind: &str, subject: &str) -> (String, String) {
    (kind.to_owned(), subject.to_owned())
}

const FALSIFIED: &str = "tests/test_digits.py::test_digits_counts_decimal_places";

#[test]
fn a_deeper_run_falsifies_the_property_the_everyday_suite_passes() {
    let Some(repo) = fixture() else { return };
    let outcome = repo.run(&["check", "--only", "gen.property", "--seed", "3"]);
    assert_eq!(outcome.code, 0, "no gate: {}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["checks"][0]["status"], json!("fail"));
    assert_eq!(
        found(&envelope),
        [
            pair("property.missing", "units.percent.clamp_percent"),
            pair("property.falsified", FALSIFIED),
        ]
    );
    let falsified = &envelope["findings"][1];
    assert_eq!(falsified["evidence"]["counterexample"], json!("n=100000"));
    assert_eq!(falsified["evidence"]["cases"], json!(1000));
    assert_eq!(
        falsified["repro"],
        json!("cupel check --only gen.property --paths tests/test_digits.py --seed 3 --cases 1000")
    );
}

#[test]
fn a_small_case_budget_is_the_engines_budget() {
    let Some(repo) = fixture() else { return };
    let outcome = repo.run(&[
        "check",
        "--only",
        "gen.property",
        "--seed",
        "3",
        "--cases",
        "5",
    ]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(
        found(&envelope),
        [pair("property.missing", "units.percent.clamp_percent")]
    );
    assert_eq!(envelope["checks"][0]["metrics"]["falsified"], json!(0.0));
}

#[test]
fn a_case_budget_of_zero_is_a_usage_error() {
    let outcome = support::run(["check", "--only", "gen.property", "--cases", "0"]);
    assert_eq!(outcome.code, 2, "{}", outcome.stdout);
}

/// A short property run of the fixture with `--scaffold`.
const SCAFFOLD: [&str; 8] = [
    "check",
    "--only",
    "gen.property",
    "--seed",
    "3",
    "--cases",
    "5",
    "--scaffold",
];

#[test]
fn scaffold_without_write_previews_each_files_diff_and_writes_nothing() {
    let Some(repo) = fixture() else { return };
    // The suite's own runs leave what its tools keep (Hypothesis's `.hypothesis/`) before the
    // preview is compared.
    assert_eq!(repo.run(&SCAFFOLD[..7]).code, 0);
    let before = repo.tree_outside_cupel();
    let preview = repo.run(&SCAFFOLD);
    assert_eq!(preview.code, 0, "{}", preview.stdout);
    assert_eq!(
        repo.tree_outside_cupel(),
        before,
        "a preview writes nothing"
    );
    let envelope = preview.envelope();
    let writes = envelope["plan"]["would_write"].as_array().unwrap();
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(writes[0]["path"], json!("tests/test_percent.py"));
    let diff = writes[0]["diff"].as_str().unwrap();
    for line in [
        "+def test_clamp_percent_property(a0):",
        "+from hypothesis import given, strategies as st",
    ] {
        assert!(diff.contains(line), "{line} in {diff}");
    }
    let commands: Vec<&str> = envelope["next_actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|action| action["command"].as_str())
        .collect();
    assert!(
        commands
            .iter()
            .any(|command| command.contains(" --scaffold ") && command.ends_with(" --write")),
        "{commands:?}"
    );
}

#[test]
fn scaffold_with_write_appends_stubs_the_next_run_counts_as_properties() {
    let Some(repo) = fixture() else { return };
    let mut written = SCAFFOLD.to_vec();
    written.push("--write");
    let write = repo.run(&written);
    assert_eq!(write.code, 0, "{}", write.stdout);
    assert!(write.envelope()["plan"].is_null());
    let text = std::fs::read_to_string(repo.path("tests/test_percent.py")).unwrap();
    assert!(
        text.starts_with("from units.percent import clamp_percent\n"),
        "{text}"
    );
    assert!(text.contains("@given(st.from_type(object))\ndef test_clamp_percent_property(a0):"));

    let after = repo.run(&SCAFFOLD[..7]);
    assert_eq!(after.code, 0, "{}", after.stdout);
    assert_eq!(found(&after.envelope()), Vec::<(String, String)>::new());
    assert_eq!(
        after.envelope()["checks"][0]["metrics"]["properties"],
        json!(2.0)
    );
}
