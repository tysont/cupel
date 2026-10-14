// ABOUTME: static.clones over synthetic facts: pairs, clusters, clone types, settings, scope, family-wide
// ABOUTME: comparison, finding identity, and deterministic order (ADR-050).
mod support;

use cupel_checks::register;
use cupel_core::config::ConfigError;
use cupel_core::syntax::{
    NormNode, NormTree, NormalizedSymbol, Span, SymbolDef, SymbolKind, Symbols, Visibility,
};
use cupel_core::{
    ActionKind, Cancellation, CheckCtx, CheckId, CheckRegistry, CheckResult, CheckStatus, Error,
    FactKind, Facts, Finding, FindingId, LangId, MetricPer, Scope, SiteHash, SymbolHash, Writer,
};
use serde_json::{Value, json};
use support::{Recorded, combined, language};

const ID: &str = "static.clones";

// ---------------------------------------------------------------- trees

fn node(label: &str, children: Vec<NormNode>) -> NormNode {
    NormNode {
        label: label.to_owned(),
        children,
    }
}

fn leaf(label: &str) -> NormNode {
    node(label, Vec::new())
}

/// `$id = $id <operator> $lit`.
fn statement(operator: &str) -> NormNode {
    node(
        "assignment",
        vec![
            leaf("$id"),
            leaf("="),
            node("binary", vec![leaf("$id"), leaf(operator), leaf("$lit")]),
        ],
    )
}

/// A call statement, `name($id, $lit)`.
fn call(name: &str) -> NormNode {
    node(
        "call",
        vec![
            leaf(name),
            node("arguments", vec![leaf("$id"), leaf("$lit")]),
        ],
    )
}

/// A function whose block holds `body`.
fn function(body: Vec<NormNode>) -> NormNode {
    node(
        "function",
        vec![
            node("parameters", vec![leaf("$id"), leaf("$id")]),
            node("block", body),
        ],
    )
}

/// A function of a dozen statements, about a hundred nodes, built from `seed`'s operators.
fn body(operators: &[&str]) -> Vec<NormNode> {
    operators
        .iter()
        .enumerate()
        .map(|(index, operator)| {
            if index % 3 == 2 {
                node(
                    "loop",
                    vec![
                        leaf("$id"),
                        node("block", vec![statement(operator), call("log")]),
                    ],
                )
            } else {
                statement(operator)
            }
        })
        .collect()
}

const OPERATORS: [&str; 12] = ["+", "-", "*", "/", "+", "%", "-", "*", "+", "-", "/", "+"];

// ---------------------------------------------------------------- facts

/// A symbol in `file` at `line` spanning `lines` lines, with its tree.
struct Def {
    lang: &'static str,
    file: &'static str,
    name: &'static str,
    line: u32,
    lines: u32,
    root: NormNode,
}

fn def(file: &'static str, name: &'static str, line: u32, root: NormNode) -> Def {
    Def {
        lang: "x",
        file,
        name,
        line,
        lines: 12,
        root,
    }
}

impl Def {
    fn symbol(&self) -> SymbolDef {
        SymbolDef {
            name: self.name.to_owned(),
            namespace: "m".to_owned(),
            display: format!("m::{}", self.name),
            kind: SymbolKind::Function,
            visibility: Visibility::Public,
            lang: LangId::from(self.lang),
            file: self.file.to_owned(),
            span: Span {
                line: self.line,
                col: 1,
                end_line: self.line + self.lines - 1,
                end_col: 2,
            },
            body: None,
            parameters: 2,
        }
    }

    /// A structural hash that differs per symbol position, like a language producer's would
    /// per tree.
    fn hash(&self) -> SymbolHash {
        let file = self.file.bytes().fold(0_u32, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(u32::from(byte))
        });
        format!("sh_{file:08x}{:08x}", self.line).parse().unwrap()
    }
}

fn symbols(defs: &[&Def]) -> Symbols {
    Symbols {
        symbols: defs.iter().map(|def| def.symbol()).collect(),
    }
}

