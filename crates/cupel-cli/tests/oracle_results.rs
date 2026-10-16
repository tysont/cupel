// ABOUTME: Results against an oracle through the CLI's entry point (ADR-091): a tier-4 check that uses one,
// ABOUTME: submitted from this test binary, is capped at tier 3 with `oracle.unverified` until `oracle verify`.
use std::collections::BTreeMap;
use std::path::Path;

use cupel_core::{
    Check, CheckCtx, CheckId, CheckResult, CheckStatus, CostClass, EventSink, Facts, Manifest,
    Plan, Result, Scope, Stability, Tier,
};
use serde_json::{Value, json};

const ID: &str = "test.against-oracle";

/// A tier-4 check that compares against the run's oracle and finds nothing.
struct AgainstOracle(Manifest);

inventory::submit!(cupel_checks::NativeCheck(|| Box::new(AgainstOracle(Manifest {
    id: CheckId::from(ID),
    version: 1,
    tier: Tier::try_from(4).unwrap(),
    cost: CostClass::Quick,
    needs: Vec::new(),
    wants: Vec::new(),
    findings: Vec::new(),
    metrics: Vec::new(),
    stability: Stability::Experimental,
    summary: "Compares against the run's oracle (test binary only).".to_owned(),
}))));

impl Check for AgainstOracle {
    fn manifest(&self) -> &Manifest {
        &self.0
    }

    fn plan(&self, _: &CheckCtx, _: &Scope) -> Result<Plan> {
        Ok(Plan::estimate(CostClass::Quick, 1))
    }

    fn run(&self, _: &CheckCtx, _: &Facts, _: &Plan, _: &mut dyn EventSink) -> Result<CheckResult> {
        Ok(CheckResult {
            id: self.0.id.clone(),
            status: CheckStatus::Pass,
            tier: self.0.tier,
            cost: self.0.cost,
            score: None,
            metrics: BTreeMap::new(),
            delta: None,
            findings: 0,
            duration_ms: 0,
            skipped: None,
            oracle: None,
            notes: Vec::new(),
        })
    }

    fn uses_oracle(&self) -> bool {
        true
    }
}

/// A repository with one Python file and `toml` as its `cupel.toml`.
fn repo(toml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("calc.py"), "def add(a, b):\n    return a + b\n").unwrap();
    std::fs::write(dir.path().join("cupel.toml"), toml).unwrap();
    dir
}

/// `cupel <args> --format json --cwd <repo>` through `cupel_cli::run`: its exit code and envelope.
fn cupel(repo: &Path, args: &[&str]) -> (u8, Value) {
    let root = repo.display().to_string();
    let words = args.iter().copied().chain(["--format", "json", "--cwd", &root]);
    let output = cupel_cli::run(&cupel_cli::Environment::new(
        words.map(Into::into).collect(),
        Some(repo.to_path_buf()),
    ));
    let envelope = serde_json::from_str(&output.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}{}", output.stdout, output.stderr));
    (output.code, envelope)
}

/// The result of the test check in `envelope`.
fn result(envelope: &Value) -> &Value {
    envelope["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["id"] == json!(ID))
        .expect("the test check has a result")
}

/// `(kind, severity, evidence)` of each finding of `envelope`.
fn findings(envelope: &Value) -> Vec<(Value, Value, Value)> {
    envelope["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            (
                finding["kind"].clone(),
                finding["severity"].clone(),
                finding["evidence"].clone(),
            )
        })
        .collect()
}

const ECHO: &str = "[oracles.echo]\nkind = \"command\"\ncmd = \"echo {input}\"\n";
const CAT: &str = "[oracles.cat]\nkind = \"command\"\ncmd = \"printf %s {input}\"\n";

#[test]
fn a_result_against_an_unverified_oracle_is_tier_3_with_a_warn_until_it_is_verified() {
    let dir = repo(ECHO);
    let (code, envelope) = cupel(dir.path(), &["check", "--only", ID]);
    assert_eq!(code, 0, "{envelope}");
    assert_eq!(
        (&result(&envelope)["tier"], &result(&envelope)["oracle"]),
        (&json!(3), &json!("echo"))
    );
    assert_eq!(
        findings(&envelope),
        [(
            json!("oracle.unverified"),
            json!("warn"),
            json!({"oracle": "echo", "state": "unverified", "checks": [ID]})
        )]
    );
    assert_eq!(envelope["summary"]["findings"]["warn"], json!(1));
    assert_eq!(envelope["next_actions"][0]["kind"], json!("verify_oracle"));

    assert_eq!(cupel(dir.path(), &["oracle", "verify", "echo"]).0, 0);
    let (_, verified) = cupel(dir.path(), &["check", "--only", ID]);
    assert_eq!(
        (&result(&verified)["tier"], &result(&verified)["oracle"]),
        (&json!(4), &json!("echo"))
    );
    assert_eq!(findings(&verified), []);
}

#[test]
fn the_first_verified_oracle_is_the_default_and_oracle_chooses_another() {
    let dir = repo(&format!("{CAT}{ECHO}"));
    let oracle = |args: &[&str]| result(&cupel(dir.path(), args).1)["oracle"].clone();
    assert_eq!(oracle(&["check", "--only", ID]), json!("cat"));
    assert_eq!(cupel(dir.path(), &["oracle", "verify", "echo"]).0, 0);
    assert_eq!(oracle(&["check", "--only", ID]), json!("echo"));
    assert_eq!(oracle(&["check", "--only", ID, "--oracle", "cat"]), json!("cat"));
}

#[test]
fn an_oracle_that_is_not_defined_is_a_usage_error_on_oracle() {
    let dir = repo(ECHO);
    let (code, envelope) = cupel(dir.path(), &["check", "--only", ID, "--oracle", "pg"]);
    let evidence = &envelope["findings"][0]["evidence"];
    assert_eq!(
        (code, &evidence["argument"], &evidence["valid"]),
        (2, &json!("--oracle"), &json!(["echo"]))
    );
}

#[test]
fn a_check_that_uses_an_oracle_is_skipped_when_none_is_defined() {
    let dir = repo("");
    let (code, envelope) = cupel(dir.path(), &["check", "--only", ID]);
    assert_eq!(code, 0, "{envelope}");
    assert_eq!(
        result(&envelope)["skipped"],
        json!({"reason": "unconfigured", "fact": null, "lang": null, "tool": null,
               "see": "cupel describe oracle add", "est_ms": null})
    );
    let (_, plan) = cupel(dir.path(), &["check", "--only", ID, "--dry-run"]);
    let skips = &plan["plan"]["would_skip"];
    assert_eq!(
        (&skips[0]["id"], &skips[0]["reason"]),
        (&json!(ID), &json!("unconfigured"))
    );
}

#[test]
fn a_check_that_uses_no_oracle_names_none() {
    let dir = repo(ECHO);
    let (_, envelope) = cupel(dir.path(), &["check", "--only", "static.complexity"]);
    let oracles: Vec<&Value> = envelope["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|result| &result["oracle"])
        .collect();
    assert_eq!(oracles, [&json!(null)]);
}
