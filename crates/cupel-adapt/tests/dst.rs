// ABOUTME: `tool.madsim` (ADR-120) on the real simulator: the conc.dst fixture's harnesses conform to the
// ABOUTME: `simulation-results` inventory, the seed range is `--seed` and `--cases`, and nothing is written in the tree.
mod support;

use std::fs;
use std::path::Path;

use cupel_core::{
    Availability, Cancellation, FactKind, LangId, RunStatus, Scope, SimulationResults,
    SimulationStatus,
};
use cupel_fixtures::conformance::{LanguageInventory, check_language, fact_inventories};

/// What `tool.madsim` makes of the repository at `root` over `cases` seeds from `seed`; `None`
/// (reported as skipped) when cargo is not installed.
fn simulated(root: &Path, seed: u64, cases: u64) -> Option<SimulationResults> {
    let producer = support::producer(
        support::shared_registry(),
        FactKind::SimulationResults,
        "tool.madsim",
        "rust",
    );
    let mut ctx = support::produce_ctx(root, "rust", Cancellation::new());
    ctx.seed = seed;
    ctx.focus.cases = Some(cases);
    if let Availability::MissingTool { name, .. } = producer.availability(&ctx.ctx) {
        eprintln!("SKIPPED: {name} not installed; tool.madsim was not run");
        return None;
    }
    let fact = producer
        .produce(&ctx, &Scope::default(), &mut Vec::new())
        .unwrap();
    Some(fact.downcast::<SimulationResults>().unwrap())
}

#[test]
fn madsim_reports_every_construct_of_the_simulation_results_inventory() {
    let fixture = support::own_fixture("conc.dst", "rust");
    let root = fixture.path();
    let before = support::tree(root);
    let Some(results) = simulated(root, 3, 1000) else {
        return;
    };
    assert_eq!(support::tree(root), before, "tool.madsim wrote in the tree");
    assert_eq!(results.runs.len(), 1);
    assert_eq!(results.runs[0].status, RunStatus::Ran, "{:?}", results.runs);
    assert_eq!((results.seed, results.seeds), (3, 1000));
    let inventory = LanguageInventory::parse(
        &LangId::from("rust"),
        &fs::read_to_string(root.join("inventory.toml")).unwrap(),
    )
    .unwrap();
    let fact = serde_json::to_value(&results).unwrap();
    let failures = check_language(
        &fact_inventories(),
        &inventory,
        &[FactKind::SimulationResults],
        |_| Ok(fact.clone()),
    );
    assert!(failures.is_empty(), "{failures:#?}\n{fact:#}");
    let lost = results
        .harnesses
        .iter()
        .find(|harness| harness.name == "concurrent_deposits_are_all_kept")
        .and_then(|harness| harness.failure.as_ref())
        .unwrap();
    assert_eq!(
        lost.message,
        "assertion `left == right` failed: a deposit was lost"
    );
    assert_eq!(
        lost.trace[..4],
        [
            "deposit 10 reads 0",
            "deposit 20 reads 0",
            "deposit 10 writes 10",
            "deposit 20 writes 20"
        ]
    );
    assert_eq!(
        lost.reproduce,
        "RUSTFLAGS='--cfg madsim' MADSIM_TEST_SEED=4 MADSIM_TEST_NUM=1 cargo test -p ledger \
         --test sim concurrent_deposits_are_all_kept -- --exact"
    );
}

#[test]
fn a_range_of_seeds_that_all_pass_passes_and_one_seed_replays_its_failure() {
    let fixture = support::own_fixture("conc.dst", "rust");
    let root = fixture.path();
    let Some(passing) = simulated(root, 3, 1) else {
        return;
    };
    let statuses: Vec<(&str, SimulationStatus)> = passing
        .harnesses
        .iter()
        .map(|harness| (harness.name.as_str(), harness.status))
        .collect();
    assert_eq!(
        statuses,
        [
            (
                "a_withdrawal_waits_for_its_deposit",
                SimulationStatus::Passed
            ),
            ("concurrent_deposits_are_all_kept", SimulationStatus::Passed),
            ("sequential_deposits_are_all_kept", SimulationStatus::Passed),
        ]
    );
    let replayed = simulated(root, 5, 1).unwrap();
    let failing: Vec<(&str, Option<u64>)> = replayed
        .harnesses
        .iter()
        .filter_map(|harness| Some((harness.name.as_str(), harness.failure.as_ref()?.seed)))
        .collect();
    assert_eq!(failing, [("a_withdrawal_waits_for_its_deposit", Some(5))]);
}
