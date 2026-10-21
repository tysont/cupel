// ABOUTME: static.dead-code over synthetic facts: modules from namespaces, entry by import, roots, symbols
// ABOUTME: nothing references, the finding kinds' confidence and evidence, scope, and the score (ADR-058/059/135).
mod support;

use std::collections::BTreeSet;

use cupel_core::config::ConfigError;
use cupel_core::syntax::{
    DynamicAccess, DynamicReach, EdgeKind, ImportEdge, ImportTarget, LiveRoot, ModuleGraph,
    NormNode, NormTree, NormalizedSymbol, Reference, ReferenceKind, ReferenceTarget, References,
    RootImports, RootReason, Roots, Span, SymbolDef, SymbolKind, Symbols, Visibility,
};
use cupel_core::{
    ActionKind, Check, CheckId, CheckResult, CheckStatus, CostClass, Error, FactKind, Facts,
    Finding, FindingId, LangId, MetricPer, Plan, Scope, Score, Severity, SymbolHash,
};
use serde_json::{Map, Value, json};
use support::{combined, language};

const ID: &str = "static.dead-code";

/// A symbol `name` in `file` on lines 1 to 3, in `namespace` with `separator`.
fn symbol(
    file: &str,
    namespace: &str,
    separator: &str,
    name: &str,
    visibility: Visibility,
) -> SymbolDef {
    SymbolDef {
        name: name.to_owned(),
        namespace: namespace.to_owned(),
        display: if namespace.is_empty() {
            name.to_owned()
        } else {
            format!("{namespace}{separator}{name}")
        },
        kind: SymbolKind::Function,
        visibility,
        lang: LangId::from("x"),
        file: file.to_owned(),
        span: Span {
            line: 1,
            col: 1,
            end_line: 3,
            end_col: 2,
        },
        body: None,
        parameters: 0,
    }
}

/// `def` moved to start at `line`, still spanning three lines.
fn at(mut def: SymbolDef, line: u32) -> SymbolDef {
    def.span.line = line;
    def.span.end_line = line + 2;
    def
}

fn private(file: &str, namespace: &str, name: &str, line: u32) -> SymbolDef {
    at(
        symbol(file, namespace, "::", name, Visibility::Private),
        line,
    )
}

fn public(file: &str, namespace: &str, name: &str, line: u32) -> SymbolDef {
    at(
        symbol(file, namespace, "::", name, Visibility::Public),
        line,
    )
}

fn method(mut def: SymbolDef) -> SymbolDef {
    def.kind = SymbolKind::Method;
    def
}

fn edge(from: &str, specifier: &str, target: ImportTarget) -> ImportEdge {
    ImportEdge {
        names: Vec::new(),
        aliases: Vec::new(),
        from: from.to_owned(),
        specifier: specifier.to_owned(),
        target,
        relative: false,
        line: 1,
        edge_kind: EdgeKind::Import,
    }
}

/// A reference to `name` on `line` of `file`, resolved to `target`.
fn reference(file: &str, line: u32, name: &str, target: ReferenceTarget) -> Reference {
    Reference {
        file: file.to_owned(),
        span: Span {
            line,
            col: 5,
            end_line: line,
            end_col: 9,
        },
        name: name.to_owned(),
        kind: ReferenceKind::Name,
        from: None,
        target,
    }
}

/// An unresolved reference to `name` on `line` of `file`.
fn named(file: &str, line: u32, name: &str) -> Reference {
    reference(file, line, name, ReferenceTarget::Unresolved)
}

/// A dynamic access on `line` of `file`.
fn dynamic(file: &str, line: u32, reach: DynamicReach) -> DynamicAccess {
    DynamicAccess {
        file: file.to_owned(),
        span: Span {
            line,
            col: 1,
            end_line: line,
            end_col: 20,
        },
        from: None,
        reach,
        text: "lookup(name)".to_owned(),
    }
}

