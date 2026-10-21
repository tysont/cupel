// ABOUTME: Tests for the syntactic fact payloads: JSON shape, semantic enum spellings, and fact kinds.
// ABOUTME: Enum spellings match the construct names used by the per-fact conformance inventories.
use cupel_core::syntax::{
    ApiItem, ApiItemKind, ApiSurface, Assertion, AssertionKind, AssertionStrength, Decision,
    DecisionKind, Decisions, EdgeKind, ImportEdge, ImportTarget, LiveRoot, Mock, ModuleGraph,
    MutationOperator, MutationSites, NormNode, NormTree, NormalizedSymbol, OperatorSet,
    ProjectInfo, ProjectRoot, RootImports, RootReason, Roots, Site, SkipMarker, Span,
    SymbolDecisions, SymbolDef, SymbolKey, SymbolKind, SymbolSites, Symbols, TestCase,
    TestConventions, TestFile, TestFrames, TestPairing, Visibility,
};
use cupel_core::{Fact, FactKind, LangId};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::fmt::Debug;

fn span(line: u32) -> Span {
    Span {
        line,
        col: 1,
        end_line: line + 2,
        end_col: 2,
    }
}

fn key() -> SymbolKey {
    SymbolKey {
        file: "src/lib.rs".into(),
        line: 10,
        col: 1,
    }
}

fn assert_round_trip<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T) -> Value {
    let json = serde_json::to_value(value).unwrap();
    assert_eq!(&serde_json::from_value::<T>(json.clone()).unwrap(), value);
    json
}

fn assert_spellings<T: Serialize + DeserializeOwned + PartialEq + Debug>(cases: &[(T, &str)]) {
    for (value, text) in cases {
        assert_eq!(serde_json::to_value(value).unwrap(), json!(text));
        assert_eq!(&serde_json::from_value::<T>(json!(text)).unwrap(), value);
    }
}

fn symbol_def() -> SymbolDef {
    SymbolDef {
        name: "push_limit".into(),
        namespace: "rewrite::Planner".into(),
        display: "rewrite::Planner::push_limit".into(),
        kind: SymbolKind::Method,
        visibility: Visibility::Public,
        lang: LangId::from("rust"),
        file: "src/lib.rs".into(),
        span: Span {
            line: 10,
            col: 1,
            end_line: 30,
            end_col: 2,
        },
        body: Some(span(11)),
        parameters: 2,
    }
}

#[test]
fn symbols_payload_shape() {
    let json = assert_round_trip(&Symbols {
        symbols: vec![symbol_def()],
    });
    assert_eq!(
        json["symbols"][0],
        json!({
            "name": "push_limit", "namespace": "rewrite::Planner",
            "display": "rewrite::Planner::push_limit", "kind": "method", "visibility": "public",
            "lang": "rust", "file": "src/lib.rs",
            "span": {"line": 10, "col": 1, "end_line": 30, "end_col": 2},
            "body": {"line": 11, "col": 1, "end_line": 13, "end_col": 2},
            "parameters": 2
        })
    );
}

#[test]
fn symbol_key_is_the_start_of_the_symbol_span() {
    assert_eq!(symbol_def().key(), key());
}

#[test]
fn decisions_payload_shape() {
    let json = assert_round_trip(&Decisions {
        symbols: vec![SymbolDecisions {
            symbol: key(),
            decisions: vec![Decision {
                kind: DecisionKind::EarlyReturn,
                nesting: 2,
                cyclomatic: 1,
                cognitive: 3,
                table: false,
                continues: false,
                span: None,
            }],
        }],
    });
    assert_eq!(
        json["symbols"][0]["decisions"][0],
        json!({"kind": "early-return", "nesting": 2, "cyclomatic": 1, "cognitive": 3, "table": false, "continues": false})
    );
}

#[test]
fn a_decision_carries_its_span_when_known() {
    let json = assert_round_trip(&Decision {
        kind: DecisionKind::Loop,
        nesting: 1,
        cyclomatic: 1,
        cognitive: 1,
        table: false,
        continues: false,
        span: Some(span(4)),
    });
    assert_eq!(
        json["span"],
        json!({"line": 4, "col": 1, "end_line": 6, "end_col": 2})
    );
}

