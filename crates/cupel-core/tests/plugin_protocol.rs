// ABOUTME: The plugin protocol's wire types (design §8.6, ADR-087, ADR-089): their JSON shapes as plugins
// ABOUTME: print and read them, and property tests that requests, events, and manifests round-trip.
use std::collections::BTreeMap;

use cupel_core::plugin::{
    PROTOCOL_VERSION, PluginEvent, PluginFact, PluginManifest, PluginResult, ProducerManifest,
    Provides, RequestScope, RunRequest, Toolchain,
};
use cupel_core::syntax::SymbolKey;
use cupel_core::{
    CheckId, CheckStatus, CostClass, FactKind, LangId, Manifest, ProducerId, Score, SettingSpec,
    Stability, Tier,
};
use proptest::prelude::*;
use serde_json::{Map, Value, json};

fn check_manifest() -> Manifest {
    Manifest {
        id: CheckId::from("ref.untested"),
        version: 1,
        tier: Tier::try_from(2).unwrap(),
        cost: CostClass::Quick,
        needs: vec![FactKind::PerTestMap],
        wants: Vec::new(),
        findings: Vec::new(),
        metrics: Vec::new(),
        settings: Vec::new(),
        stability: Stability::Experimental,
        summary: "Files no test executes.".to_owned(),
    }
}

#[test]
fn a_check_manifest_is_the_native_manifest_tagged_check() {
    let json = serde_json::to_value(PluginManifest::Check(check_manifest())).unwrap();
    assert_eq!(json["kind"], "check");
    assert_eq!(json["id"], "ref.untested");
    assert_eq!(json["needs"], json!(["per-test-map"]));
    let back: PluginManifest = serde_json::from_value(json).unwrap();
    assert_eq!(back, PluginManifest::Check(check_manifest()));
}

#[test]
fn a_check_manifest_without_settings_leaves_them_out() {
    let json = serde_json::to_value(check_manifest()).unwrap();
    assert_eq!(json.get("settings"), None);
    let manifest: Manifest = serde_json::from_value(json).unwrap();
    assert_eq!(manifest.settings, Vec::<SettingSpec>::new());
}

#[test]
fn a_setting_is_its_name_default_and_summary() {
    let manifest = Manifest {
        settings: vec![SettingSpec {
            name: "files_max".to_owned(),
            default: json!(3),
            summary: "The most untested files.".to_owned(),
        }],
        ..check_manifest()
    };
    let json = serde_json::to_value(&manifest).unwrap();
    assert_eq!(
        json["settings"],
        json!([{"name": "files_max", "default": 3, "summary": "The most untested files."}])
    );
    assert_eq!(serde_json::from_value::<Manifest>(json).unwrap(), manifest);
}

#[test]
fn a_producer_manifest_names_the_kind_and_version_it_provides() {
    let line = r#"{"kind": "producer", "id": "plugin.ref", "version": 1,
        "provides": {"kind": "per-test-map", "version": 2}, "languages": ["typescript"],
        "needs": [], "wants": [], "cost": "quick", "stability": "experimental",
        "summary": "Per-test map from LCOV."}"#;
    let PluginManifest::Producer(manifest) = serde_json::from_str(line).unwrap() else {
        panic!("a producer manifest")
    };
    assert_eq!(manifest.id, ProducerId::from("plugin.ref"));
    assert_eq!(
        manifest.provides,
        Provides {
            kind: FactKind::PerTestMap,
            version: 2
        }
    );
    assert_eq!(manifest.languages, [LangId::from("typescript")]);
}

