// ABOUTME: Integration tests for `cupel baseline`: set (previewed without --write), show, diff with
// ABOUTME: --against, export/import round trips, and named baselines (ADR-046).
mod support;

use std::fs;

use serde_json::{Value, json};
use support::{
    Outcome, Repo, complexity_check, complexity_finding, envelope, finding_fields, run_id,
};

/// Asserts `outcome` is a usage error (exit 2, `cli.usage`) and returns its finding.
fn assert_usage(outcome: &Outcome) -> Value {
    assert_eq!(outcome.code, 2, "{}", outcome.stdout);
    let finding = outcome.finding();
    assert_eq!(finding["kind"], json!("cli.usage"));
    finding
}

/// A repository whose store holds `check` runs 1 (findings a, b; score 0.25) and 2 (findings b,
/// c; score 0.5).
fn two_checks() -> Repo {
    let repo = Repo::new();
    repo.seed(&[
        envelope(
            &run_id(1),
            "check",
            &[
                complexity_finding("fnd_00000a", 1, 1),
                complexity_finding("fnd_00000b", 2, 2),
            ],
            &[complexity_check(0.25)],
        ),
        envelope(
            &run_id(2),
            "check",
            &[
                complexity_finding("fnd_00000b", 2, 2),
                complexity_finding("fnd_00000c", 3, 3),
            ],
            &[complexity_check(0.5)],
        ),
    ]);
    repo
}

#[test]
fn show_without_a_baseline_says_how_to_set_one() {
    let finding = assert_usage(&Repo::new().run(&["baseline", "show"]));
    assert_eq!(
        finding["action"]["summary"],
        json!("Set one with `cupel baseline set --write`.")
    );
}

#[test]
fn set_without_write_previews_the_change_and_sets_nothing() {
    let repo = two_checks();
    let outcome = repo.run(&["baseline", "set"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(envelope["status"], json!("plan"));
    let writes = envelope["plan"]["would_write"].as_array().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0]["path"], json!(".cupel/cupel.db"));
    assert_eq!(
        writes[0]["diff"],
        json!(format!(
            "--- /dev/null\n+++ b/baselines/default\n@@ -0,0 +1 @@\n+{}\n",
            run_id(2)
        ))
    );
    assert_eq!(
        envelope["next_actions"][0]["command"],
        json!("cupel baseline set --write")
    );
    let id = envelope["run"]["id"].as_str().unwrap();
    assert!(!repo.path(&format!(".cupel/runs/{id}.json")).exists());
    assert_usage(&repo.run(&["baseline", "show"]));
}

#[test]
fn set_write_names_the_latest_result_run_and_show_prints_it() {
    let repo = two_checks();
    repo.seed(&[envelope(&run_id(3), "explain", &[], &[])]);
    let set = repo.run(&["baseline", "set", "--write"]);
    assert_eq!(set.code, 0, "{}", set.stdout);
    assert_eq!(
        set.envelope()["run"]["baseline"],
        json!({"id": run_id(2), "name": "default"})
    );

    let show = repo.run(&["baseline", "show"]);
    assert_eq!(show.code, 0, "{}", show.stdout);
    let envelope = show.envelope();
    assert_eq!(
        envelope["run"]["baseline"],
        json!({"id": run_id(2), "name": "default"})
    );
    assert_eq!(
        finding_fields(&envelope, "id"),
        ["fnd_00000b", "fnd_00000c"]
    );
    assert_eq!(envelope["checks"], json!([complexity_check(0.5)]));
}

#[test]
fn set_from_names_a_given_run_and_rejects_an_unknown_one() {
    let repo = two_checks();
    let from = format!("run:{}", run_id(1));
    let set = repo.run(&["baseline", "set", "--from", &from, "--write"]);
    assert_eq!(set.code, 0, "{}", set.stdout);
    assert_eq!(
        repo.run(&["baseline", "show"]).envelope()["run"]["baseline"]["id"],
        json!(run_id(1))
    );
    for bad in [format!("run:{}", run_id(9)), "yesterday".to_owned()] {
        let finding = assert_usage(&repo.run(&["baseline", "set", "--from", &bad, "--write"]));
        assert_eq!(finding["evidence"]["argument"], json!("--from"), "{bad}");
    }
}

#[test]
fn set_with_no_result_run_is_a_usage_error() {
    let repo = Repo::new();
    repo.seed(&[envelope(&run_id(1), "explain", &[], &[])]);
    let finding = assert_usage(&repo.run(&["baseline", "set", "--write"]));
    assert_eq!(
        finding["title"],
        json!("No stored run can be the baseline yet")
    );
}

#[test]
fn named_baselines_are_independent() {
    let repo = two_checks();
    let from = format!("run:{}", run_id(1));
    repo.run(&["baseline", "set", "--write"]);
    repo.run(&[
        "baseline", "--name", "release", "set", "--from", &from, "--write",
    ]);
    let show = |name: &str| {
        repo.run(&["baseline", "--name", name, "show"]).envelope()["run"]["baseline"].clone()
    };
    assert_eq!(show("default"), json!({"id": run_id(2), "name": "default"}));
    assert_eq!(show("release"), json!({"id": run_id(1), "name": "release"}));
}

