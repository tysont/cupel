// ABOUTME: Integration tests for `cupel oracle` through the binary (ADR-090): add and remove edit cupel.toml
// ABOUTME: (previewed without --write), list reports state, verify runs each kind's smoke comparison.
mod support;

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use support::{Outcome, Repo, cupel, usage_argument};

const CALC: &str =
    "[oracles.calc]\nkind = \"command\"\ncmd = \"echo {input}\"\ninputs = [\"1\", \"2\"]\n";

/// A repository whose `cupel.toml` is `toml`.
fn configured(toml: &str) -> Repo {
    let repo = Repo::new();
    repo.write("cupel.toml", toml);
    repo
}

/// A git repository with one commit, tagged `v1`, whose `cupel.toml` is `toml`.
fn committed(toml: &str) -> Repo {
    let repo = configured(toml);
    repo.write("calc.py", "def add(a, b):\n    return a + b\n");
    assert!(repo.git(&["init", "-q"]));
    assert!(repo.git(&["add", "-A"]));
    assert!(repo.git(&["commit", "-q", "-m", "first"]));
    assert!(repo.git(&["tag", "v1"]));
    repo
}

/// `(name, state)` of each oracle the envelope reports.
fn states(envelope: &Value) -> Vec<(String, String)> {
    envelope["oracles"]
        .as_array()
        .expect("the oracle verb reports oracles")
        .iter()
        .map(|oracle| {
            let field = |key: &str| oracle[key].as_str().unwrap_or_default().to_owned();
            (field("name"), field("state"))
        })
        .collect()
}

fn named(name: &str, state: &str) -> (String, String) {
    (name.to_owned(), state.to_owned())
}

/// The kinds of the envelope's findings with their severity and action.
fn problems(outcome: &Outcome) -> Vec<(String, String, String)> {
    outcome.envelope()["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            let text = |value: &Value| value.as_str().unwrap_or_default().to_owned();
            (
                text(&finding["kind"]),
                text(&finding["severity"]),
                text(&finding["action"]["kind"]),
            )
        })
        .collect()
}

fn config_text(repo: &Repo) -> String {
    fs::read_to_string(repo.path("cupel.toml")).unwrap()
}

#[test]
fn add_previews_the_cupel_toml_change_without_write_and_makes_it_with_write() {
    let repo = configured("# ours\ndepth = \"quick\"\n");
    let preview = repo.run(&["oracle", "add", "old", "--kind", "ref", "--ref", "v1"]);
    let envelope = preview.envelope();
    assert_eq!((preview.code, &envelope["status"]), (0, &json!("plan")));
    let planned = &envelope["plan"]["would_write"][0];
    assert_eq!(planned["path"], json!("cupel.toml"));
    let diff = planned["diff"].as_str().unwrap();
    assert!(
        diff.contains("+[oracles.old]\n+kind = \"ref\"\n+ref = \"v1\"\n"),
        "{diff}"
    );
    assert_eq!(config_text(&repo), "# ours\ndepth = \"quick\"\n");
    assert_eq!(
        envelope["next_actions"][0]["command"],
        json!("cupel oracle add old --kind ref --ref v1 --write")
    );

    let written = repo.run(&[
        "oracle", "add", "old", "--kind", "ref", "--ref", "v1", "--write",
    ]);
    assert_eq!(written.code, 0, "{}", written.stdout);
    assert_eq!(
        config_text(&repo),
        "# ours\ndepth = \"quick\"\n\n[oracles.old]\nkind = \"ref\"\nref = \"v1\"\n"
    );
    assert_eq!(states(&written.envelope()), [named("old", "unreachable")]);
}

#[test]
fn add_writes_a_command_with_its_inputs_and_a_spec_with_its_language() {
    let repo = Repo::new();
    let calc = [
        "oracle",
        "add",
        "calc",
        "--kind",
        "command",
        "--cmd",
        "echo {input}",
    ];
    let inputs = ["--input", "1", "--input", "2", "--write"];
    assert_eq!(repo.run(&[&calc[..], &inputs[..]].concat()).code, 0);
    let model = [
        "oracle",
        "add",
        "model",
        "--kind",
        "spec",
        "--spec",
        "model.qnt",
    ];
    assert_eq!(
        repo.run(&[&model[..], &["--lang", "quint", "--write"]].concat())
            .code,
        0
    );
    assert_eq!(
        config_text(&repo),
        format!(
            "{CALC}\n[oracles.model]\nkind = \"spec\"\nspec = \"model.qnt\"\nlang = \"quint\"\n"
        )
    );
}

