// ABOUTME: The Rust NormTree producer and structural hash: the normalized shape of a function, and
// ABOUTME: properties: invariant under renames, layout, literals, and moves; sensitive to structure.
mod support;

use cupel_core::syntax::{NormNode, NormTree, Span, Symbols};
use proptest::prelude::*;
use support::{assert_distinct, normalize_at, produce, sexp, tree};

/// The normalized tree and hash of the only symbol in `source`, a Rust file at `path`.
fn hash(source: &str) -> String {
    normalize_at("rust", "src/lib.rs", source).1
}

#[test]
fn identifiers_and_literals_become_markers_and_operators_and_calls_stay() {
    let (shape, hash) = normalize_at(
        "rust",
        "src/lib.rs",
        "pub fn f(a: i64) -> i64 {\n    // a comment\n    a.max(1) + 2\n}\n",
    );
    assert_eq!(
        shape,
        "(function_item (parameters (parameter $id primitive_type)) -> primitive_type \
         (block (binary_expression (call_expression (field_expression $id max) \
         (arguments $lit)) + $lit)))"
    );
    assert!(hash.starts_with("sh_") && hash.len() == 19, "{hash}");
    assert!(
        hash[3..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
}

#[test]
fn macro_names_and_path_calls_are_kept_as_call_names() {
    let (shape, _) = normalize_at(
        "rust",
        "src/lib.rs",
        "fn f() -> Vec<String> {\n    println!(\"x\");\n    Vec::new()\n}\n",
    );
    assert_eq!(
        shape,
        "(function_item parameters -> (generic_type $id (type_arguments < $id >)) (block \
         (expression_statement (macro_invocation println ! (token_tree $lit))) \
         (call_expression (scoped_identifier Vec new) arguments)))"
    );
}

#[test]
fn every_symbol_gets_a_tree_in_symbol_order_and_nested_symbols_stay_in_their_parent() {
    let dir = tree(&[(
        "src/lib.rs",
        "fn outer() -> i32 {\n    fn inner() -> i32 { 1 }\n    inner() + 1\n}\nfn other() {}\n",
    )]);
    let norm: NormTree = produce("rust", dir.path());
    let symbols: Symbols = produce("rust", dir.path());
    let keys: Vec<_> = norm.symbols.iter().map(|s| s.symbol.clone()).collect();
    let expected: Vec<_> = symbols
        .symbols
        .iter()
        .map(cupel_core::syntax::SymbolDef::key)
        .collect();
    assert_eq!(keys, expected);
    let outer = sexp(&norm.symbols[0].root);
    assert!(
        outer.contains("(function_item $id parameters -> primitive_type (block $lit))"),
        "{outer}"
    );
}

#[test]
fn moving_a_file_keeps_its_hashes() {
    let source = "pub fn f(a: i64) -> i64 { a * 2 }\n";
    assert_eq!(
        normalize_at("rust", "src/lib.rs", source).1,
        normalize_at("rust", "crates/other/src/deep/place.rs", source).1
    );
}

#[test]
fn hashes_change_with_structure() {
    let sources = [
        "fn f(a: i64, b: i64) -> i64 { if a > b { a - b } else { g(a) } }\n",
        "fn f(a: i64, b: i64) -> i64 { if a < b { a - b } else { g(a) } }\n",
        "fn f(a: i64, b: i64) -> i64 { if a > b { a + b } else { g(a) } }\n",
        "fn f(a: i64, b: i64) -> i64 { if a > b { a - b } else { h(a) } }\n",
        "fn f(a: i64, b: i64) -> i64 { if a > b { a - b } else if a == 0 { 0 } else { g(a) } }\n",
        "fn f(a: i64) -> i64 { if a > 0 { a - 1 } else { g(a) } }\n",
    ];
    assert_distinct(&sources.map(hash));
}

/// A function whose names, literals, and layout are all chosen by the caller.
fn template(names: &[String; 4], literal: u32, space: &str, newline: &str) -> String {
    let [function, a, b, local] = names;
    format!(
        "fn {function}({a}: i64,{space}{b}: i64) -> i64 {{{newline}\
         let {local} = {a}{space}*{space}{literal};{newline}\
         if {local} > {b} {{ {local}.min({b}) }} else {{{newline}{b} - {local} }}{newline}}}\n"
    )
}

fn identifier() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,8}".prop_filter("not a keyword", |name| {
        !matches!(
            name.as_str(),
            "as" | "fn"
                | "if"
                | "in"
                | "let"
                | "loop"
                | "mod"
                | "mut"
                | "ref"
                | "use"
                | "else"
                | "enum"
                | "impl"
                | "move"
                | "pub"
                | "self"
                | "type"
                | "for"
                | "while"
                | "match"
                | "true"
                | "false"
                | "where"
                | "const"
                | "crate"
                | "super"
                | "trait"
                | "struct"
                | "static"
                | "return"
                | "break"
                | "unsafe"
                | "extern"
                | "async"
                | "await"
                | "dyn"
                | "box"
                | "do"
                | "try"
                | "yield"
                | "macro"
                | "priv"
                | "final"
                | "gen"
                | "continue"
                | "virtual"
                | "override"
                | "typeof"
                | "unsized"
                | "abstract"
                | "become"
                | "union"
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Renaming the function, its parameters, and its locals; changing a literal; and changing
    /// whitespace and line breaks never change the hash.
    #[test]
    fn the_hash_is_invariant_under_renames_literals_and_layout(
        names in prop::array::uniform4(identifier())
            .prop_filter("distinct names", |n| {
                n[0] != n[1] && n[0] != n[2] && n[0] != n[3] && n[1] != n[2] && n[1] != n[3] && n[2] != n[3]
            }),
        literal in 0u32..1000,
        space in prop::sample::select(vec![" ", "  ", "\t", " \n    "]),
        newline in prop::sample::select(vec!["\n", "\n\n", " ", "\n        "]),
    ) {
        let base = template(
            &["f".to_owned(), "a".to_owned(), "b".to_owned(), "c".to_owned()],
            7,
            " ",
            "\n",
        );
        prop_assert_eq!(hash(&template(&names, literal, space, newline)), hash(&base));
    }
}

/// Every node of the tree rooted at `node`, in pre-order.
fn nodes(node: &NormNode) -> Vec<&NormNode> {
    std::iter::once(node)
        .chain(node.children.iter().flat_map(nodes))
        .collect()
}

#[test]
fn every_node_carries_its_span_and_a_marker_its_source_text() {
    let dir = tree(&[(
        "src/lib.rs",
        "pub fn f(rate: i64) -> i64 {\n    rate.max(1) + 20\n}\n",
    )]);
    let norm: NormTree = produce("rust", dir.path());
    let symbols: Symbols = produce("rust", dir.path());
    let root = &norm.symbols[0].root;
    assert_eq!(root.span, Some(symbols.symbols[0].span));
    let all = nodes(root);
    assert!(all.iter().all(|node| node.span.is_some()), "{root:?}");
    let texts: Vec<(&str, &str, u32)> = all
        .iter()
        .filter_map(|node| {
            let span: Span = node.span?;
            Some((node.label.as_str(), node.text.as_deref()?, span.line))
        })
        .collect();
    assert_eq!(
        texts,
        [
            ("$id", "rate", 1),
            ("$id", "rate", 2),
            ("$lit", "1", 2),
            ("$lit", "20", 2)
        ]
    );
}