#[test]
fn diff_lists_new_then_resolved_findings_with_counts_and_score_deltas() {
    let repo = two_checks();
    let from = format!("run:{}", run_id(1));
    repo.run(&["baseline", "set", "--from", &from, "--write"]);

    let outcome = repo.run(&["baseline", "diff"]);

    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let envelope = outcome.envelope();
    assert_eq!(
        finding_fields(&envelope, "id"),
        ["fnd_00000c", "fnd_00000a"]
    );
    let marks: Vec<bool> = envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["new_since_baseline"].as_bool().unwrap())
        .collect();
    assert_eq!(marks, [true, false]);
    assert_eq!(envelope["summary"]["new"], json!(1));
    assert_eq!(envelope["summary"]["resolved"], json!(1));
    assert_eq!(
        envelope["run"]["baseline"],
        json!({"id": run_id(1), "name": "default"})
    );
    assert_eq!(
        envelope["checks"][0]["delta"],
        json!({"vs": "baseline", "value": 0.25})
    );
}

#[test]
fn diff_against_a_run_compares_with_that_run() {
    let repo = two_checks();
    let against = format!("run:{}", run_id(1));
    let envelope = repo
        .run(&["baseline", "diff", "--against", &against])
        .envelope();
    assert_eq!(
        finding_fields(&envelope, "id"),
        ["fnd_00000c", "fnd_00000a"]
    );
    assert_eq!(
        envelope["run"]["baseline"],
        json!({"id": run_id(1), "name": against})
    );
    assert_eq!(envelope["checks"][0]["delta"]["vs"], json!(against));
}

#[test]
fn diff_against_something_unknown_is_a_usage_error_about_against() {
    let repo = two_checks();
    for against in [
        "baseline".to_owned(),
        format!("run:{}", run_id(9)),
        "run:nonsense".to_owned(),
        "no-such-ref".to_owned(),
    ] {
        let finding = assert_usage(&repo.run(&["baseline", "diff", "--against", &against]));
        assert_eq!(
            finding["evidence"]["argument"],
            json!("--against"),
            "{against}"
        );
    }
}

#[test]
fn export_and_import_round_trip_a_baseline() {
    let repo = two_checks();
    repo.run(&["baseline", "set", "--write"]);
    let export = repo.run(&["baseline", "export", ".cupel/baseline.json"]);
    assert_eq!(export.code, 0, "{}", export.stdout);
    let exported = fs::read_to_string(repo.path(".cupel/baseline.json")).unwrap();
    assert!(exported.ends_with("}\n"));
    let stored = repo
        .store()
        .run(&run_id(2).parse().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<cupel_core::Envelope>(&exported).unwrap(),
        stored
    );

    let other = Repo::new();
    other.write("baseline.json", &exported);
    let preview = other.run(&["baseline", "import", "baseline.json"]);
    assert_eq!(preview.envelope()["status"], json!("plan"));
    assert_usage(&other.run(&["baseline", "show"]));
    let import = other.run(&["baseline", "import", "baseline.json", "--write"]);
    assert_eq!(import.code, 0, "{}", import.stdout);
    let show = other.run(&["baseline", "show"]).envelope();
    assert_eq!(show["run"]["baseline"]["id"], json!(run_id(2)));
    assert_eq!(finding_fields(&show, "id"), ["fnd_00000b", "fnd_00000c"]);
}

#[test]
fn export_outside_cupel_needs_write_and_a_dry_run_shows_the_diff() {
    let repo = two_checks();
    repo.run(&["baseline", "set", "--write"]);
    let refused = assert_usage(&repo.run(&["baseline", "export", "baseline.json"]));
    assert_eq!(refused["evidence"]["argument"], json!("FILE"));
    assert!(!repo.path("baseline.json").exists());

    let dry = repo.run(&[
        "baseline",
        "export",
        "baseline.json",
        "--write",
        "--dry-run",
    ]);
    assert_eq!(dry.envelope()["status"], json!("plan"));
    assert_eq!(
        dry.envelope()["plan"]["would_write"][0]["path"],
        json!("baseline.json")
    );
    assert!(!repo.path("baseline.json").exists());

    let written = repo.run(&["baseline", "export", "baseline.json", "--write"]);
    assert_eq!(written.code, 0, "{}", written.stdout);
    assert!(repo.path("baseline.json").exists());
}

#[test]
fn import_of_a_file_that_is_not_an_envelope_is_a_usage_error() {
    let repo = Repo::new();
    repo.write("baseline.json", "{\"not\": \"an envelope\"}\n");
    let finding = assert_usage(&repo.run(&["baseline", "import", "baseline.json", "--write"]));
    assert_eq!(finding["evidence"]["argument"], json!("FILE"));
    let missing = assert_usage(&repo.run(&["baseline", "import", "missing.json", "--write"]));
    assert_eq!(missing["evidence"]["argument"], json!("FILE"));
}

#[test]
fn baseline_runs_never_become_the_default_baseline() {
    let repo = two_checks();
    repo.run(&["baseline", "show"]);
    repo.run(&["baseline", "diff"]);
    let set = repo.run(&["baseline", "set", "--write"]);
    assert_eq!(set.envelope()["run"]["baseline"]["id"], json!(run_id(2)));
}