fn trees(defs: &[&Def]) -> NormTree {
    NormTree {
        symbols: defs
            .iter()
            .map(|def| NormalizedSymbol {
                symbol: def.symbol().key(),
                hash: def.hash(),
                root: def.root.clone(),
            })
            .collect(),
    }
}

fn registry() -> CheckRegistry {
    let mut registry = CheckRegistry::default();
    register(&mut registry);
    registry
}

/// Facts for one language's `defs`, plus each sibling language's.
fn facts(defs: &[Def], siblings: &[&[Def]]) -> Facts {
    let registry = registry();
    let check = registry.get(&CheckId::from(ID)).unwrap();
    let mut facts = Facts::for_manifest(check.manifest());
    let own: Vec<&Def> = defs.iter().collect();
    facts.insert(symbols(&own)).unwrap();
    facts.insert(trees(&own)).unwrap();
    for sibling in siblings {
        let defs: Vec<&Def> = sibling.iter().collect();
        facts.insert_sibling(symbols(&defs)).unwrap();
        facts.insert_sibling(trees(&defs)).unwrap();
    }
    facts
}

fn every_symbol(defs: &[&[Def]]) -> Scope {
    let all: Vec<&Def> = defs.iter().flat_map(|defs| defs.iter()).collect();
    Scope {
        files: all.iter().map(|def| def.file.to_owned()).collect(),
        symbols: Some(all.iter().map(|def| def.symbol().key()).collect()),
    }
}

fn ctx(scope: Scope, config: Value) -> (CheckCtx, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let writer = Writer::new(dir.path(), false).unwrap();
    let Value::Object(config) = config else {
        panic!("config is a table")
    };
    let ctx = CheckCtx {
        scope,
        config,
        writer: writer.scratch(&cupel_core::RunId::generate()),
        cancel: Cancellation::new(),
    };
    (ctx, dir)
}

/// Runs the check over `facts` in `scope` with `config`; its result and findings.
fn run_with(
    facts: &Facts,
    scope: Scope,
    config: Value,
) -> cupel_core::Result<(CheckResult, Vec<Finding>)> {
    let (result, recorded) = recorded_with(facts, scope, config)?;
    Ok((result, recorded.findings))
}

/// Runs the check; its result and every finding and measurement it reported.
fn recorded_with(
    facts: &Facts,
    scope: Scope,
    config: Value,
) -> cupel_core::Result<(CheckResult, Recorded)> {
    let registry = registry();
    let check = registry.get(&CheckId::from(ID)).unwrap();
    let (ctx, _dir) = ctx(scope, config);
    let plan = check.plan(&ctx, &ctx.scope)?;
    let mut recorded = Recorded::default();
    let result = check.run(&ctx, facts, &plan, &mut recorded)?;
    Ok((result, recorded))
}

fn run(defs: &[Def]) -> (CheckResult, Vec<Finding>) {
    run_with(&facts(defs, &[]), every_symbol(&[defs]), json!({})).unwrap()
}

fn members(finding: &Finding) -> Vec<String> {
    let evidence = &finding.evidence.0;
    let names = |member: &Value| member["symbol"].as_str().unwrap().to_owned();
    match finding.kind.as_str() {
        "clone.pair" => vec![names(&evidence["left"]), names(&evidence["right"])],
        _ => evidence["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(names)
            .collect(),
    }
}

/// The score counts `pairs` similar pairs.
fn assert_pairs(result: &CheckResult, pairs: u32) {
    let score = result.score.as_ref().unwrap();
    assert_eq!(score.unit, "pairs");
    assert!((score.value - f64::from(pairs)).abs() < 1e-9, "{score:?}");
}

// ---------------------------------------------------------------- tests

#[test]
fn the_manifest_declares_its_facts_kinds_actions_and_metrics() {
    let registry = registry();
    let check = registry.get(&CheckId::from(ID)).unwrap();
    let manifest = check.manifest();
    assert_eq!(manifest.needs, [FactKind::Symbols, FactKind::NormTree]);
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
            ("clone.pair", ActionKind::RefactorExtract),
            ("clone.cluster", ActionKind::RefactorExtract),
        ]
    );
    let metrics: Vec<(&str, MetricPer)> = manifest
        .metrics
        .iter()
        .map(|m| (m.name.as_str(), m.per))
        .collect();
    assert_eq!(
        metrics,
        [
            ("pairs", MetricPer::Scope),
            ("clusters", MetricPer::Scope),
            ("max_jaccard", MetricPer::Scope),
            ("cluster_size", MetricPer::Group),
        ]
    );
    assert!(check.spans_family());
}