#[test]
fn decision_kinds_use_conformance_names() {
    assert_spellings(&[
        (DecisionKind::Branch, "branch"),
        (DecisionKind::Loop, "loop"),
        (DecisionKind::BooleanOp, "boolean-op"),
        (DecisionKind::Case, "case"),
        (DecisionKind::Catch, "catch"),
        (DecisionKind::EarlyReturn, "early-return"),
        (DecisionKind::Ternary, "ternary"),
    ]);
}

#[test]
fn norm_tree_payload_carries_symbol_hash() {
    let json = assert_round_trip(&NormTree {
        symbols: vec![NormalizedSymbol {
            symbol: key(),
            hash: "sh_4b1d9e0c4b1d9e0c".parse().unwrap(),
            root: NormNode::new("function", vec![NormNode::leaf("$local")]),
        }],
    });
    assert_eq!(json["symbols"][0]["hash"], json!("sh_4b1d9e0c4b1d9e0c"));
    assert_eq!(
        json["symbols"][0]["root"]["children"][0]["label"],
        json!("$local")
    );
}

#[test]
fn mutation_sites_payload_shape() {
    let json = assert_round_trip(&MutationSites {
        symbols: vec![SymbolSites {
            symbol: key(),
            sites: vec![Site {
                operator: MutationOperator::Relational,
                set: OperatorSet::Default,
                from: "<=".into(),
                to: vec!["<".into(), ">".into()],
                site_hash: "a1b2c3d4e5f60718".parse().unwrap(),
                span: span(12),
            }],
        }],
    });
    let site = &json["symbols"][0]["sites"][0];
    assert_eq!(site["operator"], json!("relational"));
    assert_eq!(site["set"], json!("default"));
    assert_eq!(site["site_hash"], json!("a1b2c3d4e5f60718"));
}

#[test]
fn mutation_operators_use_conformance_names() {
    assert_spellings(&[
        (MutationOperator::Relational, "relational"),
        (MutationOperator::Equality, "equality"),
        (MutationOperator::ArithAddSub, "arith-add-sub"),
        (MutationOperator::ArithMulDiv, "arith-mul-div"),
        (MutationOperator::BooleanOp, "boolean-op"),
        (MutationOperator::BooleanLiteral, "boolean-literal"),
        (MutationOperator::ZeroOne, "zero-one"),
        (MutationOperator::StatementDeletion, "statement-deletion"),
        (MutationOperator::ReturnValue, "return-value"),
        (MutationOperator::Constant, "constant"),
        (MutationOperator::Boundary, "boundary"),
        (MutationOperator::Negation, "negation"),
    ]);
    assert_spellings(&[
        (OperatorSet::Default, "default"),
        (OperatorSet::Extended, "extended"),
    ]);
}

#[test]
fn test_frames_payload_shape() {
    let json = assert_round_trip(&TestFrames {
        files: vec![TestFile {
            file: "tests/limit_tests.rs".into(),
            framework: "cargo-test".into(),
            setup: vec![span(3)],
            cases: vec![TestCase {
                name: "pushes_through_sort".into(),
                span: span(8),
                assertions: vec![Assertion {
                    kind: AssertionKind::Equality,
                    strength: AssertionStrength::Weak,
                    span: span(9),
                }],
                mocks: vec![Mock {
                    target: "Clock::now".into(),
                    span: span(10),
                }],
                skip: Some(SkipMarker {
                    reason: Some("flaky on CI".into()),
                    span: span(7),
                }),
                property: Some("proptest".into()),
                doc: Some("/// Sorting pushes through. REQ-4".into()),
            }],
        }],
    });
    let case = &json["files"][0]["cases"][0];
    assert_eq!(case["property"], json!("proptest"));
    assert_eq!(case["doc"], json!("/// Sorting pushes through. REQ-4"));
    assert_eq!(case["assertions"][0]["kind"], json!("equality"));
    assert_eq!(case["assertions"][0]["strength"], json!("weak"));
    assert_eq!(case["skip"]["reason"], json!("flaky on CI"));
}

#[test]
fn assertion_kinds_and_strengths() {
    assert_spellings(&[
        (AssertionKind::Equality, "equality"),
        (AssertionKind::Truthiness, "truthiness"),
        (AssertionKind::NotNull, "not-null"),
        (AssertionKind::Throws, "throws"),
        (AssertionKind::Snapshot, "snapshot"),
        (AssertionKind::Custom, "custom"),
    ]);
    assert_spellings(&[
        (AssertionStrength::Strong, "strong"),
        (AssertionStrength::Weak, "weak"),
    ]);
}

