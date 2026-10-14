// ABOUTME: static.deps over synthetic module graphs: cycles, Martin's coupling metrics, layering rules,
// ABOUTME: settings validation, finding identity and shape, and the score (ADR-052, ADR-053).
use cupel_checks::register;
use cupel_core::config::ConfigError;
use cupel_core::syntax::{
    ApiItem, ApiItemKind, ApiSurface, ImportEdge, ImportTarget, ModuleGraph, RootImports, Span,
};
use cupel_core::{
    ActionKind, Cancellation, Check, CheckCtx, CheckId, CheckRegistry, CheckResult, CheckStatus,
    Error, Event, FactKind, Facts, Finding, FindingId, Plan, Scope, Score, Severity, Writer,
};
use serde_json::{Value, json};

const ID: &str = "static.deps";

fn file(path: &str) -> ImportTarget {
    ImportTarget::File {
        path: path.to_owned(),
    }
}

fn external(package: &str) -> ImportTarget {
    ImportTarget::External {
        package: package.to_owned(),
    }
}

/// A module graph and the public types each module declares.
#[derive(Default)]
struct World {
    edges: Vec<ImportEdge>,
    items: Vec<ApiItem>,
}

impl World {
    /// `from` imports `target` at `line`.
    fn import(mut self, from: &str, target: ImportTarget, line: u32) -> Self {
        let specifier = match &target {
            ImportTarget::File { path } => path.clone(),
            ImportTarget::External { package } => package.clone(),
            ImportTarget::Unresolved => "missing".to_owned(),
        };
        self.edges.push(ImportEdge {
            from: from.to_owned(),
            specifier,
            target,
            relative: false,
            line,
        });
        self
    }

    /// `from` imports the module `to` at `line`.
    fn edge(self, from: &str, to: &str, line: u32) -> Self {
        self.import(from, file(to), line)
    }

    /// `module` declares a public item of `kind`.
    fn declares(mut self, module: &str, kind: ApiItemKind) -> Self {
        self.items.push(ApiItem {
            path: format!("{module}::item{}", self.items.len()),
            kind,
            signature: String::new(),
            file: module.to_owned(),
            span: Span {
                line: 1,
                col: 1,
                end_line: 1,
                end_col: 2,
            },
        });
        self
    }

    fn facts(&self, check: &dyn Check) -> Facts {
        let mut facts = Facts::for_manifest(check.manifest());
        facts
            .insert(ModuleGraph {
                roots: vec![RootImports {
                    root: ".".to_owned(),
                    edges: self.edges.clone(),
                }],
            })
            .unwrap();
        facts
            .insert(ApiSurface {
                items: self.items.clone(),
            })
            .unwrap();
        facts
    }
}

fn registry() -> CheckRegistry {
    let mut registry = CheckRegistry::default();
    register(&mut registry);
    registry
}

fn ctx(config: Value) -> (CheckCtx, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let writer = Writer::new(dir.path(), false).unwrap();
    let Value::Object(config) = config else {
        panic!("config is a table")
    };
    let ctx = CheckCtx {
        scope: Scope::default(),
        config,
        writer: writer.scratch(&cupel_core::RunId::generate()),
        cancel: Cancellation::new(),
    };
    (ctx, dir)
}

/// Runs the check over `world` with `config`; its result and findings.
fn run(world: &World, config: Value) -> (CheckResult, Vec<Finding>) {
    let registry = registry();
    let check = registry
        .get(&CheckId::from(ID))
        .expect("static.deps is registered");
    let (ctx, _dir) = ctx(config);
    let plan = check.plan(&ctx, &ctx.scope).unwrap();
    let mut events = Vec::new();
    let result = check
        .run(&ctx, &world.facts(check), &plan, &mut events)
        .unwrap();
    let findings = events
        .into_iter()
        .filter_map(|event| match event {
            Event::Finding(finding) => Some(*finding),
            _ => None,
        })
        .collect();
    (result, findings)
}

fn of_kind<'a>(findings: &'a [Finding], kind: &str) -> Vec<&'a Finding> {
    findings.iter().filter(|f| f.kind == kind).collect()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap().to_owned())
        .collect()
}

fn cycle_score(value: f64, n: u64) -> Score {
    Score {
        value,
        unit: "cycles".to_owned(),
        n,
    }
}

fn layers() -> Value {
    json!({"layers": {
        "order": ["domain", "app", "infra"],
        "map": {"domain": "src/domain/**", "app": "src/app/**", "infra": "src/infra/**"},
    }})
}

