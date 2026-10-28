// ABOUTME: A producer plugin run as a `Producer` (ADR-089): its request and upstream fact files, the fact it
// ABOUTME: emits validated against the provided kind's type and version, and every violation refused as an error.
#![cfg(unix)]

mod support;

use std::path::Path;

use cupel_core::{
    Cancellation, CostClass, Ctx, Error, FactKind, PerTestMap, ProduceCtx, Produced, ProducerId,
    ProducerRegistry, Scope, TestResults, Writer,
};
use serde_json::{Value, json};
use support::{
    discovery, entries, languages, plugin, prints, producer_manifest, script, typescript,
};

/// Registers the producer plugin `manifest` in `dir`, running `run`, and produces with it.
fn produce_with(dir: &Path, manifest: &Value, run: &str) -> cupel_core::Result<Produced> {
    let program = script(dir, "produce.sh", &plugin(dir, manifest, "exit 1", run));
    let entries = entries(&[("sample", &program)]);
    let languages = languages();
    let cancel = Cancellation::new();
    let plugins = cupel_plugin::discover(&discovery(dir, &entries, &languages, &cancel)).unwrap();
    let mut producers = ProducerRegistry::default();
    plugins.register_producers(&mut producers);
    let kind = manifest["provides"]["kind"].as_str().unwrap();
    let kind = FactKind::all()
        .iter()
        .copied()
        .find(|candidate| candidate.as_str() == kind)
        .unwrap();
    let producer = producers.producers_for(kind, &typescript()).next().unwrap();
    let mut ctx = ProduceCtx::new(
        Ctx::new(dir.to_path_buf()),
        Writer::new(dir, false).unwrap().confined(),
        Cancellation::new(),
    );
    ctx.seed = 9;
    ctx.focus.depth = CostClass::Deep;
    ctx.config = serde_json::Map::from_iter([("per_test_lcov".to_owned(), json!("cov.lcov"))]);
    ctx.upstream.insert(Produced::new(results()).share());
    producer.produce(
        &ctx,
        &Scope::of_files(vec!["src/a.ts".to_owned()]),
        &mut Vec::new(),
    )
}

fn results() -> TestResults {
    TestResults {
        runner: ProducerId::from("tool.vitest"),
        invocations: Vec::new(),
        tests: Vec::new(),
        suite_hash: "0".to_owned(),
        reuse: None,
    }
}

fn map() -> Value {
    json!({
        "runner": "tool.vitest",
        "tests": [{"id": "adds", "file": null, "covered": [{"file": "src/a.ts", "lines": [1, 2]}]}],
        "measured": 1,
        "reused": 0
    })
}

fn fact(version: u32, payload: &Value) -> Value {
    json!({"ev": "fact", "kind": "per-test-map", "version": version, "payload": payload})
}

fn problem(made: cupel_core::Result<Produced>) -> String {
    match made {
        Err(Error::Plugin { problem, .. }) => problem,
        other => panic!("expected a plugin error, got {other:?}"),
    }
}

#[test]
fn the_fact_a_producer_plugin_prints_is_the_typed_fact() {
    let dir = tempfile::tempdir().unwrap();
    let made = produce_with(
        dir.path(),
        &producer_manifest(),
        &prints(&[fact(2, &map())]),
    )
    .unwrap();
    let map: &PerTestMap = made.downcast_ref().unwrap();
    assert_eq!(map.runner, ProducerId::from("tool.vitest"));
    assert_eq!(map.tests[0].covered[0].lines, [1, 2]);
    assert_eq!(made.to_json(), self::map());
}

#[test]
fn the_request_carries_the_language_table_seed_depth_and_upstream_facts() {
    let dir = tempfile::tempdir().unwrap();
    let mut manifest = producer_manifest();
    manifest["needs"] = json!(["test-results"]);
    produce_with(dir.path(), &manifest, &prints(&[fact(2, &map())])).unwrap();
    let request: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("request.json")).unwrap())
            .unwrap();
    assert_eq!(
        (&request["seed"], &request["depth"], &request["config"]),
        (
            &json!(9),
            &json!("deep"),
            &json!({"per_test_lcov": "cov.lcov"})
        )
    );
    assert_eq!(request["scope"]["files"], json!(["src/a.ts"]));
    let file = request["facts"]["test-results"].as_str().unwrap();
    let written: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    assert_eq!(written, serde_json::to_value(results()).unwrap());
    assert!(
        request["scratch"]
            .as_str()
            .unwrap()
            .ends_with(".cupel/work/plugins/plugin.sample")
    );
}

#[test]
fn facts_outside_the_declared_kind_version_or_shape_are_refused() {
    let wrong_kind = json!({"ev": "fact", "kind": "coverage-map", "version": 2, "payload": map()});
    let cases: [(Vec<Value>, &str); 6] = [
        (
            vec![fact(1, &map())],
            "a `per-test-map` fact of version 1, not 2",
        ),
        (
            vec![wrong_kind],
            "a `coverage-map` fact, not `per-test-map`",
        ),
        (
            vec![fact(2, &json!({"tests": []}))],
            "the fact does not match the `per-test-map` schema: missing field `runner`",
        ),
        (vec![fact(2, &map()), fact(2, &map())], "more than one fact"),
        (
            vec![
                json!({"ev": "result", "status": "pass", "score": null, "metrics": {}, "notes": []}),
            ],
            "a producer plugin printed a `result` event",
        ),
        (vec![], "no fact"),
    ];
    let (outputs, expected): (Vec<Vec<Value>>, Vec<&str>) = cases.into_iter().unzip();
    let problems: Vec<String> = outputs
        .iter()
        .map(|lines| {
            let dir = tempfile::tempdir().unwrap();
            problem(produce_with(
                dir.path(),
                &producer_manifest(),
                &prints(lines),
            ))
        })
        .collect();
    assert_eq!(problems, expected);
}

#[test]
fn a_non_zero_exit_without_a_fact_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let problem = problem(produce_with(
        dir.path(),
        &producer_manifest(),
        "echo 'no lcov file' >&2; exit 4",
    ));
    assert_eq!(problem, "exited with code 4 without a fact: no lcov file");
}

#[test]
fn every_catalog_kind_can_be_provided_by_a_plugin() {
    assert_eq!(cupel_plugin::providable_kinds(), FactKind::all());
}