#[test]
fn add_refuses_a_taken_name_a_malformed_name_and_flags_the_kind_does_not_take() {
    let repo = configured(CALC);
    let cases: [(&[&str], &str); 4] = [
        (&["calc", "--kind", "ref", "--ref", "v1"], "NAME"),
        (&["Calc", "--kind", "ref", "--ref", "v1"], "NAME"),
        (&["old", "--kind", "ref", "--cmd", "echo"], "--kind"),
        (
            &["model", "--kind", "spec", "--spec", "m.v", "--lang", "coq"],
            "--kind",
        ),
    ];
    for (args, argument) in cases {
        let outcome = repo.run(&[&["oracle", "add"][..], args, &["--write"]].concat());
        assert_eq!(usage_argument(&outcome), json!(argument), "{args:?}");
    }
    assert_eq!(config_text(&repo), CALC);
}

#[test]
fn list_reports_every_oracle_by_name_with_its_identity_and_state() {
    let repo = committed(&format!(
        "{CALC}[oracles.old]\nkind = \"ref\"\nref = \"v1\"\n"
    ));
    let outcome = repo.run(&["oracle", "list"]);
    let envelope = outcome.envelope();
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(
        states(&envelope),
        [named("calc", "unverified"), named("old", "unverified")]
    );
    let hash = envelope["oracles"][1]["hash"].as_str().unwrap();
    assert_eq!(hash.len(), 64);
    assert_eq!(envelope["oracles"][1]["verified_at"], json!(null));
    assert_eq!(envelope["oracles"][1]["definition"]["ref"], json!("v1"));
}

#[test]
fn verify_records_a_command_that_answers_each_input_the_same_way_twice() {
    let repo = configured(CALC);
    let outcome = repo.run(&["oracle", "verify", "calc"]);
    let envelope = outcome.envelope();
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(states(&envelope), [named("calc", "verified")]);
    assert_eq!(
        envelope["oracles"][0]["verified_run"],
        envelope["run"]["id"]
    );
    assert_eq!(
        envelope["oracles"][0]["verified_at"],
        envelope["run"]["started"]
    );
    let stored = repo.store().verified_oracle("calc").unwrap().unwrap();
    assert_eq!(json!(stored.hash), envelope["oracles"][0]["hash"]);
    assert_eq!(
        states(&repo.run(&["oracle", "list"]).envelope()),
        [named("calc", "verified")]
    );
}

#[test]
fn an_edited_definition_is_changed_and_a_failed_verify_revokes_the_verification() {
    let repo = configured(CALC);
    assert_eq!(repo.run(&["oracle", "verify", "calc"]).code, 0);
    repo.write("cupel.toml", &CALC.replace("echo {input}", "echo $$"));
    assert_eq!(
        states(&repo.run(&["oracle", "list"]).envelope()),
        [named("calc", "changed")]
    );

    let outcome = repo.run(&["oracle", "verify", "calc"]);
    assert_eq!(outcome.code, 1, "{}", outcome.stdout);
    assert_eq!(
        problems(&outcome),
        [(
            "oracle.smoke-failed".to_owned(),
            "fail".to_owned(),
            "verify_oracle".to_owned()
        )]
    );
    let evidence = &outcome.envelope()["findings"][0]["evidence"];
    assert_eq!(evidence["input"], json!("1"));
    assert_ne!(evidence["outputs"][0], evidence["outputs"][1]);
    assert_eq!(repo.store().verified_oracle("calc").unwrap(), None);
    assert_eq!(states(&outcome.envelope()), [named("calc", "unverified")]);
}