#[test]
fn the_manifest_declares_its_facts_kinds_actions_and_metrics() {
    let registry = registry();
    let manifest = registry.get(&CheckId::from(ID)).unwrap().manifest();
    assert_eq!(
        manifest.needs,
        [FactKind::ModuleGraph, FactKind::ApiSurface]
    );
    assert!(manifest.wants.is_empty());
    assert_eq!(manifest.tier.value(), 0);
    assert_eq!(manifest.cost, cupel_core::CostClass::Quick);
    let kinds: Vec<(&str, ActionKind)> = manifest
        .findings
        .iter()
        .map(|spec| (spec.kind.as_str(), spec.action))
        .collect();
    assert_eq!(
        kinds,
        [
            ("dep.cycle", ActionKind::BreakCycle),
            ("dep.layer-violation", ActionKind::Review),
            ("dep.unstable-abstract", ActionKind::Review),
        ]
    );
    let metrics: Vec<&str> = manifest.metrics.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(metrics, ["cycles", "max_distance", "violations"]);
    for spec in &manifest.findings {
        assert_eq!(spec.evidence["type"], json!("object"), "{}", spec.kind);
    }
}

#[test]
fn an_acyclic_graph_has_no_cycle_and_a_zero_score() {
    let world = World::default()
        .edge("a", "b", 1)
        .edge("b", "c", 1)
        .edge("a", "c", 2);
    let (result, findings) = run(&world, json!({}));
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(result.status, CheckStatus::Pass);
    assert_eq!(
        result.score,
        Some(Score {
            value: 0.0,
            unit: "cycles".to_owned(),
            n: 3
        })
    );
}

#[test]
fn a_two_module_cycle_is_one_finding_identified_by_its_members() {
    let world = World::default().edge("b", "a", 4).edge("a", "b", 7);
    let (result, findings) = run(&world, json!({}));
    let [finding] = findings.as_slice() else {
        panic!("one finding expected: {findings:?}")
    };
    assert_eq!(finding.kind, "dep.cycle");
    let check = CheckId::from(ID);
    assert_eq!(
        finding.id,
        FindingId::for_subject(&check, "dep.cycle", "a\nb")
    );
    assert_eq!(finding.severity, Severity::Warn);
    assert!(finding.symbol.is_none());
    let location = finding.location.as_ref().unwrap();
    assert_eq!((location.file.as_str(), location.line), ("a", 7));
    assert_eq!(finding.action.kind, ActionKind::BreakCycle);
    assert_eq!(finding.repro, "cupel check --only static.deps");
    assert_eq!(
        finding.evidence.0,
        *json!({"cycle": ["a", "b"], "modules": ["a", "b"], "size": 2})
            .as_object()
            .unwrap()
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.findings, 1);
    assert_eq!(result.score, Some(cycle_score(1.0, 2)));
}

#[test]
fn nested_cycles_form_one_component_and_separate_cycles_stay_separate() {
    // a → b → c → a and c → d → c share c: one component of four. e ↔ f is another.
    let world = World::default()
        .edge("a", "b", 1)
        .edge("b", "c", 1)
        .edge("c", "a", 1)
        .edge("c", "d", 2)
        .edge("d", "c", 1)
        .edge("e", "f", 1)
        .edge("f", "e", 1)
        .edge("f", "a", 2);
    let (result, findings) = run(&world, json!({}));
    let cycles = of_kind(&findings, "dep.cycle");
    let members: Vec<Vec<String>> = cycles
        .iter()
        .map(|f| strings(&f.evidence.0["modules"]))
        .collect();
    assert_eq!(members, [vec!["a", "b", "c", "d"], vec!["e", "f"]]);
    assert_eq!(strings(&cycles[0].evidence.0["cycle"]), ["a", "b", "c"]);
    assert_eq!(cycles[0].evidence.0["size"], json!(4));
    assert_eq!(result.score, Some(cycle_score(2.0, 6)));
}

#[test]
fn the_reported_cycle_is_the_shortest_through_the_smallest_module() {
    // a → b → c → d → a, with the shortcut b → d.
    let world = World::default()
        .edge("a", "b", 1)
        .edge("b", "c", 1)
        .edge("c", "d", 1)
        .edge("d", "a", 1)
        .edge("b", "d", 2);
    let (_, findings) = run(&world, json!({}));
    assert_eq!(strings(&findings[0].evidence.0["cycle"]), ["a", "b", "d"]);
}

