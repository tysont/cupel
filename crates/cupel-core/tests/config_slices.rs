// ABOUTME: Tests for the config slices handed to checks and producers: a check's table with its
// ABOUTME: per-language override merged in (`CheckCtx.config`), and a language's `[lang.<id>]` table.
use std::path::{Path, PathBuf};

use cupel_core::config::{Config, ConfigFile, ConfigRequest};
use cupel_core::{CheckId, LangId};
use serde_json::{Value, json};

fn config(text: &str) -> Config {
    let file = ConfigFile::parse(text, Path::new("/repo/cupel.toml")).unwrap();
    Config::resolve(
        Some(PathBuf::from("/repo/cupel.toml")),
        file,
        &ConfigRequest::default(),
    )
    .unwrap()
}

fn slice(config: &Config, check: &str, lang: &str) -> Value {
    Value::Object(config.check_config(&CheckId::from(check), &LangId::from(lang)))
}

const LAYERED: &str = r#"
[checks."static.deps"]
strict = false
exclude = ["vendor/**"]

[checks."static.deps".layers]
order = ["domain", "app", "infra"]
map = { domain = "src/domain/**", app = "src/app/**", infra = "src/infra/**" }

[checks."static.deps".lang.go]
strict = true
exclude = ["third_party/**"]
max_depth = 3

[checks."static.deps".lang.go.layers]
map = { infra = "internal/infra/**" }

[checks."static.complexity".lang.go]
cyclomatic_max = 15
"#;

#[test]
fn a_language_without_an_override_gets_the_neutral_table() {
    let config = config(LAYERED);
    assert_eq!(
        slice(&config, "static.deps", "rust"),
        json!({
            "strict": false,
            "exclude": ["vendor/**"],
            "layers": {
                "order": ["domain", "app", "infra"],
                "map": {"domain": "src/domain/**", "app": "src/app/**", "infra": "src/infra/**"}
            }
        })
    );
}

#[test]
fn an_override_deep_merges_tables_and_replaces_everything_else() {
    let config = config(LAYERED);
    assert_eq!(
        slice(&config, "static.deps", "go"),
        json!({
            "strict": true,
            "exclude": ["third_party/**"],
            "max_depth": 3,
            "layers": {
                "order": ["domain", "app", "infra"],
                "map": {"domain": "src/domain/**", "app": "src/app/**", "infra": "internal/infra/**"}
            }
        })
    );
}

#[test]
fn an_override_alone_is_the_whole_slice_for_that_language() {
    let config = config(LAYERED);
    assert_eq!(
        slice(&config, "static.complexity", "go"),
        json!({"cyclomatic_max": 15})
    );
    assert_eq!(slice(&config, "static.complexity", "rust"), json!({}));
}

#[test]
fn an_unconfigured_check_gets_an_empty_slice() {
    assert_eq!(slice(&config(LAYERED), "tests.mutation", "go"), json!({}));
}

#[test]
fn the_design_example_mutation_override() {
    let config = config(
        r#"
[checks."tests.mutation"]
timeout_factor = 10
exclude_tags   = ["no-mutate"]

[checks."tests.mutation".lang.python]
operators = "extended"
"#,
    );
    assert_eq!(
        slice(&config, "tests.mutation", "python"),
        json!({"timeout_factor": 10, "exclude_tags": ["no-mutate"], "operators": "extended"})
    );
    assert_eq!(
        slice(&config, "tests.mutation", "rust"),
        json!({"timeout_factor": 10, "exclude_tags": ["no-mutate"]})
    );
}

/// Producers read `[lang.<id>]`; checks never do (design §6.7).
#[test]
fn language_tables_are_separate_from_check_slices() {
    let config = config(
        r#"
[lang.python]
coverage = "coverage.py"
test = "pytest"

[checks."tests.coverage".lang.python]
branch = true
"#,
    );
    assert_eq!(
        Value::Object(config.lang_config(&LangId::from("python"))),
        json!({"coverage": "coverage.py", "test": "pytest"})
    );
    assert_eq!(
        slice(&config, "tests.coverage", "python"),
        json!({"branch": true})
    );
    assert_eq!(
        Value::Object(config.lang_config(&LangId::from("go"))),
        json!({})
    );
}