#[test]
fn module_graph_edges_are_tagged_by_target_kind() {
    let edge = |target| ImportEdge {
        from: "src/a.ts".into(),
        specifier: "./b".into(),
        target,
        relative: true,
        line: 1,
        edge_kind: EdgeKind::ReExport,
        names: Vec::new(),
    };
    let json = assert_round_trip(&ModuleGraph {
        roots: vec![RootImports {
            root: ".".into(),
            edges: vec![
                edge(ImportTarget::File {
                    path: "src/b.ts".into(),
                }),
                edge(ImportTarget::External {
                    package: "lodash".into(),
                }),
                edge(ImportTarget::Unresolved),
            ],
        }],
    });
    let edges = &json["roots"][0]["edges"];
    assert_eq!(
        edges[0]["target"],
        json!({"kind": "file", "path": "src/b.ts"})
    );
    assert_eq!(
        edges[1]["target"],
        json!({"kind": "external", "package": "lodash"})
    );
    assert_eq!(edges[2]["target"], json!({"kind": "unresolved"}));
    assert_eq!(edges[0]["relative"], json!(true));
    assert_eq!(edges[0]["edge_kind"], json!("re-export"));
}

#[test]
fn api_surface_payload_shape() {
    let json = assert_round_trip(&ApiSurface {
        items: vec![ApiItem {
            path: "planner::Planner::push_limit".into(),
            kind: ApiItemKind::Method,
            signature: "fn(&mut self, $int) -> $bool".into(),
            file: "src/lib.rs".into(),
            span: span(10),
        }],
    });
    assert_eq!(json["items"][0]["kind"], json!("method"));
    assert_spellings(&[
        (ApiItemKind::Function, "function"),
        (ApiItemKind::Method, "method"),
        (ApiItemKind::Type, "type"),
        (ApiItemKind::Interface, "interface"),
        (ApiItemKind::Constant, "constant"),
        (ApiItemKind::Variable, "variable"),
        (ApiItemKind::Module, "module"),
    ]);
}

#[test]
fn roots_payload_shape() {
    let json = assert_round_trip(&Roots {
        roots: vec![LiveRoot {
            symbol: key(),
            reason: RootReason::FrameworkHook,
        }],
    });
    assert_eq!(json["roots"][0]["reason"], json!("framework-hook"));
    assert_spellings(&[
        (RootReason::EntryPoint, "entry-point"),
        (RootReason::Export, "export"),
        (RootReason::Test, "test"),
        (RootReason::FrameworkHook, "framework-hook"),
    ]);
}

#[test]
fn test_conventions_payload_shape() {
    let json = assert_round_trip(&TestConventions {
        test_globs: vec!["tests/**/*.rs".into()],
        pairs: vec![TestPairing {
            source: "src/planner.rs".into(),
            tests: vec!["tests/planner_tests.rs".into()],
        }],
    });
    assert_eq!(
        json["pairs"][0]["tests"][0],
        json!("tests/planner_tests.rs")
    );
}

#[test]
fn project_info_payload_shape() {
    let json = assert_round_trip(&ProjectInfo {
        roots: vec![ProjectRoot {
            path: "svc".into(),
            lang: LangId::from("go"),
            build_files: vec!["go.work".into(), "api/go.mod".into()],
            members: vec!["api".into()],
            packages: vec!["./api/...".into()],
            skip_dirs: vec!["vendor".into()],
        }],
    });
    assert_eq!(json["roots"][0]["members"], json!(["api"]));
    assert_eq!(json["roots"][0]["packages"], json!(["./api/..."]));
    assert!(json["roots"][0].get("commands").is_none());
}

#[test]
fn symbol_and_visibility_kinds() {
    assert_spellings(&[
        (SymbolKind::Function, "function"),
        (SymbolKind::Method, "method"),
        (SymbolKind::Closure, "closure"),
    ]);
    assert_spellings(&[
        (Visibility::Public, "public"),
        (Visibility::Internal, "internal"),
        (Visibility::Private, "private"),
    ]);
}