#[test]
fn self_imports_are_not_cycles() {
    let world = World::default().edge("a", "a", 3).edge("a", "b", 4);
    let (result, findings) = run(&world, json!({}));
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(result.score.unwrap().n, 2);
}

#[test]
fn an_abstract_module_nothing_imports_is_unstable_abstract() {
    // port declares only interfaces and imports two modules, one package, and one unresolved
    // path; nothing imports it: I = 1, A = 1, D = 1.
    let world = World::default()
        .declares("port", ApiItemKind::Interface)
        .edge("port", "model", 1)
        .edge("port", "other", 2)
        .import("port", external("serde"), 3)
        .import("port", external("serde"), 4)
        .import("port", ImportTarget::Unresolved, 5);
    let (_, findings) = run(&world, json!({}));
    let [finding] = findings.as_slice() else {
        panic!("one finding expected: {findings:?}")
    };
    assert_eq!(finding.kind, "dep.unstable-abstract");
    assert_eq!(
        finding.id,
        FindingId::for_subject(&CheckId::from(ID), "dep.unstable-abstract", "port")
    );
    assert_eq!(finding.action.kind, ActionKind::Review);
    let location = finding.location.as_ref().unwrap();
    assert_eq!((location.file.as_str(), location.line), ("port", 1));
    assert_eq!(
        finding.evidence.0,
        *json!({
            "module": "port",
            "afferent": 0,
            "efferent": 2,
            "external": 1,
            "unresolved": 1,
            "instability": 1.0,
            "abstractness": 1.0,
            "distance": 1.0,
            "distance_max": 0.7,
        })
        .as_object()
        .unwrap()
    );
}

#[test]
fn instability_abstractness_and_distance_follow_martin() {
    // m: Ca = 1 (u), Ce = 3 (x, y, z), so I = 0.75; one interface of two types, so A = 0.5;
    // D = |0.5 + 0.75 − 1| = 0.25.
    let world = World::default()
        .declares("m", ApiItemKind::Interface)
        .declares("m", ApiItemKind::Type)
        .declares("m", ApiItemKind::Function)
        .edge("u", "m", 1)
        .edge("m", "x", 1)
        .edge("m", "y", 2)
        .edge("m", "z", 3);
    assert!(run(&world, json!({})).1.is_empty());
    let (_, findings) = run(&world, json!({"distance_max": 0.2}));
    let [finding] = findings.as_slice() else {
        panic!("one finding expected: {findings:?}")
    };
    let evidence = &finding.evidence.0;
    assert_eq!(
        (
            &evidence["afferent"],
            &evidence["efferent"],
            &evidence["instability"],
            &evidence["abstractness"],
            &evidence["distance"],
        ),
        (
            &json!(1),
            &json!(3),
            &json!(0.75),
            &json!(0.5),
            &json!(0.25)
        )
    );
}