#[test]
fn a_manifest_of_an_unknown_kind_is_refused() {
    let error = serde_json::from_str::<PluginManifest>(r#"{"kind": "language"}"#).unwrap_err();
    assert_eq!(
        error.to_string(),
        "unknown variant `language`, expected `check` or `producer` at line 1 column 19"
    );
}

#[test]
fn a_run_request_carries_every_key_of_design_8_6() {
    let request = RunRequest {
        protocol: PROTOCOL_VERSION,
        root: "/repo".to_owned(),
        scope: RequestScope {
            files: vec!["src/a.ts".to_owned()],
            symbols: None,
        },
        depth: Some(CostClass::Standard),
        budget_ms: None,
        seed: Some(3),
        config: Map::new(),
        facts: BTreeMap::from([(FactKind::PerTestMap, "/repo/.cupel/f.json".to_owned())]),
        scratch: "/repo/.cupel/work".to_owned(),
        toolchain: Toolchain {
            cupel: "0.1.0".to_owned(),
        },
        write: false,
    };
    let json = serde_json::to_value(&request).unwrap();
    let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
    // serde_json orders keys by name.
    assert_eq!(
        keys,
        [
            "budget_ms",
            "config",
            "depth",
            "facts",
            "protocol",
            "root",
            "scope",
            "scratch",
            "seed",
            "toolchain",
            "write"
        ]
    );
    assert_eq!(PROTOCOL_VERSION, 1);
    assert_eq!(
        json["facts"],
        json!({"per-test-map": "/repo/.cupel/f.json"})
    );
    assert_eq!(
        json["scope"],
        json!({"files": ["src/a.ts"], "symbols": null})
    );
}

#[test]
fn result_and_fact_events_parse_from_the_lines_a_plugin_prints() {
    let result: PluginEvent = serde_json::from_str(
        r#"{"ev": "result", "status": "fail", "score": {"value": 0.5, "unit": "files", "n": 2},
            "metrics": {"untested": 1}, "notes": []}"#,
    )
    .unwrap();
    assert_eq!(
        result,
        PluginEvent::Result(PluginResult {
            status: CheckStatus::Fail,
            score: Some(Score {
                value: 0.5,
                unit: "files".to_owned(),
                n: 2
            }),
            metrics: BTreeMap::from([("untested".to_owned(), 1.0)]),
            notes: Vec::new(),
        })
    );
    let fact: PluginEvent = serde_json::from_str(
        r#"{"ev": "fact", "kind": "per-test-map", "version": 2, "payload": {"tests": []}}"#,
    )
    .unwrap();
    assert_eq!(
        fact,
        PluginEvent::Fact(PluginFact {
            kind: FactKind::PerTestMap,
            version: 2,
            payload: json!({"tests": []}),
        })
    );
}

#[test]
fn an_event_outside_the_protocol_is_refused() {
    let errors: Vec<String> = [
        r#"{"ev": "run.end", "status": "pass", "exit_code": 0}"#,
        r#"{"ev": "result", "status": "fail"}"#,
        r#"{"status": "pass"}"#,
    ]
    .iter()
    .map(|line| {
        serde_json::from_str::<PluginEvent>(line)
            .unwrap_err()
            .to_string()
    })
    .collect();
    assert_eq!(
        errors,
        [
            "unknown variant `run.end`, expected one of `finding`, `progress`, `result`, `fact` at line 1 column 16",
            "missing field `metrics`",
            "missing field `ev` at line 1 column 18",
        ]
    );
}

/// Every status a plugin's result may report.
const STATUSES: [CheckStatus; 4] = [
    CheckStatus::Pass,
    CheckStatus::Fail,
    CheckStatus::Partial,
    CheckStatus::Error,
];

fn fact_kind() -> impl Strategy<Value = FactKind> {
    proptest::sample::select(FactKind::all().to_vec())
}

fn cost() -> impl Strategy<Value = CostClass> {
    proptest::sample::select(
        &[
            CostClass::Quick,
            CostClass::Standard,
            CostClass::Deep,
            CostClass::Exhaustive,
        ][..],
    )
}

fn text() -> impl Strategy<Value = String> {
    "[a-z0-9./ _-]{0,12}"
}

fn symbol_key() -> impl Strategy<Value = SymbolKey> {
    (text(), 1u32..500, 1u32..80).prop_map(|(file, line, col)| SymbolKey { file, line, col })
}

