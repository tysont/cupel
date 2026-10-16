// ABOUTME: Facts as they serialize, read by kind without knowing their type: what a check's `Facts` and a
// ABOUTME: producer's `Upstream` hand a plugin as JSON files (ADR-089 §3).
use cupel_core::{
    CoveredLines, FactKind, Facts, PerTestMap, Produced, ProducerId, TestCoverage, Upstream,
};
use serde_json::json;

fn map() -> PerTestMap {
    PerTestMap::new(
        ProducerId::from("tool.vitest"),
        vec![TestCoverage::new(
            "adds",
            None,
            vec![CoveredLines {
                file: "src/a.ts".to_owned(),
                lines: vec![2, 1],
            }],
        )],
    )
}

fn expected() -> serde_json::Value {
    json!({
        "runner": "tool.vitest",
        "tests": [{"id": "adds", "file": null, "covered": [{"file": "src/a.ts", "lines": [1, 2]}]}],
        "measured": 1,
        "reused": 0,
    })
}

#[test]
fn facts_serve_each_declared_kind_as_json_whether_inserted_typed_or_shared() {
    let mut typed = Facts::declaring([FactKind::PerTestMap, FactKind::Symbols]);
    typed.insert(map()).unwrap();
    let mut shared = Facts::declaring([FactKind::PerTestMap]);
    shared
        .insert_shared(&Produced::new(map()).share())
        .unwrap();

    assert_eq!(typed.json(FactKind::PerTestMap), Some(expected()));
    assert_eq!(shared.json(FactKind::PerTestMap), Some(expected()));
    // Declared but not produced, and not declared at all.
    assert_eq!(typed.json(FactKind::Symbols), None);
    assert_eq!(typed.json(FactKind::CoverageMap), None);
    assert_eq!(typed.get::<PerTestMap>().unwrap(), &map());
}

#[test]
fn upstream_serves_each_made_fact_as_json() {
    let mut upstream = Upstream::default();
    upstream.insert(Produced::new(map()).share());
    assert_eq!(upstream.json(FactKind::PerTestMap), Some(expected()));
    assert_eq!(upstream.json(FactKind::TestResults), None);
    assert_eq!(upstream.kinds().collect::<Vec<_>>(), [FactKind::PerTestMap]);
}
