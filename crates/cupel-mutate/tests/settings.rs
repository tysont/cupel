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

#[test]
fn an_unknown_key_or_a_bad_value_is_refused() {
    for (value, what) in [
        (json!({"strategy": "schemata"}), "mutation"),
        (json!({"operators": "all"}), "mutation.operators"),
        (json!({"jobs": 0}), "mutation.jobs"),
        (json!({"timeout_factor": "ten"}), "mutation.timeout_factor"),
        (json!("fast"), "mutation"),
    ] {
        let error = Settings::read(&table(&value)).unwrap_err();
        assert!(
            matches!(error, Error::InvalidValue { what: found, .. } if found == what),
            "{error:?}"
        );
    }
}