/// A hash unique to the symbol's position.
fn hash_of(def: &SymbolDef) -> SymbolHash {
    let mut digest = 0u64;
    for byte in def.file.bytes() {
        digest = digest.wrapping_mul(31).wrapping_add(u64::from(byte));
    }
    digest = digest
        .wrapping_mul(1_000)
        .wrapping_add(u64::from(def.span.line));
    format!("sh_{digest:016x}").parse().unwrap()
}

/// The facts the check needs: symbols, their roots by reason, import edges, and references.
#[derive(Default)]
struct World {
    symbols: Vec<SymbolDef>,
    roots: Vec<(usize, RootReason)>,
    edges: Vec<ImportEdge>,
    references: References,
}

impl World {
    fn with(symbols: Vec<SymbolDef>) -> Self {
        let roots = symbols
            .iter()
            .enumerate()
            .filter(|(_, def)| def.visibility == Visibility::Public)
            .map(|(index, _)| (index, RootReason::Export))
            .collect();
        Self {
            symbols,
            roots,
            edges: Vec::new(),
            references: References::default(),
        }
    }

    fn refer(mut self, reference: Reference) -> Self {
        self.references.references.push(reference);
        self
    }

    fn root(mut self, index: usize, reason: RootReason) -> Self {
        self.roots.push((index, reason));
        self
    }

    fn import(mut self, edge: ImportEdge) -> Self {
        self.edges.push(edge);
        self
    }

    fn facts(&self, check: &dyn Check) -> Facts {
        let mut facts = Facts::for_manifest(check.manifest());
        facts
            .insert(Symbols {
                symbols: self.symbols.clone(),
            })
            .unwrap();
        facts
            .insert(NormTree {
                symbols: self
                    .symbols
                    .iter()
                    .map(|def| NormalizedSymbol {
                        symbol: def.key(),
                        hash: hash_of(def),
                        root: NormNode {
                            span: None,
                            text: None,
                            label: "fn".to_owned(),
                            children: Vec::new(),
                        },
                    })
                    .collect(),
            })
            .unwrap();
        facts
            .insert(ModuleGraph {
                roots: vec![RootImports {
                    root: ".".to_owned(),
                    edges: self.edges.clone(),
                }],
            })
            .unwrap();
        facts
            .insert(Roots {
                roots: self
                    .roots
                    .iter()
                    .map(|(index, reason)| LiveRoot {
                        symbol: self.symbols[*index].key(),
                        reason: *reason,
                    })
                    .collect(),
            })
            .unwrap();
        facts.insert(self.references.clone()).unwrap();
        facts
    }

    fn scope(&self) -> Scope {
        Scope {
            files: self.symbols.iter().map(|def| def.file.clone()).collect(),
            symbols: Some(self.symbols.iter().map(SymbolDef::key).collect()),
        }
    }
}

fn run(world: &World) -> (CheckResult, Vec<Finding>) {
    run_in(world, world.scope())
}

fn run_in(world: &World, scope: Scope) -> (CheckResult, Vec<Finding>) {
    let check = support::check(ID);
    let (result, recorded) =
        support::recorded(check, &world.facts(check), scope, json!({})).unwrap();
    (result, recorded.findings)
}

/// Each finding as (symbol display, kind).

#[test]
fn the_manifest_states_its_known_limitation() {
    let manifest = support::check("static.dead-code").manifest();
    let limitation = manifest
        .summary
        .split_once("Limitation: ")
        .map(|(_, rest)| rest);
    assert_eq!(
        limitation,
        Some(
            "references match by name, not by type, so any same-named reference keeps a symbol \
             alive, and a function only dead code calls is reported once that code is gone; an \
             export of an unimported module is only `dead.export` (info, low confidence) \
             (ADR-058, ADR-135)."
        )
    );
}