#[test]
fn each_payload_declares_its_own_kind_and_version() {
    let declared = [
        (Symbols::KIND, Symbols::VERSION),
        (Decisions::KIND, Decisions::VERSION),
        (NormTree::KIND, NormTree::VERSION),
        (MutationSites::KIND, MutationSites::VERSION),
        (TestFrames::KIND, TestFrames::VERSION),
        (ModuleGraph::KIND, ModuleGraph::VERSION),
        (ApiSurface::KIND, ApiSurface::VERSION),
        (Roots::KIND, Roots::VERSION),
        (TestConventions::KIND, TestConventions::VERSION),
        (ProjectInfo::KIND, ProjectInfo::VERSION),
    ];
    assert_eq!(
        declared,
        [
            (FactKind::Symbols, 2),
            (FactKind::Decisions, 2),
            (FactKind::NormTree, 1),
            (FactKind::MutationSites, 1),
            (FactKind::TestFrames, 3),
            (FactKind::ModuleGraph, 2),
            (FactKind::ApiSurface, 1),
            (FactKind::Roots, 1),
            (FactKind::TestConventions, 1),
            (FactKind::ProjectInfo, 2),
        ]
    );
}

#[test]
fn test_conventions_answer_is_test_and_test_files_for() {
    let conventions = TestConventions {
        test_globs: vec!["**/tests/**/*.rs".to_owned(), "**/*_test.go".to_owned()],
        pairs: vec![TestPairing {
            source: "src/lib.rs".to_owned(),
            tests: vec!["tests/ledger.rs".to_owned()],
        }],
    };
    assert!(conventions.is_test("tests/ledger.rs"));
    assert!(conventions.is_test("crates/a/tests/deep/x.rs"));
    assert!(conventions.is_test("pkg/math_test.go"));
    assert!(conventions.is_test("math_test.go"));
    assert!(!conventions.is_test("src/lib.rs"));
    assert!(!conventions.is_test("src/attests/x.go"));
    assert_eq!(
        conventions.test_files_for("src/lib.rs"),
        ["tests/ledger.rs"]
    );
    assert!(conventions.test_files_for("src/other.rs").is_empty());
}

#[test]
fn a_normalized_node_carries_its_span_and_a_marker_its_text_only_when_known() {
    let bare = NormNode::new("call", vec![NormNode::leaf("$id")]);
    assert_eq!(
        serde_json::to_value(&bare).unwrap(),
        json!({"label": "call", "children": [{"label": "$id", "children": []}]})
    );
    let mut placed = bare.clone();
    placed.span = Some(span(3));
    placed.children[0].span = Some(span(3));
    placed.children[0].text = Some("rate".into());
    let json = assert_round_trip(&placed);
    assert_eq!(json["span"]["line"], json!(3));
    assert_eq!(json["children"][0]["text"], json!("rate"));
}

#[test]
fn a_node_shape_is_its_labels_and_children_without_spans_or_texts() {
    let bare = NormNode::new("call", vec![NormNode::leaf("$id")]);
    let mut placed = bare.clone();
    placed.span = Some(span(3));
    placed.children[0].text = Some("rate".into());
    assert!(bare.same_shape(&placed));
    assert_eq!(bare.shape_json(), placed.shape_json());
    // The shape serializes exactly as a node did before spans and texts existed, so every
    // structural hash taken over it is unchanged (ADR-133).
    assert_eq!(
        String::from_utf8(placed.shape_json()).unwrap(),
        r#"{"label":"call","children":[{"label":"$id","children":[]}]}"#
    );
    assert!(!bare.same_shape(&NormNode::new("call", vec![NormNode::leaf("$lit")])));
    assert!(!bare.same_shape(&NormNode::new("call", Vec::new())));
}

#[test]
fn an_import_edge_lists_the_names_it_imports_when_the_statement_names_them() {
    let edge = ImportEdge {
        from: "src/a.rs".into(),
        specifier: "super::beta::BETA".into(),
        target: ImportTarget::File {
            path: "src/beta.rs".into(),
        },
        relative: true,
        line: 1,
        edge_kind: EdgeKind::Import,
        names: vec!["BETA".into()],
    };
    let json = assert_round_trip(&edge);
    assert_eq!(json["names"], json!(["BETA"]));
    let whole = ImportEdge {
        names: Vec::new(),
        ..edge
    };
    assert!(assert_round_trip(&whole).get("names").is_none());
}
