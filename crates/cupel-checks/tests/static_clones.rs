// ABOUTME: static.clones over synthetic facts: pairs, clusters, clone types, settings, scope, family-wide
// ABOUTME: comparison, finding identity, deterministic order (ADR-050), and the common-subtree hint (ADR-133).
mod support;

use cupel_core::config::ConfigError;
use cupel_core::hint::{self, CommonSubtree, Parameter, ParameterKind, PlacedSubtree};
use cupel_core::syntax::{
    NormNode, NormTree, NormalizedSymbol, Span, SymbolDef, SymbolKind, Symbols, Visibility,
};
use cupel_core::{
    ActionKind, CheckId, CheckResult, CheckStatus, Error, FactKind, Facts, Finding, FindingId,
    LangId, Location, MetricPer, Scope, SiteHash, SymbolHash,
};
use serde_json::{Value, json};
use support::{Recorded, combined, language};

const ID: &str = "static.clones";

// ---------------------------------------------------------------- trees

/// `$id = $id <operator> $lit`.
fn statement(operator: &str) -> NormNode {
    NormNode::new(
        "assignment",
        vec![
            NormNode::leaf("$id"),
            NormNode::leaf("="),
            NormNode::new(
                "binary",
                vec![
                    NormNode::leaf("$id"),
                    NormNode::leaf(operator),
                    NormNode::leaf("$lit"),
                ],
            ),
        ],
    )
}

/// A call statement, `name($id, $lit)`.
fn call(name: &str) -> NormNode {
    NormNode::new(
        "call",
        vec![
            NormNode::leaf(name),
            NormNode::new(
                "arguments",
                vec![NormNode::leaf("$id"), NormNode::leaf("$lit")],
            ),
        ],
    )
}