#[test]
fn the_manifest_declares_its_facts_kinds_actions_and_metrics() {
    let manifest = support::check(ID).manifest();
    assert_eq!(
        support::Shape::of(manifest),
        support::Shape {
            needs: vec![
                FactKind::Symbols,
                FactKind::NormTree,
                FactKind::ModuleGraph,
                FactKind::Roots,
                FactKind::References
            ],
            wants: vec![],
            tier: 0,
            cost: CostClass::Quick,
            kinds: vec![
                ("dead.symbol", ActionKind::Review),
                ("dead.export", ActionKind::Review)
            ],
            metrics: vec![
                ("dead_symbols", MetricPer::Scope),
                ("dead_exports", MetricPer::Scope)
            ],
            evidence: vec!["object"; 2],
        }
    );
}

#[test]
fn of_a_root_an_export_an_imported_item_and_an_unreferenced_private_only_the_last_is_dead() {
    let world = World::with(vec![
        private("main.x", "", "main", 1),
        private("main.x", "", "assist", 5),
        public("api.x", "api", "open", 1),
        private("api.x", "api", "check", 5),
        symbol("util.x", "util", "::", "helper", Visibility::Internal),
        private("orphan.x", "orphan", "forgotten", 1),
    ])
    .root(0, RootReason::EntryPoint)
    .import(edge("main.x", "api::open", support::imported_file("api.x")))
    .import(edge(
        "main.x",
        "util::helper",
        support::imported_file("util.x"),
    ))
    .refer(named("main.x", 2, "assist"))
    .refer(named("api.x", 2, "check"))
    .refer(named("main.x", 2, "helper"));
    let (result, findings) = run(&world);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("orphan::forgotten", "dead.symbol")])
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.findings, 1);
}

#[test]
fn a_finding_is_identified_located_explained_and_reproducible() {
    let world = World::with(vec![private("lost.x", "lost", "forgotten", 4)]);
    let (_, findings) = run(&world);
    let [finding] = findings.as_slice() else {
        panic!("one finding expected: {findings:?}")
    };
    let hash = hash_of(&world.symbols[0]);
    let check = CheckId::from(ID);
    assert_eq!(
        finding.id,
        FindingId::compute(&check, "dead.symbol", &hash, None)
    );
    assert_eq!(finding.severity, Severity::Warn);
    assert!((finding.confidence - 0.8).abs() < f64::EPSILON);
    assert_eq!(finding.tier.value(), 0);
    assert_eq!(finding.symbol.as_ref().unwrap().hash, hash);
    let location = finding.location.as_ref().unwrap();
    assert_eq!(
        (location.file.as_str(), location.line, location.end_line),
        ("lost.x", 4, 6)
    );
    assert_eq!(finding.action.kind, ActionKind::Review);
    assert!(finding.action.summary.contains("`forgotten`"));
    assert!(finding.action.summary.contains("lost.x"));
    let detail = finding.detail.as_deref().unwrap();
    assert!(
        detail.contains("No import in the repository resolves to lost.x"),
        "{detail}"
    );
    assert!(
        detail.contains("calls"),
        "the detail names what is invisible"
    );
    assert_eq!(
        finding.repro,
        format!("cupel check --only static.dead-code --symbols {hash}")
    );
    assert_eq!(
        finding.evidence.0,
        *json!({
            "file": "lost.x",
            "module": "lost",
            "module_files": ["lost.x"],
            "suspect_imports": [],
            "reach": "module",
            "dynamic": [],
        })
        .as_object()
        .unwrap()
    );
}

#[test]
fn an_unimported_export_is_an_info_finding_and_its_private_neighbors_are_kept() {
    let world = World::with(vec![
        public("api.x", "api", "open", 1),
        private("api.x", "api", "check", 5),
    ])
    .refer(named("api.x", 2, "check"));
    let (result, findings) = run(&world);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("api::open", "dead.export")])
    );
    let finding = &findings[0];
    assert_eq!(finding.severity, Severity::Info);
    assert!((finding.confidence - 0.4).abs() < f64::EPSILON);
    assert!(
        finding.detail.as_deref().unwrap().contains("library"),
        "the detail says a library's API looks the same"
    );
    assert_eq!(result.status, CheckStatus::Pass, "info does not fail");
    assert_eq!(
        result.score,
        Some(Score {
            value: 0.0,
            unit: "dead_symbols".to_owned(),
            n: 2
        })
    );
}

