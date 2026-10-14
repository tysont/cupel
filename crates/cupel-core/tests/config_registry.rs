// ABOUTME: Tests for validating a parsed config against the check and language registries: unknown
// ABOUTME: check IDs, gate metrics, and language IDs become usage errors listing what is registered.
mod support;

use std::path::{Path, PathBuf};

use cupel_core::config::{Config, ConfigError, ConfigFile, ConfigRequest};
use cupel_core::{Check, CheckRegistry, Error, LangId, Manifest, MetricPer, MetricSpec, Result};
use support::Declared;

/// A check declaring `metrics`; a name ending in `@symbol` is a per-symbol ranking dimension.
fn declared(id: &str, metrics: &[&str]) -> Box<dyn Check> {
    Box::new(Declared(Manifest {
        metrics: metrics
            .iter()
            .map(|name| match name.strip_suffix("@symbol") {
                Some(name) => MetricSpec {
                    name: name.to_owned(),
                    summary: String::new(),
                    per: MetricPer::Symbol,
                },
                None => MetricSpec {
                    name: (*name).to_owned(),
                    summary: String::new(),
                    per: MetricPer::Scope,
                },
            })
            .collect(),
        ..support::manifest(id, &[], &[])
    }))
}

fn registry() -> CheckRegistry {
    let mut checks = CheckRegistry::default();
    checks
        .register(declared("tests.mutation", &["score", "survivors@symbol"]))
        .unwrap();
    checks.register(declared("tests.weakening", &[])).unwrap();
    checks
}

fn langs() -> Vec<LangId> {
    ["go", "python", "rust"].map(LangId::from).to_vec()
}

fn validate(text: &str) -> Result<()> {
    let file = ConfigFile::parse(text, Path::new("/repo/cupel.toml")).unwrap();
    let config = Config::resolve(
        Some(PathBuf::from("/repo/cupel.toml")),
        file,
        &ConfigRequest::default(),
    )
    .unwrap();
    config.validate_registered(&registry(), &langs())
}

#[test]
fn a_config_naming_only_registered_ids_is_valid() {
    let result = validate(
        r#"
[roots]
api = { path = "api", lang = "go" }

[lang.rust]
test = "cargo test"

[checks."tests.mutation"]
timeout_factor = 10

[checks."tests.mutation".lang.python]
operators = "extended"

[gate.changed]
"tests.mutation.score" = ">= 0.80"
"tests.weakening" = "none"
"#,
    )
    .map_err(|err| err.to_string());
    assert_eq!(result, Ok(()));
}

#[test]
fn unregistered_ids_are_usage_errors_listing_the_valid_ones() {
    let cases = [
        (
            "[checks.\"tests.mutaton\"]\nx = 1\n",
            "unknown check `tests.mutaton` in `[checks]`; registered: `tests.mutation`, `tests.weakening`",
        ),
        (
            "[gate.tree]\n\"static.crap.max\" = \"< 30\"\n",
            "unknown check `static.crap` in `[gate.tree]`; registered: `tests.mutation`, `tests.weakening`",
        ),
        (
            "[gate.changed]\n\"tests.mutation.ratio\" = \">= 0.8\"\n",
            "unknown metric `ratio` of `tests.mutation` in `[gate.changed]`; registered: `score`",
        ),
        (
            "[gate.changed]\n\"tests.mutation.survivors\" = \"< 3\"\n",
            "unknown metric `survivors` of `tests.mutation` in `[gate.changed]`; registered: `score`",
        ),
        (
            "[gate.changed]\n\"tests.weakening.count\" = \"== 0\"\n",
            "unknown metric `count` of `tests.weakening` in `[gate.changed]`; registered: none",
        ),
        (
            "[lang.ruby]\ntest = \"rspec\"\n",
            "unknown language `ruby` in `[lang]`; registered: `go`, `python`, `rust`",
        ),
        (
            "[roots]\nweb = { path = \"web\", lang = \"typescript\" }\n",
            "unknown language `typescript` in `[roots.web]`; registered: `go`, `python`, `rust`",
        ),
        (
            "[checks.\"tests.mutation\".lang.java]\noperators = \"extended\"\n",
            "unknown language `java` in `[checks.\"tests.mutation\".lang]`; registered: `go`, `python`, `rust`",
        ),
    ];
    for (text, expected) in cases {
        let err = validate(text).expect_err(text);
        assert!(
            matches!(err, Error::Config(ConfigError::Unregistered { .. })),
            "{err:?}"
        );
        assert_eq!(err.to_string(), expected, "{text}");
    }
}

fn validate_languages(text: &str) -> Result<()> {
    let file = ConfigFile::parse(text, Path::new("/repo/cupel.toml")).unwrap();
    let config = Config::resolve(
        Some(PathBuf::from("/repo/cupel.toml")),
        file,
        &ConfigRequest::default(),
    )
    .unwrap();
    config.validate_languages(&langs())
}

#[test]
fn language_validation_accepts_check_tables_and_gates_while_no_check_is_registered() {
    let result = validate_languages(
        r#"
[roots]
api = { path = "api", lang = "go" }

[lang.rust]
test = "cargo test"

[checks."static.clones"]
threshold = 0.82

[checks."static.clones".lang.python]
threshold = 0.9

[gate.changed]
"tests.mutation.score" = ">= 0.80"
"#,
    )
    .map_err(|err| err.to_string());
    assert_eq!(result, Ok(()));
}

#[test]
fn language_validation_rejects_unregistered_languages_wherever_they_appear() {
    for (text, section) in [
        ("[lang.java]\ntest = \"mvn test\"\n", "[lang]"),
        (
            "[roots]\nsvc = { path = \"svc\", lang = \"java\" }\n",
            "[roots.svc]",
        ),
        (
            "[checks.\"static.clones\".lang.java]\nthreshold = 1\n",
            "[checks.\"static.clones\".lang]",
        ),
    ] {
        let err = validate_languages(text).expect_err(text);
        assert_eq!(
            err.to_string(),
            format!("unknown language `java` in `{section}`; registered: `go`, `python`, `rust`")
        );
    }
}
