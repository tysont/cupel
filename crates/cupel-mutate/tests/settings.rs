// ABOUTME: The engine's `[lang.<id>] mutation` table (ADR-080, ADR-082) and the cap a check declares for it (ADR-147):
// ABOUTME: defaults, each key read, every other key or a value of the wrong shape refused, and a mutant's timeout.
use std::time::Duration;

use cupel_core::Error;
use cupel_core::syntax::OperatorSet;
use cupel_mutate::settings::{
    DEFAULT_MAX_MUTANT_TIMEOUT, DEFAULT_TIMEOUT_FACTOR, MAX_MUTANT_TIMEOUT, Settings, Strategy,
};
use serde_json::{Map, Value, json};

/// The settings a check declares for the engine, with `max_mutant_timeout` set to `value`.
fn cap(value: &str) -> Map<String, Value> {
    Map::from_iter([(MAX_MUTANT_TIMEOUT.to_owned(), json!(value))])
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
            settings.max_mutant_timeout
        ),
        (
            OperatorSet::Default,
            DEFAULT_TIMEOUT_FACTOR,
            Strategy::Schemata,
            Duration::from_secs(120)
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
        &cap("90s"),
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

/// The setting reading `value` as the cap names as invalid.
fn refused_cap(value: Value) -> &'static str {
    let settings = Map::from_iter([(MAX_MUTANT_TIMEOUT.to_owned(), value)]);
    match Settings::read(&Map::new(), &settings) {
        Err(Error::InvalidValue { what, .. }) => what,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_cap_that_is_not_a_duration_is_refused() {
    assert_eq!(
        [json!("fast"), json!(120), json!("0s")].map(refused_cap),
        [MAX_MUTANT_TIMEOUT; 3]
    );
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
