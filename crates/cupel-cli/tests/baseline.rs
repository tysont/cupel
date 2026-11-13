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

/// `run_id(n)`'s `check` run with status `status` and `git` as its `run.git`.
fn run_on(n: u8, status: &str, git: Value) -> cupel_core::Envelope {
    let run = envelope(&run_id(n), "check", &[], &[complexity_check(0.5)]);
    let mut value = serde_json::to_value(&run).unwrap();
    value["status"] = json!(status);
    value["exit_code"] = json!(i32::from(status == "fail"));
    value["run"]["git"] = git;
    serde_json::from_value(value).unwrap()
}

/// A run's `run.git` in a work tree on `branch`, or on a detached `HEAD` when `None`.
fn on_branch(branch: Option<&str>) -> Value {
    json!({"sha": "9f1c2d3", "dirty": false, "base": null, "branch": branch})
}

/// The baseline's run ID, or null when none is set.
fn baseline_id(repo: &Repo) -> Value {
    repo.run(&["baseline", "show"]).envelope()["run"]["baseline"]["id"].clone()
}

#[test]
fn set_and_import_refuse_a_run_that_did_not_pass() {
    let repo = Repo::new();
    repo.seed(&[
        run_on(1, "pass", Value::Null),
        run_on(2, "fail", Value::Null),
    ]);
    let title = format!(
        "The run {} did not pass (status `fail`); a baseline names only a passing run on the \
         default branch",
        run_id(2)
    );
    for args in [&["baseline", "set"][..], &["baseline", "set", "--write"]] {
        let refused = assert_usage(&repo.run(args));
        assert_eq!(
            (&refused["title"], &refused["evidence"]["argument"]),
            (&json!(title), &json!("--from")),
            "{args:?}"
        );
    }
    let from = format!("run:{}", run_id(1));
    let set = repo.run(&["baseline", "set", "--from", &from, "--write"]);
    assert_eq!(set.code, 0, "{}", set.stdout);

    repo.write(
        "failed.json",
        &serde_json::to_string(&run_on(2, "fail", Value::Null)).unwrap(),
    );
    let refused = assert_usage(&repo.run(&["baseline", "import", "failed.json", "--write"]));
    assert_eq!(
        (&refused["title"], &refused["evidence"]["argument"]),
        (&json!(title), &json!("FILE"))
    );
    assert_eq!(baseline_id(&repo), json!(run_id(1)));
}

#[test]
fn set_refuses_a_run_on_another_branch_or_a_detached_head() {
    let repo = Repo::new();
    repo.write("README", "a\n");
    let made = repo.git(&["init", "-q", "-b", "main"])
        && repo.git(&["add", "README"])
        && repo.git(&["commit", "-q", "-m", "a"]);
    assert!(made, "git could not make the repository");
    repo.seed(&[
        run_on(1, "pass", on_branch(Some("main"))),
        run_on(2, "pass", on_branch(Some("feature"))),
        run_on(3, "pass", on_branch(None)),
    ]);
    let refusal = |n: u8| {
        let from = format!("run:{}", run_id(n));
        assert_usage(&repo.run(&["baseline", "set", "--from", &from, "--write"]))["title"].clone()
    };
    assert_eq!(
        [refusal(2), refusal(3)],
        [
            json!(format!(
                "The run {} is on branch `feature`, not the default branch `main`; a baseline \
                 names only a passing run on the default branch",
                run_id(2)
            )),
            json!(format!(
                "The run {} is on a detached HEAD, not the default branch `main`; a baseline \
                 names only a passing run on the default branch",
                run_id(3)
            )),
        ]
    );
    let from = format!("run:{}", run_id(1));
    let set = repo.run(&["baseline", "set", "--from", &from, "--write"]);
    assert_eq!(set.code, 0, "{}", set.stdout);
    assert_eq!(baseline_id(&repo), json!(run_id(1)));

    // `default_branch` in cupel.toml names the default branch over git's.
    repo.write("cupel.toml", "default_branch = \"feature\"\n");
    let from = format!("run:{}", run_id(2));
    let set = repo.run(&["baseline", "set", "--from", &from, "--write"]);
    assert_eq!(set.code, 0, "{}", set.stdout);
    assert_eq!(baseline_id(&repo), json!(run_id(2)));
}

/// `run_id(n)`'s run whose gate failed on one skipped rule, skipped for `reason`.
fn gated(n: u8, reason: &str) -> cupel_core::Envelope {
    let mut value = serde_json::to_value(run_on(n, "fail", Value::Null)).unwrap();
    value["summary"]["gate"] = json!({
        "result": "fail", "evaluated": "gate.tree",
        "rules": [
            {"rule": "static.complexity.cyclomatic_max <= 10", "actual": 4.0, "bound": 10.0,
             "result": "pass", "reason": null},
            {"rule": "static.clones.pairs <= base", "actual": 1.0, "bound": null,
             "result": "skipped", "reason": reason}
        ]
    });
    serde_json::from_value(value).unwrap()
}

#[test]
fn a_run_whose_gate_failed_only_for_want_of_a_reference_can_be_the_baseline() {
    let repo = Repo::new();
    repo.seed(&[
        gated(1, "no reference: no baseline is set"),
        gated(2, "`static.clones` was skipped (unsupported)"),
    ]);
    let refused = assert_usage(&repo.run(&["baseline", "set", "--write"]));
    assert_eq!(
        refused["title"],
        json!(format!(
            "The run {} did not pass (status `fail`); a baseline names only a passing run on \
             the default branch",
            run_id(2)
        ))
    );
    let from = format!("run:{}", run_id(1));
    let set = repo.run(&["baseline", "set", "--from", &from, "--write"]);
    assert_eq!(set.code, 0, "{}", set.stdout);
    assert_eq!(baseline_id(&repo), json!(run_id(1)));
}