fn request() -> impl Strategy<Value = RunRequest> {
    (
        (text(), proptest::collection::vec(text(), 0..4)),
        proptest::option::of(proptest::collection::vec(symbol_key(), 0..3)),
        (proptest::option::of(cost()), any::<Option<u64>>()),
        any::<Option<u64>>(),
        proptest::collection::btree_map(fact_kind(), text(), 0..3),
        proptest::collection::btree_map(text(), any::<i64>(), 0..3),
        any::<bool>(),
    )
        .prop_map(
            |((root, files), symbols, (depth, budget_ms), seed, facts, config, write)| RunRequest {
                protocol: PROTOCOL_VERSION,
                scratch: format!("{root}/.cupel/work"),
                root,
                scope: RequestScope { files, symbols },
                depth,
                budget_ms,
                seed,
                config: config
                    .into_iter()
                    .map(|(key, value)| (key, Value::from(value)))
                    .collect(),
                facts,
                toolchain: Toolchain {
                    cupel: "0.1.0".to_owned(),
                },
                write,
            },
        )
}

fn event() -> impl Strategy<Value = PluginEvent> {
    prop_oneof![
        (
            proptest::sample::select(&STATUSES[..]),
            proptest::option::of((0u32..=1000, text(), any::<u32>())),
            proptest::collection::btree_map(text(), 0u32..1000, 0..3),
            proptest::collection::vec(text(), 0..3),
        )
            .prop_map(|(status, score, metrics, notes)| {
                PluginEvent::Result(PluginResult {
                    status,
                    score: score.map(|(value, unit, n)| Score {
                        value: f64::from(value) / 1000.0,
                        unit,
                        n: u64::from(n),
                    }),
                    metrics: metrics
                        .into_iter()
                        .map(|(name, value)| (name, f64::from(value)))
                        .collect(),
                    notes,
                })
            }),
        (
            fact_kind(),
            1u32..5,
            proptest::collection::vec(text(), 0..4)
        )
            .prop_map(|(kind, version, items)| PluginEvent::Fact(PluginFact {
                kind,
                version,
                payload: json!({ "items": items }),
            })),
        (
            "[a-z]{1,6}\\.[a-z-]{1,8}",
            any::<u64>(),
            any::<u64>(),
            any::<Option<u64>>()
        )
            .prop_map(|(id, done, total, eta_ms)| PluginEvent::Progress {
                id: CheckId::from(id),
                done,
                total,
                eta_ms,
            }),
    ]
}

fn producer_manifest() -> impl Strategy<Value = ProducerManifest> {
    (
        "plugin\\.[a-z-]{1,10}",
        (1u32..9, fact_kind(), 1u32..5),
        proptest::collection::vec("[a-z]{2,10}", 1..4),
        (
            proptest::collection::vec(fact_kind(), 0..3),
            proptest::collection::vec(fact_kind(), 0..3),
        ),
        cost(),
        text(),
    )
        .prop_map(
            |(id, (version, kind, fact_version), languages, (needs, wants), cost, summary)| {
                ProducerManifest {
                    id: ProducerId::from(id),
                    version,
                    provides: Provides {
                        kind,
                        version: fact_version,
                    },
                    languages: languages.into_iter().map(LangId::from).collect(),
                    needs,
                    wants,
                    cost,
                    stability: Stability::Experimental,
                    summary,
                }
            },
        )
}

proptest! {
    #[test]
    fn a_run_request_round_trips_through_json(request in request()) {
        let line = serde_json::to_string(&request).unwrap();
        prop_assert_eq!(serde_json::from_str::<RunRequest>(&line).unwrap(), request);
    }

    #[test]
    fn an_event_round_trips_through_one_json_line(event in event()) {
        let line = serde_json::to_string(&event).unwrap();
        prop_assert!(!line.contains('\n'));
        prop_assert_eq!(serde_json::from_str::<PluginEvent>(&line).unwrap(), event);
    }

    #[test]
    fn a_producer_manifest_round_trips_through_json(manifest in producer_manifest()) {
        let manifest = PluginManifest::Producer(manifest);
        let line = serde_json::to_string(&manifest).unwrap();
        prop_assert_eq!(serde_json::from_str::<PluginManifest>(&line).unwrap(), manifest);
    }
}