#[test]
fn a_command_whose_program_is_missing_or_that_fails_is_not_verified() {
    let missing = configured(&CALC.replace("echo", "cupel-test-no-such-program"));
    let outcome = missing.run(&["oracle", "verify", "calc"]);
    assert_eq!(outcome.code, 3, "{}", outcome.stdout);
    assert_eq!(
        problems(&outcome),
        [(
            "oracle.unreachable".to_owned(),
            "fail".to_owned(),
            "install_tool".to_owned()
        )]
    );
    let failing = configured(&CALC.replace("echo {input}", "exit 4"));
    let outcome = failing.run(&["oracle", "verify", "calc"]);
    assert_eq!(outcome.code, 1, "{}", outcome.stdout);
    let evidence = &outcome.envelope()["findings"][0]["evidence"];
    assert_eq!(evidence["exit_code"], json!(4));
}

#[test]
fn a_ref_oracle_is_verified_until_its_ref_moves_and_a_missing_ref_is_exit_3() {
    let repo = committed("[oracles.rel]\nkind = \"ref\"\nref = \"release\"\n");
    assert!(repo.git(&["branch", "release"]));
    assert_eq!(repo.run(&["oracle", "verify", "rel"]).code, 0);
    assert_eq!(
        states(&repo.run(&["oracle", "list"]).envelope()),
        [named("rel", "verified")]
    );
    repo.write("calc.py", "def add(a, b):\n    return b + a\n");
    assert!(repo.git(&["commit", "-q", "-am", "second"]));
    assert!(repo.git(&["branch", "-f", "release"]));
    assert_eq!(
        states(&repo.run(&["oracle", "list"]).envelope()),
        [named("rel", "changed")]
    );

    assert!(repo.git(&["branch", "-D", "release"]));
    let outcome = repo.run(&["oracle", "verify", "rel"]);
    assert_eq!(outcome.code, 3, "{}", outcome.stdout);
    assert_eq!(
        problems(&outcome),
        [(
            "oracle.unreachable".to_owned(),
            "fail".to_owned(),
            "configure".to_owned()
        )]
    );
    assert_eq!(states(&outcome.envelope()), [named("rel", "unreachable")]);
}