#[test]
fn a_renamed_copy_is_a_type_2_pair_with_both_spans_in_evidence() {
    let defs = [
        def("a.x", "settle", 1, function(body(&OPERATORS))),
        def("b.x", "reconcile", 20, function(body(&OPERATORS))),
        def(
            "b.x",
            "unrelated",
            40,
            function(vec![call("a"), call("b"), call("c"), call("d")]),
        ),
    ];
    let (result, findings) = run(&defs);
    assert_eq!(findings.len(), 1, "{findings:#?}");
    let finding = &findings[0];
    assert_eq!(finding.kind, "clone.pair");
    assert_eq!(finding.action.kind, ActionKind::RefactorExtract);
    assert_eq!(finding.symbol.as_ref().unwrap().display, "m::settle");
    assert_eq!(finding.location.as_ref().unwrap().file, "a.x");
    let evidence = &finding.evidence.0;
    assert_eq!(evidence["clone_type"], json!(2));
    assert_eq!(evidence["jaccard"], json!(1.0));
    assert_eq!(evidence["threshold"], json!(0.82));
    assert_eq!(
        evidence["right"],
        json!({
            "symbol": "m::reconcile", "hash": defs[1].hash().to_string(), "lang": "x",
            "file": "b.x", "line": 20, "col": 1, "end_line": 31, "end_col": 2,
            "lines": 12, "nodes": evidence["right"]["nodes"],
        })
    );
    assert_eq!(evidence["left"]["line"], json!(1));
    assert!(
        finding.title.contains("type-2") && finding.title.contains("b.x:20"),
        "{}",
        finding.title
    );
    assert_eq!(result.status, CheckStatus::Fail);
    assert_eq!(result.findings, 1);
    let score = result.score.unwrap();
    assert_eq!(
        (score.value, score.unit.as_str(), score.n),
        (1.0, "pairs", 3)
    );
}

#[test]
fn a_copy_with_an_extra_statement_is_a_type_3_pair() {
    let mut extended = body(&OPERATORS);
    extended.push(call("audit"));
    let defs = [
        def("a.x", "orders", 1, function(body(&OPERATORS))),
        def("a.x", "refunds", 20, function(extended)),
    ];
    let (_, findings) = run(&defs);
    assert_eq!(findings.len(), 1);
    let evidence = &findings[0].evidence.0;
    assert_eq!(evidence["clone_type"], json!(3));
    let jaccard = evidence["jaccard"].as_f64().unwrap();
    assert!((0.82..1.0).contains(&jaccard), "{jaccard}");
}

#[test]
fn three_similar_symbols_are_one_cluster_and_no_pairs() {
    let mut changed = body(&OPERATORS);
    changed[0] = statement("<<");
    let defs = [
        def("a.x", "one", 1, function(body(&OPERATORS))),
        def("a.x", "two", 20, function(body(&OPERATORS))),
        def("c.x", "three", 1, function(changed)),
    ];
    let (result, findings) = run(&defs);
    assert_eq!(findings.len(), 1, "{findings:#?}");
    let finding = &findings[0];
    assert_eq!(finding.kind, "clone.cluster");
    assert_eq!(members(finding), ["m::one", "m::two", "m::three"]);
    let evidence = &finding.evidence.0;
    assert_eq!(evidence["pairs"], json!(3));
    assert_eq!(evidence["max_jaccard"], json!(1.0));
    assert!(evidence["min_jaccard"].as_f64().unwrap() < 1.0);
    assert!(
        finding.title.contains("2 other symbols"),
        "{}",
        finding.title
    );
    assert_pairs(&result, 3);
}