#[test]
fn files_sharing_a_module_namespace_keep_each_other_alive() {
    // A package whose files share a namespace: one holds the entry point, one an unimported
    // helper, one only a method whose namespace is the package plus its receiver.
    let world = World::with(vec![
        symbol("cmd/a.x", "main", ".", "main", Visibility::Internal),
        symbol("cmd/b.x", "main", ".", "helper", Visibility::Internal),
        method(symbol(
            "cmd/c.x",
            "main.server",
            ".",
            "serve",
            Visibility::Internal,
        )),
        symbol("other/d.x", "other", ".", "stray", Visibility::Internal),
    ])
    .root(0, RootReason::EntryPoint)
    .refer(named("cmd/a.x", 2, "helper"))
    .refer(named("cmd/a.x", 2, "serve"));
    let (_, findings) = run(&world);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("other.stray", "dead.symbol")])
    );
    assert_eq!(findings[0].evidence.0["module_files"], json!(["other/d.x"]));
}

#[test]
fn every_file_of_an_unreached_module_is_listed_in_evidence() {
    let world = World::with(vec![
        symbol("pkg/a.x", "pkg", ".", "one", Visibility::Internal),
        symbol("pkg/b.x", "pkg", ".", "two", Visibility::Internal),
    ]);
    let (_, findings) = run(&world);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("pkg.one", "dead.symbol"), ("pkg.two", "dead.symbol")])
    );
    let modules: Vec<(&Value, &Value)> = findings
        .iter()
        .map(|finding| {
            (
                &finding.evidence.0["module"],
                &finding.evidence.0["module_files"],
            )
        })
        .collect();
    let pkg = (json!("pkg"), json!(["pkg/a.x", "pkg/b.x"]));
    assert_eq!(modules, [(&pkg.0, &pkg.1), (&pkg.0, &pkg.1)]);
}

#[test]
fn tests_and_framework_hooks_root_their_files_but_exports_alone_do_not() {
    let world = World::with(vec![
        private("t.x", "t", "case", 1),
        private("t.x", "t", "fixture", 5),
        private("h.x", "h", "hook", 1),
        private("h.x", "h", "helper", 5),
        public("e.x", "e", "api", 1),
    ])
    .root(0, RootReason::Test)
    .root(2, RootReason::FrameworkHook)
    .refer(named("t.x", 2, "fixture"))
    .refer(named("h.x", 2, "helper"));
    let (_, findings) = run(&world);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("e::api", "dead.export")])
    );
}

#[test]
fn only_imports_from_other_modules_enter_one() {
    let world = World::with(vec![
        private("self.x", "selfish", "loop_back", 1),
        symbol("pkg/a.x", "pkg", ".", "one", Visibility::Internal),
        symbol("pkg/b.x", "pkg", ".", "two", Visibility::Internal),
        private("used.x", "used", "kept", 1),
    ])
    .import(edge("self.x", "self", support::imported_file("self.x")))
    .import(edge("pkg/a.x", "pkg/b", support::imported_file("pkg/b.x")))
    .import(edge("index.x", "./used", support::imported_file("used.x")))
    .refer(named("index.x", 1, "kept"));
    let (_, findings) = run(&world);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[
            ("pkg.one", "dead.symbol"),
            ("pkg.two", "dead.symbol"),
            ("selfish::loop_back", "dead.symbol"),
        ]),
        "a file without symbols (index.x) still enters what it imports"
    );
}

