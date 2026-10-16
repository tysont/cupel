// ABOUTME: The budget scheduler's pure parts on synthetic DAGs (ADR-086): the cost model from recorded
// ABOUTME: durations, task value, and the greedy knapsack that takes each check with its dependency closure.
use cupel_core::budget::{
    Candidate, History, MIN_RUNS, Novelty, TaskShape, choose, p50, tie, value,
};
use cupel_core::{CheckId, CostClass, LangId, ProducerId, Tier};

/// A check task worth `value`, costing `cost_ms` itself, needing the producer tasks `closure`.
fn candidate(value: f64, cost_ms: u64, closure: &[usize]) -> Candidate {
    Candidate {
        value,
        cost_ms,
        closure: closure.to_vec(),
        tie: 0,
    }
}

fn taken(choice: &cupel_core::budget::Choice) -> Vec<usize> {
    choice.taken.iter().map(|(index, _)| *index).collect()
}

// ---------------------------------------------------------------- the knapsack

#[test]
fn without_a_budget_every_task_is_taken_in_order_and_each_producer_is_costed_once() {
    let producers = [1_000, 50];
    let candidates = [
        candidate(1.0, 10, &[0]),
        candidate(8.0, 20, &[0, 1]),
        candidate(1.0, 5, &[]),
    ];
    let choice = choose(&candidates, &producers, None);
    assert_eq!(choice.taken, [(0, 1_010), (1, 70), (2, 5)]);
    assert!(choice.skipped.is_empty());
}

#[test]
fn the_budget_is_never_exceeded_by_what_is_taken() {
    let producers = [400, 300, 200];
    let candidates = [
        candidate(1.0, 100, &[0]),
        candidate(2.0, 100, &[1]),
        candidate(3.0, 100, &[2]),
        candidate(1.0, 100, &[0, 1, 2]),
    ];
    for budget in [0, 99, 250, 500, 700, 1_000, 1_400] {
        let choice = choose(&candidates, &producers, Some(budget));
        let spent: u64 = choice.taken.iter().map(|(_, marginal)| marginal).sum();
        assert!(spent <= budget, "{budget}: spent {spent}");
        assert_eq!(choice.taken.len() + choice.skipped.len(), candidates.len());
    }
}

#[test]
fn a_check_is_taken_only_with_its_closure_and_a_shared_producer_costs_once() {
    // The coverage run (producer 0) costs 1 000; two checks share it.
    let producers = [1_000];
    let candidates = [candidate(4.0, 10, &[0]), candidate(4.0, 10, &[0])];
    let choice = choose(&candidates, &producers, Some(1_020));
    assert_eq!(choice.taken, [(0, 1_010), (1, 10)]);
    // Without room for the closure, neither runs, however cheap the check itself is.
    let choice = choose(&candidates, &producers, Some(999));
    assert!(choice.taken.is_empty());
    assert_eq!(choice.skipped, [(0, 1_010), (1, 1_010)]);
}

#[test]
fn the_highest_value_per_millisecond_is_chosen_first() {
    let producers = [];
    let candidates = [
        candidate(1.0, 100, &[]),  // 0.01 per ms
        candidate(7.0, 100, &[]),  // 0.07 per ms
        candidate(4.0, 50, &[]),   // 0.08 per ms
        candidate(20.0, 900, &[]), // 0.022 per ms
    ];
    let choice = choose(&candidates, &producers, Some(250));
    assert_eq!(taken(&choice), [2, 1, 0]);
    assert_eq!(choice.skipped, [(3, 900)]);
}

#[test]
fn a_task_that_does_not_fit_leaves_room_for_cheaper_ones_after_it() {
    let producers = [];
    let candidates = [candidate(100.0, 600, &[]), candidate(1.0, 300, &[])];
    let choice = choose(&candidates, &producers, Some(500));
    assert_eq!(taken(&choice), [1]);
    assert_eq!(choice.skipped, [(0, 600)]);
}

#[test]
fn a_task_whose_closure_alone_exceeds_the_budget_is_skipped_with_its_whole_cost() {
    // tests.mutation: the baseline (30 s), coverage (30 s), and the engine (30 s), then 500 ms itself.
    let producers = [100, 30_000, 30_000, 30_000];
    let candidates = [candidate(1.0, 3, &[0]), candidate(4.0, 500, &[0, 1, 2, 3])];
    let choice = choose(&candidates, &producers, Some(1_000));
    assert_eq!(choice.taken, [(0, 103)]);
    assert_eq!(
        choice.skipped,
        [(1, 90_500)],
        "the taken producer costs nothing again"
    );
}