#[test]
fn distinct_functions_produce_no_findings() {
    let defs = [
        def("a.x", "one", 1, function(body(&OPERATORS))),
        def(
            "a.x",
            "two",
            20,
            function(vec![call("a"), call("b"), call("c"), call("d"), call("e")]),
        ),
        def(
            "a.x",
            "three",
            40,
            function(vec![node(
                "loop",
                vec![
                    leaf("$id"),
                    node("block", (0..8).map(|_| statement("^")).collect()),
                ],
            )]),
        ),
    ];
    let (result, findings) = run(&defs);
    assert!(findings.is_empty(), "{findings:#?}");
    assert_eq!(result.status, CheckStatus::Pass);
    assert_pairs(&result, 0);
}

#[test]
fn symbols_under_min_lines_or_min_nodes_are_not_compared() {
    let mut short = def("a.x", "short", 1, function(body(&OPERATORS)));
    short.lines = 3;
    let tiny = |name, line| def("a.x", name, line, function(vec![statement("+")]));
    let defs = [
        short,
        def("a.x", "long", 20, function(body(&OPERATORS))),
        tiny("tiny", 40),
        tiny("tinier", 60),
    ];
    let (result, findings) = run(&defs);
    assert!(findings.is_empty(), "{findings:#?}");
    assert_eq!(result.score.unwrap().n, 1, "only `long` is compared");

    let lenient = run_with(
        &facts(&defs, &[]),
        every_symbol(&[&defs]),
        json!({"min_lines": 3, "min_nodes": 5}),
    )
    .unwrap()
    .1;
    let kinds: Vec<Vec<String>> = lenient.iter().map(members).collect();
    assert_eq!(
        kinds,
        [vec!["m::short", "m::long"], vec!["m::tiny", "m::tinier"]]
    );
}

#[test]
fn a_symbol_is_never_a_clone_of_a_symbol_inside_it() {
    let inner = function(body(&OPERATORS));
    let mut outer = def("a.x", "outer", 1, node("function", vec![inner.clone()]));
    outer.lines = 30;
    let defs = [outer, def("a.x", "inner", 5, inner)];
    let (_, findings) = run(&defs);
    assert!(findings.is_empty(), "{findings:#?}");
}

#[test]
fn the_threshold_is_configurable_and_validated() {
    let mut changed = body(&OPERATORS);
    changed[0] = statement("<<");
    changed[3] = call("other");
    let defs = [
        def("a.x", "one", 1, function(body(&OPERATORS))),
        def("a.x", "two", 20, function(changed)),
    ];
    let facts = facts(&defs, &[]);
    let at = |threshold: f64| {
        run_with(
            &facts,
            every_symbol(&[&defs]),
            json!({"threshold": threshold}),
        )
        .unwrap()
        .1
        .len()
    };
    assert_eq!(at(0.5), 1);
    assert_eq!(at(1.0), 0);
    for (config, key) in [
        (json!({"threshold": 0}), "threshold"),
        (json!({"threshold": 1.5}), "threshold"),
        (json!({"threshold": "high"}), "threshold"),
        (json!({"min_lines": -1}), "min_lines"),
        (json!({"min_nodes": 2.5}), "min_nodes"),
        (json!({"distance": 3}), "distance"),
    ] {
        let error = run_with(&facts, every_symbol(&[&defs]), config.clone()).unwrap_err();
        let Error::Config(ConfigError::CheckSetting {
            check,
            key: got,
            valid,
            ..
        }) = error
        else {
            panic!("{config}: expected a setting error, got {error:?}")
        };
        assert_eq!((check.as_str(), got.as_str()), (ID, key), "{config}");
        assert_eq!(valid, ["threshold", "min_lines", "min_nodes"]);
    }
}