#[test]
fn an_unresolved_or_external_import_naming_the_file_lowers_confidence() {
    let world = World::with(vec![
        private("src/plugins/loader.x", "loader", "load", 1),
        public("src/extra.x", "extra", "more", 1),
        private("src/clear.x", "clear", "gone", 1),
    ])
    .import(edge(
        "main.x",
        "./plugins/loader.js",
        ImportTarget::Unresolved,
    ))
    .import(edge(
        "main.x",
        "@scope/extra",
        ImportTarget::External {
            package: "@scope/extra".to_owned(),
        },
    ))
    .import(edge("main.x", "./clearly", ImportTarget::Unresolved));
    let (_, findings) = run(&world);
    let confidence = |display: &str| {
        let finding = findings
            .iter()
            .find(|f| f.symbol.as_ref().unwrap().display == display)
            .unwrap();
        (
            finding.confidence,
            finding.evidence.0["suspect_imports"].clone(),
        )
    };
    let (load, load_suspects) = confidence("loader::load");
    assert!((load - 0.2).abs() < f64::EPSILON, "{load}");
    assert_eq!(load_suspects, json!(["./plugins/loader.js"]));
    let (more, more_suspects) = confidence("extra::more");
    assert!((more - 0.1).abs() < f64::EPSILON, "{more}");
    assert_eq!(more_suspects, json!(["@scope/extra"]));
    let (gone, gone_suspects) = confidence("clear::gone");
    assert!(
        (gone - 0.8).abs() < f64::EPSILON,
        "a segment must equal the stem"
    );
    assert_eq!(gone_suspects, json!([]));
}

#[test]
fn symbols_nested_in_a_dead_symbol_are_part_of_it() {
    let mut inner = private("lost.x", "lost::outer", "inner", 2);
    inner.kind = SymbolKind::Closure;
    inner.span.end_line = 3;
    let mut outer = private("lost.x", "lost", "outer", 1);
    outer.span.end_line = 10;
    let world = World::with(vec![outer, inner]);
    let (_, findings) = run(&world);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("lost::outer", "dead.symbol")])
    );
}

#[test]
fn only_symbols_in_scope_are_reported_but_every_fact_is_read() {
    let world = World::with(vec![
        private("in.x", "inside", "dead", 1),
        private("out.x", "outside", "dead", 1),
        private("lib.x", "lib", "used", 1),
        private("caller.x", "caller", "main", 1),
    ])
    .root(3, RootReason::EntryPoint)
    .import(edge("caller.x", "lib", support::imported_file("lib.x")))
    .refer(named("caller.x", 2, "used"));
    let scope = Scope {
        files: vec!["in.x".to_owned(), "lib.x".to_owned()],
        symbols: Some(BTreeSet::from([
            world.symbols[0].key(),
            world.symbols[2].key(),
        ])),
    };
    let (result, findings) = run_in(&world, scope);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("inside::dead", "dead.symbol")])
    );
    assert_eq!(result.score.unwrap().n, 2);
}

#[test]
fn the_score_counts_dead_symbols_and_combines_by_sum() {
    let world = World::with(vec![
        private("a.x", "a", "one", 1),
        private("b.x", "b", "two", 1),
        public("c.x", "c", "three", 1),
    ]);
    let (result, findings) = run(&world);
    assert_eq!(findings.len(), 3);
    assert_eq!(
        result.metrics,
        [
            ("dead_exports".to_owned(), 1.0),
            ("dead_symbols".to_owned(), 2.0)
        ]
        .into()
    );
    let score = result.score.clone().unwrap();
    assert_eq!(
        score,
        Score {
            value: 2.0,
            unit: "dead_symbols".to_owned(),
            n: 3
        }
    );
    let check = support::check(ID);
    assert_eq!(
        check.combine(&[score.clone(), score]),
        Some(Score {
            value: 4.0,
            unit: "dead_symbols".to_owned(),
            n: 6
        })
    );
    assert_eq!(check.combine(&[]), None);
    let results = [
        language(check, &[("dead_symbols", 2.0), ("dead_exports", 1.0)], 3),
        language(check, &[("dead_symbols", 0.0), ("dead_exports", 4.0)], 9),
    ];
    assert_eq!(combined(check, "dead_symbols", &results), Some(2.0));
    assert_eq!(combined(check, "dead_exports", &results), Some(5.0));
}