#[test]
fn ties_are_broken_by_the_seeded_key_so_a_seed_repeats_the_order() {
    let producers = [];
    let with_ties = |ties: [u64; 3]| -> Vec<Candidate> {
        ties.iter()
            .map(|tie| Candidate {
                tie: *tie,
                ..candidate(1.0, 10, &[])
            })
            .collect()
    };
    let check = |id: &str| CheckId::from(id);
    let rust = LangId::from("rust");
    for seed in [1, 2, 42] {
        let keys = [
            tie(seed, &check("static.a"), &rust),
            tie(seed, &check("static.b"), &rust),
            tie(seed, &check("static.c"), &rust),
        ];
        let first = taken(&choose(&with_ties(keys), &producers, Some(100)));
        let again = taken(&choose(&with_ties(keys), &producers, Some(100)));
        assert_eq!(first, again, "seed {seed}");
        let mut by_key = vec![0, 1, 2];
        by_key.sort_by_key(|index| keys[*index]);
        assert_eq!(first, by_key, "seed {seed}");
    }
    assert_ne!(
        tie(1, &check("static.a"), &rust),
        tie(2, &check("static.a"), &rust)
    );
}

// ---------------------------------------------------------------- value

#[test]
fn value_is_tier_weight_times_novelty_times_the_change_bonus() {
    let tier = |t: u8| Tier::try_from(t).unwrap();
    let values = [
        value(tier(0), Novelty::Scored, false),
        value(tier(3), Novelty::Scored, false),
        value(tier(3), Novelty::Unscored, false),
        value(tier(3), Novelty::Repeated, false),
        value(tier(3), Novelty::Unscored, true),
    ];
    assert_eq!(
        values.map(|value| value.to_string()),
        ["1", "4", "8", "2", "16"]
    );
}

#[test]
fn a_check_the_store_never_scored_is_novel() {
    let mut history = History::default();
    let check = CheckId::from("tests.mutation");
    assert_eq!(history.novelty(&check), Novelty::Unscored);
    history.set_novelty(check.clone(), Novelty::Repeated);
    assert_eq!(history.novelty(&check), Novelty::Repeated);
}

// ---------------------------------------------------------------- cost

#[test]
fn p50_is_the_lower_median() {
    assert_eq!(p50(&[]), None);
    assert_eq!(p50(&[7]), Some(7));
    assert_eq!(p50(&[30, 10, 20]), Some(20));
    assert_eq!(p50(&[40, 10, 30, 20]), Some(20));
}

#[test]
fn recorded_runs_beat_the_default_once_there_are_enough() {
    let shape = TaskShape::producer(
        &ProducerId::from("tool.pytest"),
        &LangId::from("python"),
        false,
    );
    let mut history = History::default();
    let default = CostClass::Standard.est_ms_per_production();
    assert_eq!(history.cost_ms(&shape), None);
    assert_eq!(history.cost_or(&shape, default), default);
    let few: Vec<u64> = (1..MIN_RUNS as u64).map(|n| n * 100).collect();
    history.insert(shape.clone(), few);
    assert_eq!(
        history.cost_or(&shape, default),
        default,
        "fewer than MIN_RUNS recorded runs fall back to the default"
    );
    history.insert(shape.clone(), vec![900, 1_200, 1_000, 5_000]);
    assert_eq!(history.cost_or(&shape, default), 1_000);
}

#[test]
fn only_the_last_ten_recorded_runs_count() {
    let shape = TaskShape::check(&CheckId::from("static.clones"), &LangId::from("rust"), 120);
    let mut history = History::default();
    // Newest first: ten fast runs, then older slow ones that no longer count.
    let mut recorded = vec![10; 10];
    recorded.extend([10_000; 20]);
    history.insert(shape.clone(), recorded);
    assert_eq!(history.cost_ms(&shape), Some(10));
}

#[test]
fn shapes_separate_sizes_languages_and_cached_productions() {
    let check = CheckId::from("static.clones");
    let rust = LangId::from("rust");
    assert_eq!(TaskShape::check(&check, &rust, 0).class, "0");
    assert_eq!(TaskShape::check(&check, &rust, 9).class, "1");
    assert_eq!(TaskShape::check(&check, &rust, 10).class, "2");
    assert_eq!(TaskShape::check(&check, &rust, 999).class, "3");
    assert_ne!(
        TaskShape::check(&check, &rust, 5),
        TaskShape::check(&check, &LangId::from("go"), 5)
    );
    let producer = ProducerId::from("tool.cargo-llvm-cov");
    assert_eq!(TaskShape::producer(&producer, &rust, true).class, "cached");
    assert_eq!(TaskShape::producer(&producer, &rust, false).class, "run");
}

#[test]
fn a_production_s_default_follows_its_cost_class() {
    assert_eq!(CostClass::Quick.est_ms_per_production(), 100);
    assert_eq!(CostClass::Standard.est_ms_per_production(), 30_000);
    assert_eq!(CostClass::Deep.est_ms_per_production(), 300_000);
    assert_eq!(CostClass::Exhaustive.est_ms_per_production(), 3_000_000);
}