/// A function whose block holds `body`.
fn function(body: Vec<NormNode>) -> NormNode {
    NormNode::new(
        "function",
        vec![
            NormNode::new(
                "parameters",
                vec![NormNode::leaf("$id"), NormNode::leaf("$id")],
            ),
            NormNode::new("block", body),
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
                NormNode::new(
                    "loop",
                    vec![
                        NormNode::leaf("$id"),
                        NormNode::new("block", vec![statement(operator), call("log")]),
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

/// Facts for one language's `defs`, plus each sibling language's.
fn facts(defs: &[Def], siblings: &[&[Def]]) -> Facts {
    let check = support::check(ID);
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
    support::recorded(support::check(ID), facts, scope, config)
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
fn the_manifest_states_its_known_limitation() {
    let manifest = support::check("static.clones").manifest();
    let limitation = manifest
        .summary
        .split_once("Limitation: ")
        .map(|(_, rest)| rest);
    assert_eq!(
        limitation,
        Some(
            "type 1 is reported as type 2, because NormTree keeps no identifier or literal text, \
             so two functions that differ only in literals (two lookup tables of the same shape) \
             are reported as clones (ADR-050; a raw-token digest is a P3 follow-up)."
        )
    );
}

#[test]
fn the_manifest_declares_its_settings_with_their_defaults() {
    assert_eq!(
        support::declared_settings(ID),
        [
            ("threshold", json!(0.82)),
            ("min_lines", json!(4)),
            ("min_nodes", json!(20))
        ]
    );
}

#[test]
fn the_manifest_declares_its_facts_kinds_actions_and_metrics() {
    let check = support::check(ID);
    let manifest = check.manifest();
    assert_eq!(
        support::Shape::of(manifest),
        support::Shape {
            needs: vec![FactKind::Symbols, FactKind::NormTree],
            wants: vec![],
            tier: 0,
            cost: cupel_core::CostClass::Quick,
            kinds: vec![
                ("clone.pair", ActionKind::RefactorExtract),
                ("clone.cluster", ActionKind::RefactorExtract)
            ],
            metrics: vec![
                ("pairs", MetricPer::Scope),
                ("clusters", MetricPer::Scope),
                ("max_jaccard", MetricPer::Scope),
                ("cluster_size", MetricPer::Group)
            ],
            evidence: vec!["object"; 2],
        }
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
            function(vec![NormNode::new(
                "loop",
                vec![
                    NormNode::leaf("$id"),
                    NormNode::new("block", (0..8).map(|_| statement("^")).collect()),
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
    let mut outer = def(
        "a.x",
        "outer",
        1,
        NormNode::new("function", vec![inner.clone()]),
    );
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
    let check = support::check(ID);
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

// ---------------------------------------------------------------- the common-subtree hint

/// `$id = $id <operator> $lit` with its markers' texts: `total = <operand> <operator> 100`.
fn named_statement(operator: &str, operand: &str) -> NormNode {
    texts(statement(operator), &["total", operand, "100"])
}

/// `node` with the given texts on its marker leaves (`$id`, `$lit`), in pre-order.
fn texts(mut node: NormNode, values: &[&str]) -> NormNode {
    fn fill(node: &mut NormNode, values: &mut std::slice::Iter<'_, &str>) {
        if node.label.starts_with('$') {
            node.text = values.next().map(|value| (*value).to_owned());
        }
        for child in &mut node.children {
            fill(child, values);
        }
    }
    fill(&mut node, &mut values.iter());
    node
}

/// The loop every member of the hint's cluster shares: `for items { three statements; audit }`,
/// its operand and the audit's literal differing per member.
fn shared(operand: &str, literal: &str) -> NormNode {
    NormNode::new(
        "batch",
        vec![
            texts(NormNode::leaf("$id"), &["items"]),
            NormNode::new(
                "block",
                vec![
                    named_statement("+", operand),
                    named_statement("-", operand),
                    named_statement("*", operand),
                    texts(call("audit"), &["total", literal]),
                ],
            ),
        ],
    )
}

/// Every node of `root` placed on its own line from `first`, in pre-order, each spanning its
/// line from column 1 to 9.
fn placed(mut root: NormNode, first: u32) -> NormNode {
    fn place(node: &mut NormNode, line: &mut u32) {
        node.span = Some(Span {
            line: *line,
            col: 1,
            end_line: *line,
            end_col: 9,
        });
        *line += 1;
        for child in &mut node.children {
            place(child, line);
        }
    }
    let mut line = first;
    place(&mut root, &mut line);
    root
}

/// The line `label`'s first node is placed on in `def`'s tree.
fn line_of(def: &Def, label: &str) -> u32 {
    fn find(node: &NormNode, label: &str) -> Option<u32> {
        if node.label == label {
            return node.span.map(|span| span.line);
        }
        node.children.iter().find_map(|child| find(child, label))
    }
    find(&def.root, label).unwrap()
}

/// A member of the hint's cluster: the common body, the shared loop, and a last statement of
/// its own, so no two members' whole trees are equal.
fn member(
    file: &'static str,
    name: &'static str,
    line: u32,
    own: &str,
    operand: &str,
    literal: &str,
) -> Def {
    let mut statements = body(&OPERATORS);
    statements.insert(4, shared(operand, literal));
    statements.push(statement(own));
    def(file, name, line, placed(function(statements), line))
}

/// How many nodes the tree rooted at `node` has.
fn size(node: &NormNode) -> u32 {
    1 + node.children.iter().map(size).sum::<u32>()
}

fn located(file: &str, line: u32) -> Location {
    Location {
        file: file.to_owned(),
        line,
        col: Some(1),
        end_line: line,
        end_col: Some(9),
    }
}

#[test]
fn a_cluster_names_the_largest_subtree_its_members_share_and_the_parameters_that_differ() {
    let defs = [
        member("a.x", "one", 1, "<<", "rate", "1"),
        member("a.x", "two", 200, ">>", "fee", "2"),
        member("c.x", "three", 1, "^", "tax", "3"),
    ];
    let (_, findings) = run(&defs);
    assert_eq!(findings.len(), 1, "{findings:#?}");
    assert_eq!(findings[0].kind, "clone.cluster");
    let found: CommonSubtree = hint::read(&findings[0].action.hints).unwrap();
    let nodes = size(&shared("x", "y"));
    let share = (f64::from(nodes) / f64::from(size(&defs[0].root)) * 1000.0).round() / 1000.0;
    assert_eq!(
        found,
        CommonSubtree {
            nodes,
            share,
            location: located("a.x", line_of(&defs[0], "batch")),
            others: vec![
                PlacedSubtree {
                    symbol: "m::two".to_owned(),
                    location: located("a.x", line_of(&defs[1], "batch")),
                },
                PlacedSubtree {
                    symbol: "m::three".to_owned(),
                    location: located("c.x", line_of(&defs[2], "batch")),
                },
            ],
            parameters: vec![
                Parameter {
                    kind: ParameterKind::Identifier,
                    values: vec!["rate".to_owned(), "fee".to_owned(), "tax".to_owned()],
                    occurrences: 3,
                },
                Parameter {
                    kind: ParameterKind::Literal,
                    values: vec!["1".to_owned(), "2".to_owned(), "3".to_owned()],
                    occurrences: 1,
                },
            ],
        }
    );
}

#[test]
fn a_cluster_whose_trees_carry_no_spans_has_no_common_subtree_hint() {
    let mut changed = body(&OPERATORS);
    changed[0] = statement("<<");
    let defs = [
        def("a.x", "one", 1, function(body(&OPERATORS))),
        def("a.x", "two", 20, function(body(&OPERATORS))),
        def("c.x", "three", 1, function(changed)),
    ];
    let (_, findings) = run(&defs);
    assert_eq!(findings[0].kind, "clone.cluster");
    assert!(
        findings[0].action.hints.is_empty(),
        "{:?}",
        findings[0].action.hints
    );
}

#[test]
fn a_pair_carries_no_common_subtree_hint() {
    let defs = [
        member("a.x", "one", 1, "<<", "rate", "1"),
        member("a.x", "two", 200, ">>", "fee", "2"),
    ];
    let (_, findings) = run(&defs);
    assert_eq!(findings[0].kind, "clone.pair");
    assert!(findings[0].action.hints.is_empty());
}

#[test]
fn the_cluster_kind_declares_its_common_subtree_hint() {
    let manifest = support::check(ID).manifest().clone();
    let hints = |kind: &str| -> Vec<String> {
        manifest
            .findings
            .iter()
            .find(|spec| spec.kind == kind)
            .unwrap()
            .hints
            .keys()
            .cloned()
            .collect()
    };
    assert_eq!(hints("clone.cluster"), ["common_subtree"]);
    assert!(hints("clone.pair").is_empty());
}