/// A directory holding an executable `quint` that accepts `parse <file>` when the file's first
/// line is `module`, and rejects it otherwise.
fn fake_quint() -> tempfile::TempDir {
    let bin = tempfile::tempdir().unwrap();
    let script = bin.path().join("quint");
    fs::write(
        &script,
        "#!/bin/sh\n[ \"$1\" = parse ] && head -n 1 \"$2\" | grep -q '^module' && exit 0\n\
         echo \"error: cannot parse $2\"\nexit 1\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    fs::set_permissions(&script, permissions).unwrap();
    bin
}

/// `cupel <args> --cwd <repo>` with `PATH` set to `bin` (when given) and the system directories.
fn with_path(repo: &Repo, bin: Option<&Path>, args: &[&str]) -> Outcome {
    let system = "/usr/bin:/bin";
    let path = bin.map_or_else(
        || system.to_owned(),
        |bin| format!("{}:{system}", bin.display()),
    );
    repo.run_with(cupel().env("PATH", path), args)
}

#[test]
fn a_spec_oracle_is_verified_by_its_languages_parser_and_changes_with_its_file() {
    let repo =
        configured("[oracles.model]\nkind = \"spec\"\nspec = \"model.qnt\"\nlang = \"quint\"\n");
    repo.write("model.qnt", "module counter {}\n");
    let bin = fake_quint();
    let verify = ["oracle", "verify", "model"];
    assert_eq!(with_path(&repo, Some(bin.path()), &verify).code, 0);
    assert_eq!(
        states(&repo.run(&["oracle", "list"]).envelope()),
        [named("model", "verified")]
    );
    repo.write("model.qnt", "garbage\n");
    assert_eq!(
        states(&repo.run(&["oracle", "list"]).envelope()),
        [named("model", "changed")]
    );

    let rejected = with_path(&repo, Some(bin.path()), &verify);
    assert_eq!(rejected.code, 1, "{}", rejected.stdout);
    let without_parser = with_path(&repo, None, &verify);
    assert_eq!(without_parser.code, 3, "{}", without_parser.stdout);
    assert_eq!(
        problems(&without_parser),
        [(
            "oracle.unreachable".to_owned(),
            "fail".to_owned(),
            "install_tool".to_owned()
        )]
    );
    fs::remove_file(repo.path("model.qnt")).unwrap();
    assert_eq!(
        states(&repo.run(&["oracle", "list"]).envelope()),
        [named("model", "unreachable")]
    );
}

#[test]
fn verify_all_verifies_each_oracle_and_exit_3_outranks_exit_1() {
    let toml = format!(
        "{CALC}[oracles.flaky]\nkind = \"command\"\ncmd = \"echo $$\"\n\
         [oracles.gone]\nkind = \"ref\"\nref = \"no-such-ref\"\n"
    );
    let repo = committed(&toml);
    let outcome = repo.run(&["oracle", "verify", "--all"]);
    assert_eq!(outcome.code, 3, "{}", outcome.stdout);
    assert_eq!(
        states(&outcome.envelope()),
        [
            named("calc", "verified"),
            named("flaky", "unverified"),
            named("gone", "unreachable")
        ]
    );
    let none = Repo::new().run(&["oracle", "verify", "--all"]);
    assert_eq!(
        (none.code, none.envelope()["oracles"].clone()),
        (0, json!([]))
    );
}

#[test]
fn a_dry_run_of_verify_runs_nothing_and_records_nothing() {
    let repo = configured("[oracles.side]\nkind = \"command\"\ncmd = \"touch ran\"\n");
    let outcome = repo.run(&["oracle", "verify", "side", "--dry-run"]);
    assert_eq!(
        (outcome.code, outcome.envelope()["status"].clone()),
        (0, json!("plan"))
    );
    assert!(!repo.path("ran").exists());
    assert_eq!(repo.store().verified_oracles().unwrap(), []);
}

#[test]
fn remove_previews_then_deletes_the_table_and_the_verification() {
    let repo = configured(&format!(
        "# ours\ndepth = \"quick\"\n\n# the calculator\n{CALC}"
    ));
    assert_eq!(repo.run(&["oracle", "verify", "calc"]).code, 0);
    let preview = repo.run(&["oracle", "remove", "calc"]);
    assert_eq!(preview.envelope()["status"], json!("plan"));
    assert!(config_text(&repo).contains("[oracles.calc]"));
    assert!(repo.store().verified_oracle("calc").unwrap().is_some());

    let removed = repo.run(&["oracle", "remove", "calc", "--write"]);
    assert_eq!(removed.code, 0, "{}", removed.stdout);
    assert_eq!(config_text(&repo), "# ours\ndepth = \"quick\"\n");
    assert_eq!(states(&removed.envelope()), [named("calc", "verified")]);
    assert_eq!(repo.store().verified_oracle("calc").unwrap(), None);
}

#[test]
fn an_oracle_that_is_not_defined_is_a_usage_error() {
    let repo = configured(CALC);
    for verb in ["verify", "remove"] {
        let outcome = repo.run(&["oracle", verb, "nope", "--write"]);
        assert_eq!(usage_argument(&outcome), json!("NAME"), "{verb}");
        assert_eq!(
            outcome.finding()["evidence"]["valid"],
            json!(["calc"]),
            "{verb}"
        );
    }
}

#[test]
fn describe_knows_the_finding_a_result_against_an_unverified_oracle_carries() {
    let outcome = Repo::new().run(&["describe", "finding", "oracle.unverified"]);
    let description = &outcome.envelope()["description"];
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert_eq!(
        (&description["check"], &description["action"]),
        (&json!("oracle"), &json!("verify_oracle"))
    );
    let required = &description["evidence"]["required"];
    assert_eq!(required, &json!(["oracle", "state", "checks"]));
}

#[test]
fn add_to_oracles_written_as_an_inline_table_asks_for_tables() {
    let repo = configured("oracles = {}\n");
    let add = [
        "oracle", "add", "old", "--kind", "ref", "--ref", "v1", "--write",
    ];
    assert_eq!(usage_argument(&repo.run(&add)), json!("--config"));
    assert_eq!(config_text(&repo), "oracles = {}\n");
}

#[test]
fn each_oracle_sub_verb_describes_itself() {
    let described = |sub: &str| {
        let outcome = Repo::new().run(&["oracle", sub, "--describe"]);
        outcome.envelope()["description"]["name"].clone()
    };
    let names: Vec<Value> = ["add", "list", "remove", "verify"]
        .into_iter()
        .map(described)
        .collect();
    let expected = [
        "oracle add",
        "oracle list",
        "oracle remove",
        "oracle verify",
    ];
    assert_eq!(names, expected.map(|name| json!(name)));
}
