// ABOUTME: Discovery (ADR-088): `[plugins]` entries and `cupel-check-*`/`cupel-produce-*` on PATH are described,
// ABOUTME: validated, and registered beside native checks and producers; every failure names the plugin and the fix.
#![cfg(unix)]

mod support;

use std::path::Path;

use cupel_core::config::ConfigError;
use cupel_core::{
    Cancellation, CheckId, CheckRegistry, Ctx, FactKind, Lang, LangFamily, LangId, ProducerRegistry,
};
use cupel_plugin::{Plugins, discover};
use serde_json::{Value, json};
use support::{check_manifest, discovery, entries, languages, plugin, producer_manifest, script};

fn describing(dir: &Path, name: &str, manifest: &Value) -> std::path::PathBuf {
    script(dir, name, &plugin(dir, manifest, "exit 1", "exit 1"))
}

fn discovered(root: &Path, plugins: &[(&str, &Path)]) -> Result<Plugins, ConfigError> {
    let entries = entries(plugins);
    let languages = languages();
    let cancel = Cancellation::new();
    discover(&discovery(root, &entries, &languages, &cancel))
}

fn problem(result: Result<Plugins, ConfigError>) -> (String, String, bool) {
    match result {
        Err(ConfigError::Plugin {
            name,
            problem,
            on_path,
            ..
        }) => (name, problem, on_path),
        Err(other) => panic!("not a plugin error: {other}"),
        Ok(_) => panic!("discovery should have failed"),
    }
}

fn typescript() -> Lang {
    Lang {
        id: LangId::from("typescript"),
        family: LangFamily::from("ecma"),
    }
}

#[test]
fn configured_plugins_register_as_checks_and_producers() {
    let dir = tempfile::tempdir().unwrap();
    let check = describing(dir.path(), "check.sh", &check_manifest());
    let producer = describing(dir.path(), "produce.sh", &producer_manifest());
    let plugins = discovered(dir.path(), &[("a-check", &check), ("b-produce", &producer)]).unwrap();

    let mut checks = CheckRegistry::default();
    plugins.register_checks(&mut checks);
    let ids: Vec<&str> = checks
        .iter()
        .map(|check| check.manifest().id.as_str())
        .collect();
    assert_eq!(ids, ["ref.sample"]);
    assert_eq!(plugins.check_ids(), [CheckId::from("ref.sample")]);

    let mut producers = ProducerRegistry::default();
    plugins.register_producers(&mut producers);
    let made: Vec<String> = producers
        .producers_for(FactKind::PerTestMap, &typescript())
        .map(|producer| producer.id().to_string())
        .collect();
    assert_eq!(made, ["plugin.sample"]);
    let python = Lang {
        id: LangId::from("python"),
        family: LangFamily::from("python"),
    };
    assert_eq!(
        producers
            .producers_for(FactKind::PerTestMap, &python)
            .count(),
        0
    );
}

#[test]
fn a_producer_plugin_is_ready_and_detected_at_its_manifest_cost() {
    let dir = tempfile::tempdir().unwrap();
    let producer = describing(dir.path(), "produce.sh", &producer_manifest());
    let plugins = discovered(dir.path(), &[("produce", &producer)]).unwrap();
    let mut producers = ProducerRegistry::default();
    plugins.register_producers(&mut producers);
    let ctx = Ctx::new(dir.path().to_path_buf());
    let registered = producers
        .producers_for(FactKind::PerTestMap, &typescript())
        .next()
        .unwrap();
    assert_eq!(
        (
            registered.availability(&ctx),
            registered.detected(&ctx),
            registered.cost(),
            registered.needs().len(),
        ),
        (
            cupel_core::Availability::Ready,
            true,
            cupel_core::CostClass::Quick,
            0
        )
    );
}

#[test]
fn plugins_on_path_are_found_by_name_in_path_order() {
    let root = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let mut other_check = check_manifest();
    other_check["id"] = json!("ref.other");
    describing(second.path(), "cupel-check-zeta", &other_check);
    // Shadowed by the first directory's file of the same name.
    describing(second.path(), "cupel-check-alpha", &json!({"broken": true}));
    describing(first.path(), "cupel-check-alpha", &check_manifest());
    describing(first.path(), "cupel-produce-map", &producer_manifest());
    describing(first.path(), "unrelated-tool", &json!({}));
    let not_executable = first.path().join("cupel-check-plain");
    std::fs::write(&not_executable, "#!/bin/sh\nexit 1\n").unwrap();

    let path = format!("{}:{}", first.path().display(), second.path().display());
    let entries = entries(&[]);
    let languages = languages();
    let cancel = Cancellation::new();
    let mut request = discovery(root.path(), &entries, &languages, &cancel);
    request.path = Some(&path);
    let plugins = discover(&request).unwrap();

    assert_eq!(
        plugins.check_ids(),
        [CheckId::from("ref.sample"), CheckId::from("ref.other")]
    );
    assert_eq!(
        plugins.names(),
        ["cupel-check-alpha", "cupel-produce-map", "cupel-check-zeta"]
    );
    request.path = None;
    assert!(discover(&request).unwrap().is_empty());
}