#[test]
fn a_group_is_reported_when_any_member_is_in_scope() {
    let defs = [
        def("a.x", "settle", 1, function(body(&OPERATORS))),
        def("b.x", "reconcile", 20, function(body(&OPERATORS))),
    ];
    let facts = facts(&defs, &[]);
    let only = |keys: Vec<cupel_core::syntax::SymbolKey>| Scope {
        files: vec!["a.x".to_owned(), "b.x".to_owned()],
        symbols: Some(keys.into_iter().collect()),
    };
    let second = run_with(&facts, only(vec![defs[1].symbol().key()]), json!({}))
        .unwrap()
        .1;
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].symbol.as_ref().unwrap().display, "m::settle");
    let none = run_with(&facts, only(vec![]), json!({})).unwrap();
    assert!(none.1.is_empty());
    // The score counts every similar pair, reported or not.
    assert_pairs(&none.0, 1);
}

#[test]
fn symbols_of_every_family_language_are_compared_with_each_other() {
    let javascript = [Def {
        lang: "javascript",
        ..def("a.js", "settle", 1, function(body(&OPERATORS)))
    }];
    let typescript = [Def {
        lang: "typescript",
        ..def("b.ts", "settle", 1, function(body(&OPERATORS)))
    }];
    let facts = facts(&javascript, &[&typescript]);
    let (_, findings) =
        run_with(&facts, every_symbol(&[&javascript, &typescript]), json!({})).unwrap();
    assert_eq!(findings.len(), 1);
    let evidence = &findings[0].evidence.0;
    assert_eq!(
        (
            evidence["left"]["lang"].as_str(),
            evidence["right"]["lang"].as_str()
        ),
        (Some("javascript"), Some("typescript"))
    );
}

#[test]
fn finding_ids_come_from_the_first_member_and_the_others_hashes() {
    let defs = [
        def("a.x", "settle", 1, function(body(&OPERATORS))),
        def("b.x", "reconcile", 20, function(body(&OPERATORS))),
    ];
    let finding = run(&defs).1.remove(0);
    assert_eq!(finding.symbol.as_ref().unwrap().hash, defs[0].hash());
    assert_eq!(
        finding.repro,
        format!("cupel check --only {ID} --symbols {}", defs[0].hash())
    );
    assert_eq!(
        finding.suppress,
        format!("cupel suppress {} --reason '…'", finding.id)
    );
    let moved = [
        def("a.x", "settle", 1, function(body(&OPERATORS))),
        def("c.x", "elsewhere", 20, function(body(&OPERATORS))),
    ];
    let other_partner = run(&moved).1.remove(0);
    assert_ne!(
        finding.id, other_partner.id,
        "the partner's hash is part of the identity"
    );
    let any_site: SiteHash = "0000000000000000".parse().unwrap();
    assert_ne!(
        finding.id,
        FindingId::compute(
            &CheckId::from(ID),
            "clone.pair",
            &defs[0].hash(),
            Some(&any_site)
        )
    );
}

#[test]
fn findings_do_not_depend_on_the_order_of_the_facts() {
    let mut changed = body(&OPERATORS);
    changed[0] = statement("<<");
    let defs = [
        def("a.x", "one", 1, function(body(&OPERATORS))),
        def("b.x", "two", 1, function(changed)),
        def(
            "c.x",
            "three",
            1,
            function(vec![call("a"), call("b"), call("c"), call("d"), call("e")]),
        ),
        def(
            "d.x",
            "four",
            1,
            function(vec![call("a"), call("b"), call("c"), call("d"), call("e")]),
        ),
    ];
    let reversed: Vec<Def> = defs
        .iter()
        .rev()
        .map(|d| def(d.file, d.name, d.line, d.root.clone()))
        .collect();
    let ids = |findings: Vec<Finding>| findings.into_iter().map(|f| f.id).collect::<Vec<_>>();
    assert_eq!(ids(run(&defs).1), ids(run(&reversed).1));
    assert_eq!(run(&defs).1.len(), 2);
}