#[test]
fn stable_concrete_modules_and_modules_without_types_are_not_unstable_abstract() {
    // c: concrete and depended upon (zone of pain, D = 1, not this kind). f: unstable but
    // declares no type, so it has no abstractness. lone: an interface with no couplings.
    let world = World::default()
        .declares("c", ApiItemKind::Type)
        .edge("u", "c", 1)
        .edge("f", "c", 1)
        .declares("lone", ApiItemKind::Interface);
    let (_, findings) = run(&world, json!({"distance_max": 0.0}));
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn a_lower_layer_importing_a_higher_one_is_a_violation_reported_once() {
    let world = World::default()
        .edge("src/domain/order.x", "src/infra/db.x", 3)
        .edge("src/domain/order.x", "src/infra/db.x", 9)
        .edge("src/infra/db.x", "src/domain/model.x", 1)
        .edge("src/infra/db.x", "src/app/service.x", 2)
        .edge("src/app/service.x", "src/domain/model.x", 1)
        .edge("src/app/service.x", "lib/util.x", 2)
        .edge("lib/util.x", "src/domain/model.x", 1);
    let (result, findings) = run(&world, layers());
    let [finding] = findings.as_slice() else {
        panic!("one finding expected: {findings:?}")
    };
    assert_eq!(finding.kind, "dep.layer-violation");
    assert_eq!(
        finding.id,
        FindingId::for_subject(
            &CheckId::from(ID),
            "dep.layer-violation",
            "src/domain/order.x -> src/infra/db.x"
        )
    );
    let location = finding.location.as_ref().unwrap();
    assert_eq!(
        (location.file.as_str(), location.line),
        ("src/domain/order.x", 3)
    );
    assert_eq!(
        finding.evidence.0,
        *json!({
            "from": "src/domain/order.x",
            "to": "src/infra/db.x",
            "from_layer": "domain",
            "to_layer": "infra",
            "specifier": "src/infra/db.x",
            "order": ["domain", "app", "infra"],
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(result.status, CheckStatus::Fail);
}

#[test]
fn without_layers_nothing_is_a_violation() {
    let world = World::default().edge("src/domain/order.x", "src/infra/db.x", 3);
    assert!(run(&world, json!({})).1.is_empty());
}

#[test]
fn a_module_takes_the_first_layer_that_matches_and_star_stays_in_a_segment() {
    let config = json!({"layers": {
        "order": ["core", "edge"],
        "map": {"core": "src/*.x", "edge": "src/**"},
    }});
    // src/a.x is core (first match); src/deep/b.x is edge, since `*` does not cross `/`.
    let world =
        World::default()
            .edge("src/a.x", "src/deep/b.x", 1)
            .edge("src/deep/b.x", "src/c.x", 1);
    let (_, findings) = run(&world, config);
    let violations: Vec<(String, String)> = findings
        .iter()
        .map(|f| {
            (
                f.evidence.0["from"].as_str().unwrap().to_owned(),
                f.evidence.0["to"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        violations,
        [("src/a.x".to_owned(), "src/deep/b.x".to_owned())]
    );
}

#[test]
fn external_and_unresolved_imports_are_never_violations_or_coupling() {
    let world = World::default()
        .import("src/domain/a.x", external("src/infra/db.x"), 1)
        .import("src/domain/a.x", ImportTarget::Unresolved, 2);
    let (result, findings) = run(&world, layers());
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(result.score.unwrap().n, 1);
}

#[test]
fn malformed_settings_are_configuration_errors_naming_the_key() {
    let registry = registry();
    let check = registry.get(&CheckId::from(ID)).unwrap();
    for (config, key) in [
        (json!({"layer": {}}), "layer"),
        (json!({"distance_max": 1.5}), "distance_max"),
        (json!({"distance_max": "far"}), "distance_max"),
        (json!({"layers": []}), "layers"),
        (json!({"layers": {"order": ["a"]}}), "layers"),
        (
            json!({"layers": {"order": ["a"], "map": {"a": "*"}, "extra": 1}}),
            "layers",
        ),
        (json!({"layers": {"order": [], "map": {}}}), "layers.order"),
        (
            json!({"layers": {"order": ["a", "a"], "map": {"a": "*"}}}),
            "layers.order",
        ),
        (
            json!({"layers": {"order": ["a", 1], "map": {"a": "*"}}}),
            "layers.order",
        ),
        (
            json!({"layers": {"order": ["a", "b"], "map": {"a": "*"}}}),
            "layers.map.b",
        ),
        (
            json!({"layers": {"order": ["a"], "map": {"a": "*", "z": "*"}}}),
            "layers.map.z",
        ),
        (
            json!({"layers": {"order": ["a"], "map": {"a": "src/[a"}}}),
            "layers.map.a",
        ),
        (
            json!({"layers": {"order": ["a"], "map": {"a": 3}}}),
            "layers.map.a",
        ),
    ] {
        let (ctx, _dir) = ctx(config.clone());
        let error = check.plan(&ctx, &Scope::default()).unwrap_err();
        let Error::Config(ConfigError::CheckSetting {
            key: named, valid, ..
        }) = error
        else {
            panic!("expected a check-setting error for {config}, got {error:?}")
        };
        assert_eq!(named, key, "{config}");
        assert_eq!(valid, ["distance_max", "layers"], "{config}");
    }
}

#[test]
fn plans_count_the_files_in_scope_at_quick_cost() {
    let registry = registry();
    let check = registry.get(&CheckId::from(ID)).unwrap();
    let scope = Scope::of_files(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
    let (ctx, _dir) = ctx(json!({}));
    assert_eq!(
        check.plan(&ctx, &scope).unwrap(),
        Plan {
            targets: 3,
            est_ms: 3
        }
    );
}

#[test]
fn scores_sum_across_languages() {
    let registry = registry();
    let check = registry.get(&CheckId::from(ID)).unwrap();
    let score = |value: f64, n: u64| Score {
        value,
        unit: "cycles".to_owned(),
        n,
    };
    assert_eq!(
        check.combine(&[score(1.0, 10), score(2.0, 4)]),
        Some(score(3.0, 14))
    );
    assert_eq!(check.combine(&[]), None);
}