#[test]
fn the_check_has_no_settings() {
    let check = support::check(ID);
    let (ctx, _dir) = support::ctx(Scope::default(), json!({"strict": true}));
    let error = check.plan(&ctx, &Scope::default()).unwrap_err();
    let Error::Config(ConfigError::CheckSetting { key, valid, .. }) = error else {
        panic!("expected a check-setting error, got {error:?}")
    };
    assert_eq!(key, "strict");
    assert!(valid.is_empty());
}

#[test]
fn plans_count_the_symbols_in_scope_at_quick_cost() {
    let check = support::check(ID);
    let scope = World::with(vec![
        private("a.x", "a", "one", 1),
        private("b.x", "b", "two", 1),
    ])
    .scope();
    let (ctx, _dir) = support::ctx(scope.clone(), Value::Object(Map::new()));
    assert_eq!(
        check.plan(&ctx, &scope).unwrap(),
        Plan {
            targets: 2,
            est_ms: 2
        }
    );
}

/// A reached module whose entry point calls `helper` and names nothing else in it.
fn reached() -> World {
    World::with(vec![
        private("app.x", "app", "main", 1),
        private("app.x", "app", "helper", 5),
        private("app.x", "app", "orphan", 9),
    ])
    .root(0, RootReason::EntryPoint)
    .refer(named("app.x", 2, "helper"))
}

#[test]
fn a_symbol_nothing_names_in_a_reached_module_is_dead_by_references() {
    let (result, findings) = run(&reached());
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("app::orphan", "dead.symbol")])
    );
    let finding = &findings[0];
    assert_eq!(finding.severity, Severity::Warn);
    assert!((finding.confidence - 0.6).abs() < f64::EPSILON);
    assert_eq!(finding.title, "`app::orphan` is never referenced");
    let detail = finding.detail.as_deref().unwrap();
    assert!(
        detail.contains("names `orphan` outside its own body"),
        "{detail}"
    );
    assert!(detail.contains("by name, not by type"), "{detail}");
    assert_eq!(
        finding.evidence.0,
        *json!({
            "file": "app.x",
            "module": "app",
            "module_files": ["app.x"],
            "suspect_imports": [],
            "reach": "references",
            "dynamic": [],
        })
        .as_object()
        .unwrap()
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert!((result.metrics["dead_symbols"] - 1.0).abs() < f64::EPSILON);
}

#[test]
fn a_reference_resolved_to_one_symbol_does_not_keep_its_namesakes_alive() {
    let world = World::with(vec![
        private("a.x", "a", "main", 1),
        private("a.x", "a", "parse", 5),
        private("b.x", "b", "parse", 1),
        private("c.x", "c", "parse", 1),
    ])
    .root(0, RootReason::EntryPoint)
    .import(edge("a.x", "b", support::imported_file("b.x")))
    .import(edge("a.x", "c", support::imported_file("c.x")))
    .refer(reference(
        "a.x",
        2,
        "parse",
        ReferenceTarget::Local {
            symbols: vec![private("a.x", "a", "parse", 5).key()],
        },
    ))
    .refer(reference(
        "a.x",
        3,
        "parse",
        ReferenceTarget::Imported {
            file: "c.x".to_owned(),
            symbols: vec![private("c.x", "c", "parse", 1).key()],
        },
    ));
    let (_, findings) = run(&world);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[("b::parse", "dead.symbol")])
    );
}

