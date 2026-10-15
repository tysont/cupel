// ABOUTME: The engine's `[lang.<id>] mutation` table (ADR-080): defaults, each key read, and every other key or a
// ABOUTME: value of the wrong shape refused with what the table takes.
use cupel_core::Error;
use cupel_core::syntax::OperatorSet;
use cupel_mutate::settings::{DEFAULT_TIMEOUT_FACTOR, Settings};
use serde_json::{Map, Value, json};

fn table(value: &Value) -> Map<String, Value> {
    let Value::Object(map) = json!({ "mutation": value }) else {
        unreachable!("a JSON object literal")
    };
    map
}

#[test]
fn nothing_configured_is_the_default_set_at_the_default_factor() {
    let settings = Settings::read(&Map::new()).unwrap();
    assert_eq!(
        (settings.operators, settings.timeout_factor),
        (OperatorSet::Default, DEFAULT_TIMEOUT_FACTOR)
    );
    assert!((1..=4).contains(&settings.jobs));
}

#[test]
fn each_key_is_read() {
    let settings = Settings::read(&table(
        &json!({"operators": "extended", "jobs": 3, "timeout_factor": 4}),
    ))
    .unwrap();
    assert_eq!(
        settings,
        Settings {
            operators: OperatorSet::Extended,
            jobs: 3,
            timeout_factor: 4
        }
    );
}

/// The setting reading `value` as the engine's table names as invalid.
fn refused_setting(value: &Value) -> &'static str {
    match Settings::read(&table(value)) {
        Err(Error::InvalidValue { what, .. }) => what,
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_unknown_key_or_a_bad_value_is_refused() {
    let refused: Vec<&str> = [
        json!({"strategy": "schemata"}),
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
            "mutation.operators",
            "mutation.jobs",
            "mutation.timeout_factor",
            "mutation"
        ]
    );
}