const OTHER_OPERATORS: [&str; 12] = [
    "&", "|", "^", "<<", "&", ">>", "|", "^", "&", "<<", ">>", "|",
];

/// A cluster of three (`one`, `two`, `three`) and, unrelated to it, a pair (`four`, `five`).
fn a_cluster_and_a_pair() -> [Def; 5] {
    let mut changed = body(&OPERATORS);
    changed[0] = statement("<<");
    [
        def("a.x", "one", 1, function(body(&OPERATORS))),
        def("a.x", "two", 20, function(body(&OPERATORS))),
        def("c.x", "three", 1, function(changed)),
        def("d.x", "four", 1, function(body(&OTHER_OPERATORS))),
        def("d.x", "five", 20, function(body(&OTHER_OPERATORS))),
    ]
}

#[test]
fn the_result_counts_pairs_and_clusters_of_the_reported_groups() {
    let defs = a_cluster_and_a_pair();
    let facts = facts(&defs, &[]);
    let (all, findings) = run_with(&facts, every_symbol(&[&defs]), json!({})).unwrap();
    assert_eq!(findings.len(), 2, "{findings:#?}");
    assert_eq!(
        all.metrics,
        [
            ("clusters".to_owned(), 1.0),
            ("max_jaccard".to_owned(), 1.0),
            ("pairs".to_owned(), 4.0),
        ]
        .into(),
        "three pairs in the cluster and the pair"
    );
    let only = |defs: &[&Def]| Scope {
        files: vec!["a.x".to_owned(), "c.x".to_owned(), "d.x".to_owned()],
        symbols: Some(defs.iter().map(|def| def.symbol().key()).collect()),
    };
    let (pair, _) = run_with(&facts, only(&[&defs[4]]), json!({})).unwrap();
    assert_eq!(
        pair.metrics,
        [
            ("clusters".to_owned(), 0.0),
            ("max_jaccard".to_owned(), 1.0),
            ("pairs".to_owned(), 1.0),
        ]
        .into()
    );
    let (none, _) = run_with(&facts, only(&[]), json!({})).unwrap();
    assert!(
        none.metrics.values().all(|value| *value == 0.0),
        "{:?}",
        none.metrics
    );
}

#[test]
fn each_reported_group_is_measured_by_its_members_for_the_cluster_ranking() {
    let defs = a_cluster_and_a_pair();
    let (_, recorded) =
        recorded_with(&facts(&defs, &[]), every_symbol(&[&defs]), json!({})).unwrap();
    assert_eq!(
        recorded.measured(),
        [
            ("cluster_size", "m::one and 2 others", 3.0),
            ("cluster_size", "m::four and 1 other", 2.0),
        ]
    );
    let cluster = &recorded.measurements[0].target;
    assert_eq!(cluster.symbol, None, "a group is not a symbol");
    let location = cluster.location.as_ref().unwrap();
    assert_eq!((location.file.as_str(), location.line), ("a.x", 1));
}

#[test]
fn counts_add_up_across_families_and_similarity_takes_the_largest() {
    let registry = registry();
    let check = registry.get(&CheckId::from(ID)).unwrap();
    let results = [
        language(
            check,
            &[("pairs", 3.0), ("clusters", 1.0), ("max_jaccard", 0.9)],
            10,
        ),
        language(
            check,
            &[("pairs", 2.0), ("clusters", 0.0), ("max_jaccard", 1.0)],
            5,
        ),
    ];
    assert_eq!(combined(check, "pairs", &results), Some(5.0));
    assert_eq!(combined(check, "clusters", &results), Some(1.0));
    assert_eq!(combined(check, "max_jaccard", &results), Some(1.0));
}
