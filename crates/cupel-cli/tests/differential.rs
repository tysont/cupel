// ABOUTME: `gen.differential` through the binary (ADR-094): a ref oracle's result capped until it is verified,
// ABOUTME: the repro that regenerates a finding, `--cases`, and the skips without entry points or with a spec oracle.
mod support;

use serde_json::{Value, json};
use support::{Outcome, Repo, finding_fields};

const ENTRIES: &str = "[lang.python]\ndifferential_entries = [\"quarter.py:quarter\"]\n";
const RELEASE: &str = "[oracles.release]\nkind = \"ref\"\nref = \"v1\"\n";

/// A git repository whose `quarter.py` rounded toward zero at the commit tagged `v1` and rounds
/// down now, configured with `toml`; `None` (reported as skipped) without `python3`.
fn changed_repo(toml: &str) -> Option<Repo> {
    if cupel_core::find_program("python3").is_none() {
        eprintln!("SKIPPED: python3 not installed; gen.differential was not run");
        return None;
    }
    let repo = Repo::new();
    repo.write("cupel.toml", toml);
    repo.write(
        "quarter.py",
        "def quarter(cents: int) -> int:\n    return int(cents / 4)\n",
    );
    assert!(repo.git(&["init", "-q"]));
    assert!(repo.git(&["add", "-A"]));
    assert!(repo.git(&["commit", "-q", "-m", "v1"]));
    assert!(repo.git(&["tag", "v1"]));
    repo.write(
        "quarter.py",
        "def quarter(cents: int) -> int:\n    return cents // 4\n",
    );
    Some(repo)
}

fn differential(repo: &Repo, extra: &[&str]) -> Outcome {
    let mut args = vec!["check", "--only", "gen.differential", "--format", "json"];
    args.extend(extra);
    repo.run(&args)
}

/// The check's one entry in the envelope.
fn entry(envelope: &Value) -> &Value {
    &envelope["checks"][0]
}

#[test]
fn a_ref_oracle_caps_the_result_until_it_is_verified() {
    let Some(repo) = changed_repo(&format!("{RELEASE}{ENTRIES}")) else {
        return;
    };
    let unverified = differential(&repo, &["--seed", "3"]).envelope();
    assert_eq!(
        (&entry(&unverified)["tier"], &entry(&unverified)["oracle"]),
        (&json!(3), &json!("release"))
    );
    assert_eq!(
        finding_fields(&unverified, "kind"),
        ["diff.divergence", "oracle.unverified"]
    );
    assert_eq!(repo.run(&["oracle", "verify", "release"]).code, 0);
    let verified = differential(&repo, &["--seed", "3"]).envelope();
    assert_eq!(entry(&verified)["tier"], json!(4));
    assert_eq!(finding_fields(&verified, "kind"), ["diff.divergence"]);
    let evidence = &verified["findings"][0]["evidence"];
    assert_eq!(
        (
            &evidence["input"],
            &evidence["implementation"],
            &evidence["reference"]
        ),
        (
            &json!([-1]),
            &json!({"returned": "-1"}),
            &json!({"returned": "0"})
        )
    );
    assert_eq!(
        evidence["oracle"]["commit"].as_str().map(str::len),
        Some(40)
    );
}

#[test]
fn the_repro_regenerates_the_same_finding() {
    let Some(repo) = changed_repo(&format!("{RELEASE}{ENTRIES}")) else {
        return;
    };
    let first = differential(&repo, &["--seed", "11", "--cases", "7"]).envelope();
    let finding = &first["findings"][0];
    let repro = finding["repro"].as_str().unwrap();
    assert!(
        repro.ends_with("--oracle release --seed 11 --cases 7"),
        "{repro}"
    );
    let mut words: Vec<&str> = repro.split_whitespace().skip(1).collect();
    words.extend(["--format", "json"]);
    let again = repo.run(&words).envelope();
    let matching = &again["findings"][0];
    assert_eq!(
        (&matching["id"], &matching["evidence"]),
        (&finding["id"], &finding["evidence"])
    );
}

#[test]
fn without_entry_points_the_language_is_skipped_as_unconfigured() {
    let Some(repo) = changed_repo(RELEASE) else {
        return;
    };
    let envelope = differential(&repo, &[]).envelope();
    assert_eq!(entry(&envelope)["status"], json!("skipped"));
    assert_eq!(entry(&envelope)["skipped"]["reason"], json!("unconfigured"));
    assert_eq!(finding_fields(&envelope, "kind"), Vec::<String>::new());
}

#[test]
fn a_spec_oracle_skips_the_check_and_points_to_its_description() {
    let spec = "[oracles.model]\nkind = \"spec\"\nspec = \"model.qnt\"\nlang = \"quint\"\n";
    let Some(repo) = changed_repo(&format!("{spec}{ENTRIES}")) else {
        return;
    };
    let envelope = differential(&repo, &["--oracle", "model"]).envelope();
    assert_eq!(
        (
            &entry(&envelope)["skipped"]["reason"],
            &entry(&envelope)["skipped"]["see"]
        ),
        (
            &json!("unconfigured"),
            &json!("cupel describe gen.differential")
        )
    );
}

#[test]
fn describe_finding_documents_the_divergence_evidence() {
    let outcome = support::run(["describe", "finding", "diff.divergence"]);
    let description = &outcome.envelope()["description"];
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    let properties = description["evidence"]["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{description:#}"));
    let mut keys: Vec<&str> = properties.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "case",
            "entry",
            "found",
            "implementation",
            "input",
            "oracle",
            "reference",
            "shrinks"
        ]
    );
}