#[test]
fn a_path_plugin_that_a_config_entry_already_runs_is_described_once() {
    let root = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let check = describing(bin.path(), "cupel-check-sample", &check_manifest());
    let entries = entries(&[("sample", &check)]);
    let languages = languages();
    let cancel = Cancellation::new();
    let path = bin.path().display().to_string();
    let mut request = discovery(root.path(), &entries, &languages, &cancel);
    request.path = Some(&path);
    assert_eq!(discover(&request).unwrap().names(), ["sample"]);
}

#[test]
fn a_relative_program_runs_from_the_repository_root() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("tools")).unwrap();
    describing(&root.path().join("tools"), "check.sh", &check_manifest());
    let plugins = discovered(root.path(), &[("local", Path::new("tools/check.sh"))]).unwrap();
    assert_eq!(plugins.check_ids(), [CheckId::from("ref.sample")]);
}

#[test]
fn a_plugin_that_cannot_describe_itself_is_a_usage_error_naming_it() {
    let dir = tempfile::tempdir().unwrap();
    let failing = script(dir.path(), "fails.sh", "echo broken >&2; exit 3");
    let garbled = script(dir.path(), "garbled.sh", "echo 'not json'");
    let cases = [
        (
            discovered(dir.path(), &[("missing", Path::new("/no/such/plugin"))]),
            "missing",
            "cannot run `/no/such/plugin`",
        ),
        (
            discovered(dir.path(), &[("fails", &failing)]),
            "fails",
            "`describe` exited with code 3: broken",
        ),
        (
            discovered(dir.path(), &[("garbled", &garbled)]),
            "garbled",
            "`describe` did not print a manifest: expected ident at line 1 column 2",
        ),
    ];
    for (result, name, expected) in cases {
        let (named, problem, on_path) = problem(result);
        assert_eq!((named.as_str(), on_path), (name, false));
        assert!(problem.contains(expected), "{name}: {problem}");
    }
}

#[test]
fn manifests_that_cannot_be_registered_are_refused_with_the_reason() {
    let mut no_prefix = producer_manifest();
    no_prefix["id"] = json!("tool.sample");
    let mut old_version = producer_manifest();
    old_version["provides"]["version"] = json!(1);
    let mut no_payload = producer_manifest();
    no_payload["provides"] = json!({"kind": "bench-samples", "version": 1});
    let mut unknown_language = producer_manifest();
    unknown_language["languages"] = json!(["cobol"]);
    let mut taken = check_manifest();
    taken["id"] = json!("static.complexity");
    let mut bad_id = check_manifest();
    bad_id["id"] = json!("sample");
    let mut no_findings = check_manifest();
    no_findings["findings"] = json!([]);
    let cases = [
        (
            no_prefix,
            "producer ID `tool.sample` must start with `plugin.`",
        ),
        (
            old_version,
            "provides `per-test-map` version 1, but this Cupel reads version 2",
        ),
        (no_payload, "`bench-samples` facts have no payload type yet"),
        (
            unknown_language,
            "language `cobol` is not registered; registered: `python`, `typescript`",
        ),
        (taken, "check `static.complexity` is already registered"),
        (
            bad_id,
            "invalid check ID `sample`: check ID must be `<group>.<name>`",
        ),
        (
            no_findings,
            "a check plugin must declare at least one finding kind",
        ),
    ];
    let dir = tempfile::tempdir().unwrap();
    for (manifest, expected) in cases {
        let plugin = describing(dir.path(), "plugin.sh", &manifest);
        let (_, problem, _) = problem(discovered(dir.path(), &[("sample", &plugin)]));
        assert!(problem.contains(expected), "{expected}: {problem}");
    }
}

#[test]
fn two_plugins_with_one_id_and_a_name_that_says_the_wrong_kind_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let check = describing(dir.path(), "check.sh", &check_manifest());
    let (name, problem_text, _) = problem(discovered(dir.path(), &[("a", &check), ("b", &check)]));
    assert_eq!(name, "b");
    assert!(
        problem_text.contains("check `ref.sample` is already registered"),
        "{problem_text}"
    );

    let bin = tempfile::tempdir().unwrap();
    describing(bin.path(), "cupel-produce-sample", &check_manifest());
    let entries = entries(&[]);
    let languages = languages();
    let cancel = Cancellation::new();
    let path = bin.path().display().to_string();
    let mut request = discovery(dir.path(), &entries, &languages, &cancel);
    request.path = Some(&path);
    let (name, problem_text, on_path) = problem(discover(&request));
    assert_eq!((name.as_str(), on_path), ("cupel-produce-sample", true));
    assert!(
        problem_text.contains("a `cupel-produce-` plugin must describe a producer, not a check"),
        "{problem_text}"
    );
}