#[test]
fn an_unresolved_reference_keeps_every_symbol_of_its_name_alive() {
    let world = World::with(vec![
        private("a.x", "a", "main", 1),
        method(private("b.x", "b::T", "len", 1)),
        method(private("c.x", "c::U", "len", 1)),
    ])
    .root(0, RootReason::EntryPoint)
    .import(edge("a.x", "b", support::imported_file("b.x")))
    .import(edge("a.x", "c", support::imported_file("c.x")))
    .refer(named("a.x", 2, "len"));
    let (_, findings) = run(&world);
    assert_eq!(support::kinds(&findings), support::pairs(&[]));
}

#[test]
fn a_reference_inside_its_own_body_is_recursion_but_one_in_its_signature_counts() {
    let mut recursive = private("app.x", "app", "recursive", 20);
    recursive.body = Some(Span {
        line: 21,
        col: 1,
        end_line: 22,
        end_col: 2,
    });
    let mut decorated = private("app.x", "app", "decorated", 30);
    decorated.body = Some(Span {
        line: 31,
        col: 1,
        end_line: 32,
        end_col: 2,
    });
    let decorated_key = decorated.key();
    let mut world = reached();
    world.symbols.extend([recursive, decorated]);
    let world = world
        .refer(named("app.x", 21, "recursive"))
        .refer(Reference {
            kind: ReferenceKind::Attribute,
            ..reference(
                "app.x",
                30,
                "route",
                ReferenceTarget::Local {
                    symbols: vec![decorated_key],
                },
            )
        });
    let (_, findings) = run(&world);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[
            ("app::orphan", "dead.symbol"),
            ("app::recursive", "dead.symbol")
        ])
    );
}

#[test]
fn roots_nested_symbols_and_constructors_are_never_reported_by_references() {
    let mut world = reached();
    let mut outer = private("app.x", "app", "outer", 20);
    outer.span.end_line = 30;
    let mut inner = private("app.x", "app::outer", "inner", 22);
    inner.kind = SymbolKind::Closure;
    world.symbols.extend([
        outer,
        inner,
        method(private("app.x", "app::Ledger", "Ledger", 40)),
        method(private("app.x", "app::Ledger", "close", 45)),
        private("app.x", "app", "hook", 50),
        private("app.x", "app", "case", 55),
    ]);
    let world = world
        .refer(named("app.x", 2, "outer"))
        .root(7, RootReason::FrameworkHook)
        .root(8, RootReason::Test);
    let (_, findings) = run(&world);
    assert_eq!(
        support::kinds(&findings),
        support::pairs(&[
            ("app::orphan", "dead.symbol"),
            ("app::Ledger::close", "dead.symbol")
        ])
    );
}

#[test]
fn a_lookup_by_name_lowers_every_finding_and_a_computed_member_only_methods() {
    let mut world = reached();
    world
        .symbols
        .push(method(private("app.x", "app::Ledger", "close", 20)));
    world
        .references
        .dynamic
        .push(dynamic("app.x", 30, DynamicReach::Members));
    let by_member = world;
    let (_, findings) = run(&by_member);
    let confidence = |findings: &[Finding], display: &str| {
        let finding = findings
            .iter()
            .find(|f| f.symbol.as_ref().unwrap().display == display)
            .unwrap();
        (finding.confidence, finding.evidence.0["dynamic"].clone())
    };
    assert_eq!(confidence(&findings, "app::orphan"), (0.6, json!([])));
    assert_eq!(
        confidence(&findings, "app::Ledger::close"),
        (0.3, json!(["app.x:30"]))
    );
    let mut by_name = by_member;
    by_name
        .references
        .dynamic
        .push(dynamic("lib.x", 7, DynamicReach::Names));
    let (_, findings) = run(&by_name);
    assert_eq!(
        confidence(&findings, "app::orphan"),
        (0.3, json!(["lib.x:7"]))
    );
    let detail = findings[0].detail.as_deref().unwrap();
    assert!(detail.contains("computes names at run time"), "{detail}");
}
