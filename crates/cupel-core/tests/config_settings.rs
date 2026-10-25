// ABOUTME: Tests for the layered settings: their value types, the `CUPEL_*` env mapping, and env parsing.
// ABOUTME: The env mapping is defined once on `Setting`; these tests pin its names to design §6.7.
use std::collections::BTreeMap;
use std::time::Duration;

use cupel_core::config::{
    ConfigError, Format, MemoryLimit, PROFILE_ENV_VAR, Setting, Settings, TimeLimit,
};
use cupel_core::{CostClass, Error};

fn env(vars: &[(&str, &str)]) -> BTreeMap<String, String> {
    vars.iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn env_vars_are_cupel_plus_the_uppercased_key() {
    let names: Vec<(&str, &str)> = Setting::ALL
        .iter()
        .map(|setting| (setting.key(), setting.env_var()))
        .collect();
    assert_eq!(
        names,
        [
            ("depth", "CUPEL_DEPTH"),
            ("format", "CUPEL_FORMAT"),
            ("budget", "CUPEL_BUDGET"),
        ]
    );
    for setting in Setting::ALL {
        assert_eq!(
            setting.env_var(),
            format!("CUPEL_{}", setting.key().to_uppercase())
        );
    }
    assert_eq!(PROFILE_ENV_VAR, "CUPEL_PROFILE");
}

#[test]
fn reads_every_setting_from_env() {
    let settings = Settings::from_env(&env(&[
        ("CUPEL_DEPTH", "deep"),
        ("CUPEL_FORMAT", "sarif"),
        ("CUPEL_BUDGET", "20s"),
    ]))
    .unwrap();
    assert_eq!(
        settings,
        Settings {
            depth: Some(CostClass::Deep),
            format: Some(Format::Sarif),
            budget: Some("20s".parse().unwrap()),
        }
    );
}

#[test]
fn ignores_unrelated_and_unknown_env_vars() {
    let settings = Settings::from_env(&env(&[
        ("PATH", "/usr/bin"),
        ("CUPEL_NO_DAEMON", "1"),
        ("cupel_depth", "deep"),
    ]))
    .unwrap();
    assert_eq!(settings, Settings::default());
}

#[test]
fn invalid_env_value_is_a_usage_error_naming_the_variable() {
    let err = Settings::from_env(&env(&[("CUPEL_DEPTH", "medium")])).unwrap_err();
    assert!(matches!(
        &err,
        Error::Config(ConfigError::InvalidEnv { var: "CUPEL_DEPTH", value, .. }) if value == "medium"
    ));
    assert_eq!(
        err.to_string(),
        "invalid value `medium` for CUPEL_DEPTH: unknown variant `medium`, \
         expected one of `quick`, `standard`, `deep`, `exhaustive`"
    );
}

#[test]
fn format_names() {
    for (name, format) in [
        ("json", Format::Json),
        ("jsonl", Format::Jsonl),
        ("text", Format::Text),
        ("sarif", Format::Sarif),
    ] {
        assert_eq!(format.as_str(), name);
        assert_eq!(serde_json::to_value(format).unwrap(), name);
    }
    assert_eq!(Format::ALL.len(), 4);
}

#[test]
fn time_limits_parse_whole_numbers_with_a_unit() {
    for (text, duration) in [
        ("250ms", Duration::from_millis(250)),
        ("10s", Duration::from_secs(10)),
        ("5m", Duration::from_secs(300)),
        ("1h", Duration::from_secs(3600)),
    ] {
        let limit: TimeLimit = text.parse().unwrap();
        assert_eq!(limit.as_duration(), duration, "{text}");
        assert_eq!(limit.to_string(), text);
        assert_eq!(serde_json::to_value(limit).unwrap(), text);
    }
}

#[test]
fn time_limits_reject_everything_else() {
    for text in [
        "", "10", "s", "0s", "-5s", "1.5s", "10 s", "10sec", "5d", "1h30m", "ten s",
    ] {
        let err = text.parse::<TimeLimit>().unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "invalid duration `{text}`: expected a positive whole number followed by \
                 `ms`, `s`, `m`, or `h`, such as `20s`"
            ),
            "{text:?}"
        );
    }
}

#[test]
fn memory_limits_parse_whole_numbers_with_a_unit_or_a_share_of_physical_memory() {
    let physical = 16 << 30;
    for (text, bytes) in [
        ("512k", 512 << 10),
        ("3m", 3 << 20),
        ("2g", 2 << 30),
        ("50%", 8 << 30),
        ("100%", 16 << 30),
        ("1%", physical / 100),
    ] {
        let limit: MemoryLimit = text.parse().unwrap();
        assert_eq!(limit.resolve(physical), bytes, "{text}");
        assert_eq!(limit.to_string(), text);
    }
}

#[test]
fn memory_limits_reject_everything_else() {
    for text in [
        "", "10", "m", "0m", "-1g", "1.5g", "10 m", "3gb", "3G", "0%", "101%", "50 %",
    ] {
        let err = text.parse::<MemoryLimit>().unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "invalid memory limit `{text}`: expected a positive whole number followed by \
                 `k`, `m`, or `g`, such as `512m`, or a share of physical memory from `1%` to \
                 `100%`"
            ),
            "{text:?}"
        );
    }
}
