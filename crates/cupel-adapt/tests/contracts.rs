// ABOUTME: The contract producers (ADR-113) on the real tools: the spec.contracts fixtures' baselines are green, and the
// ABOUTME: contract run reports each contract they leave unchecked, writing nothing in the tree; skipped when absent.
mod support;

use std::path::Path;

use cupel_core::{
    Availability, Cancellation, ContractViolations, Ctx, FactKind, Produced, ProducerId, RunStatus,
    Scope, TestResults,
};
use serde_json::{Value, json};

/// Every file of the copy at `root` except what the runs build and cache into.
fn sources(root: &Path) -> Vec<String> {
    support::tree(root)
        .into_iter()
        .filter(|file| !file.starts_with("target/"))
        .collect()
}

/// The contract run of `tool` on a copy of the spec.contracts fixture for `lang`, after the
/// baseline of `runner`, which must be green; `None` (reported as skipped) when the runner is
/// not installed here.
fn contract_run(tool: &str, runner: &str, lang: &str) -> Option<ContractViolations> {
    let fixture = support::own_fixture("spec.contracts", lang);
    let root = fixture.path();
    let registry = support::shared_registry();
    let producer = support::producer(registry, FactKind::ContractViolations, tool, lang);
    let runner_producer = support::runner(registry, runner, lang);
    if let Availability::MissingTool { name, .. } =
        runner_producer.availability(&Ctx::new(root.to_path_buf()))
    {
        eprintln!("SKIPPED: {name} not installed; {tool} did not run");
        return None;
    }
    let before = sources(root);
    let mut ctx = support::produce_ctx(root, lang, Cancellation::new());
    ctx.runners = registry.runners();
    let scope = Scope::default();
    let baseline = runner_producer
        .produce(&ctx, &scope, &mut Vec::new())
        .unwrap();
    assert!(
        baseline.downcast_ref::<TestResults>().unwrap().is_green(),
        "the project's own run leaves the contracts unchecked: {:?}",
        baseline.to_json()
    );
    ctx.upstream.insert(baseline.share());
    let fact = producer.produce(&ctx, &scope, &mut Vec::new()).unwrap();
    assert_eq!(sources(root), before, "{tool} wrote in the tree");
    Some(fact.downcast::<ContractViolations>().unwrap())
}

/// Each violation as (kind, function, file, line, test), sorted by test.
fn summary(fact: &ContractViolations) -> Value {
    let mut rows: Vec<Value> = fact
        .violations
        .iter()
        .map(|violation| {
            let place = violation.place.as_ref();
            json!([
                violation.kind,
                violation.function,
                place.map(|place| &place.file),
                place.map(|place| place.line),
                violation.test,
            ])
        })
        .collect();
    rows.sort_by_key(|row| row[4].as_str().unwrap_or_default().to_owned());
    json!(rows)
}

#[test]
fn the_rust_contract_run_turns_on_debug_contracts_and_reads_each_violation() {
    let Some(fact) = contract_run("tool.rust-contracts", "tool.cargo-test", "rust") else {
        return;
    };
    assert!(fact.declared);
    assert_eq!(fact.runner.as_str(), "tool.cargo-test");
    assert_eq!(fact.runs.len(), 1);
    assert_eq!(fact.runs[0].status, RunStatus::Ran, "{:?}", fact.runs);
    assert_eq!(fact.tests, 4);
    assert_eq!(
        summary(&fact),
        json!([
            [
                "postcondition",
                "half",
                "src/lib.rs",
                10,
                "ledger/src/lib.rs::tests::halves_one"
            ],
            [
                "invariant",
                "post",
                "src/lib.rs",
                25,
                "ledger/src/lib.rs::tests::posts_three_entries"
            ],
            [
                "precondition",
                "share",
                "src/lib.rs",
                4,
                "ledger/src/lib.rs::tests::shares_nothing_among_no_parts"
            ],
        ])
    );
}

#[test]
fn the_icontract_run_turns_on_slow_contracts_and_reads_each_violation() {
    let imports = std::process::Command::new("python3")
        .args(["-c", "import icontract"])
        .status()
        .is_ok_and(|status| status.success());
    if !imports {
        eprintln!("SKIPPED: python3 does not import icontract; tool.icontract did not run");
        return;
    }
    let Some(fact) = contract_run("tool.icontract", "tool.pytest", "python") else {
        return;
    };
    assert!(fact.declared);
    assert_eq!(fact.tests, 4);
    assert_eq!(
        summary(&fact),
        json!([
            [
                "postcondition",
                "half",
                "src/ledger.py",
                11,
                "tests/test_ledger.py::test_halves_one"
            ],
            [
                "invariant",
                "Account",
                "src/ledger.py",
                21,
                "tests/test_ledger.py::test_posts_three_entries"
            ],
            [
                "precondition",
                "share",
                "src/ledger.py",
                6,
                "tests/test_ledger.py::test_shares_nothing_among_no_parts"
            ],
        ])
    );
}

#[test]
fn a_project_that_declares_no_contract_library_is_not_run_again() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"plain\"\n",
    )
    .unwrap();
    let mut ctx = support::produce_ctx(dir.path(), "rust", Cancellation::new());
    ctx.upstream.insert(
        Produced::new(TestResults {
            runner: ProducerId::from("tool.cargo-test"),
            invocations: Vec::new(),
            tests: Vec::new(),
            suite_hash: String::new(),
        })
        .share(),
    );
    let producer = support::producer(
        support::shared_registry(),
        FactKind::ContractViolations,
        "tool.rust-contracts",
        "rust",
    );
    let fact = producer
        .produce(&ctx, &Scope::default(), &mut Vec::new())
        .unwrap()
        .downcast::<ContractViolations>()
        .unwrap();
    assert_eq!(
        serde_json::to_value(&fact).unwrap(),
        json!({
            "tool": "tool.rust-contracts",
            "runner": "tool.cargo-test",
            "declared": false,
            "runs": [],
            "tests": 0,
            "violations": [],
        })
    );
    assert_eq!(
        producer.needs(),
        [FactKind::TestResults, FactKind::TestFrames]
    );
}
