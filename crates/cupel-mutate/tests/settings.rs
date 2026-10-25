// ABOUTME: The engine's `[lang.<id>] mutation` table (ADR-080, ADR-082) and the cap and budget a check declares for it
// ABOUTME: (ADR-147, ADR-148): defaults, each key read, wrong shapes refused, a mutant's timeout, and each slot's share.
use std::time::Duration;

use cupel_core::Error;
use cupel_core::config::MemoryLimit;
use cupel_core::syntax::OperatorSet;
use cupel_mutate::settings::{
    DEFAULT_MAX_MUTANT_MEMORY, DEFAULT_MAX_MUTANT_TIMEOUT, DEFAULT_TIMEOUT_FACTOR,
    MAX_MUTANT_MEMORY, MAX_MUTANT_TIMEOUT, Settings, Strategy, threads,
};
use serde_json::{Map, Value, json};

/// The settings a check declares for the engine, with `max_mutant_timeout` set to `value`.
fn cap(value: &str) -> Map<String, Value> {
    Map::from_iter([(MAX_MUTANT_TIMEOUT.to_owned(), json!(value))])
}

/// The settings a check declares for the engine, with `max_mutant_memory` set to `value`.
fn budget(value: &str) -> Map<String, Value> {
    Map::from_iter([(MAX_MUTANT_MEMORY.to_owned(), json!(value))])
}

fn memory(text: &str) -> MemoryLimit {
    text.parse().unwrap()
}

fn table(value: &Value) -> Map<String, Value> {
    let Value::Object(map) = json!({ "mutation": value }) else {
        unreachable!("a JSON object literal")
    };
    map
}

#[test]
fn nothing_configured_is_the_default_set_at_the_default_factor() {
    let settings = Settings::read(&Map::new(), &Map::new()).unwrap();
    assert_eq!(
        (
            settings.operators,
            settings.timeout_factor,
            settings.strategy,
            settings.max_mutant_timeout,
            settings.max_mutant_memory
        ),
        (
            OperatorSet::Default,
            DEFAULT_TIMEOUT_FACTOR,
            Strategy::Schemata,
            Duration::from_secs(120),
            memory("50%")
        )
    );
    assert!((1..=4).contains(&settings.jobs));
}

#[test]
fn each_key_is_read() {
    let settings = Settings::read(
        &table(
            &json!({"operators": "extended", "jobs": 3, "timeout_factor": 4, "strategy": "file-copy"}),
        ),
        &cap("90s").into_iter().chain(budget("3g")).collect(),
    )
    .unwrap();
    assert_eq!(
        settings,
        Settings {
            operators: OperatorSet::Extended,
            jobs: 3,
            timeout_factor: 4,
            strategy: Strategy::FileCopy,
            max_mutant_timeout: Duration::from_secs(90),
            max_mutant_memory: memory("3g"),
        }
    );
}

/// The setting reading `value` as the engine's table names as invalid.
fn refused_setting(value: &Value) -> &'static str {
    match Settings::read(&table(value), &Map::new()) {
        Err(Error::InvalidValue { what, .. }) => what,
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_unknown_key_or_a_bad_value_is_refused() {
    let refused: Vec<&str> = [
        json!({"engine": "schemata"}),
        json!({"strategy": "fast"}),
        json!({"operators": "all"}),
        json!({"jobs": 0}),
        json!({"timeout_factor": "ten"}),
        json!("fast"),
    ]
    .iter()
    .map(refused_setting)
    .collect();
    assert_eq!(
        refused,
        [
            "mutation",
            "mutation.strategy",
            "mutation.operators",
            "mutation.jobs",
            "mutation.timeout_factor",
            "mutation"
        ]
    );
}

/// The setting reading `value` as the declared setting `name` names as invalid.
fn refused_declared(name: &str, value: Value) -> &'static str {
    let settings = Map::from_iter([(name.to_owned(), value)]);
    match Settings::read(&Map::new(), &settings) {
        Err(Error::InvalidValue { what, .. }) => what,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_cap_that_is_not_a_duration_is_refused() {
    assert_eq!(
        [json!("fast"), json!(120), json!("0s")]
            .map(|value| refused_declared(MAX_MUTANT_TIMEOUT, value)),
        [MAX_MUTANT_TIMEOUT; 3]
    );
}

#[test]
fn a_budget_that_is_neither_a_size_nor_a_share_is_refused() {
    assert_eq!(
        [json!("lots"), json!(3), json!("0g"), json!("101%")]
            .map(|value| refused_declared(MAX_MUTANT_MEMORY, value)),
        [MAX_MUTANT_MEMORY; 4]
    );
}

#[test]
fn each_slot_gets_an_equal_part_of_the_budget_and_of_the_cores() {
    let physical = Some(16 << 30);
    let share = Settings::read(&Map::new(), &budget("50%")).unwrap();
    let fixed = Settings::read(&Map::new(), &budget("3g")).unwrap();
    assert_eq!(
        [
            share.memory_per_slot(physical, 4),
            share.memory_per_slot(physical, 1),
            fixed.memory_per_slot(physical, 2),
            fixed.memory_per_slot(None, 2),
            share.memory_per_slot(None, 2),
        ],
        [
            Some(2 << 30),
            Some(8 << 30),
            Some(3 << 29),
            Some(3 << 29),
            None
        ]
    );
    assert_eq!(
        [threads(8, 4), threads(8, 1), threads(8, 16), threads(7, 2)],
        [2, 8, 1, 3]
    );
    assert_eq!(DEFAULT_MAX_MUTANT_MEMORY, "50%");
}

#[test]
fn a_mutant_gets_the_factor_times_its_reference_at_least_two_seconds_and_at_most_the_cap() {
    let settings = Settings::read(&table(&json!({"timeout_factor": 10})), &cap("2m")).unwrap();
    let timeout = |ms| settings.timeout(Duration::from_millis(ms));
    assert_eq!(
        [
            timeout(100),
            timeout(1_500),
            timeout(11_999),
            timeout(12_001),
            timeout(540_000)
        ],
        [
            Duration::from_secs(2),
            Duration::from_secs(15),
            Duration::from_millis(119_990),
            Duration::from_secs(120),
            Duration::from_secs(120),
        ]
    );
    assert_eq!(DEFAULT_MAX_MUTANT_TIMEOUT, Duration::from_secs(120));
}

#[test]
fn the_cap_a_check_declares_for_the_engines_fact_defaults_to_the_engines_own() {
    let registries = cupel_fixtures::matrix::Registries::shared();
    let declared: Vec<Duration> = registries
        .checks
        .iter()
        .flat_map(|check| &check.manifest().settings)
        .filter(|setting| {
            setting.name == MAX_MUTANT_TIMEOUT
                && setting.configures == Some(cupel_core::FactKind::MutationOutcomes)
        })
        .map(|setting| {
            let declared =
                Map::from_iter([(MAX_MUTANT_TIMEOUT.to_owned(), setting.default.clone())]);
            Settings::read(&Map::new(), &declared)
                .unwrap()
                .max_mutant_timeout
        })
        .collect();
    assert_eq!(declared, [DEFAULT_MAX_MUTANT_TIMEOUT]);
}

#[test]
fn the_budget_a_check_declares_for_the_engines_fact_is_the_engines_default() {
    let registries = cupel_fixtures::matrix::Registries::shared();
    let declared: Vec<&Value> = registries
        .checks
        .iter()
        .flat_map(|check| &check.manifest().settings)
        .filter(|setting| setting.configures == Some(cupel_core::FactKind::MutationOutcomes))
        .filter(|setting| setting.name == MAX_MUTANT_MEMORY)
        .map(|setting| &setting.default)
        .collect();
    assert_eq!(declared, [&json!(DEFAULT_MAX_MUTANT_MEMORY)]);
    assert_eq!(
        memory(DEFAULT_MAX_MUTANT_MEMORY).resolve(32 << 30),
        16 << 30
    );
}
