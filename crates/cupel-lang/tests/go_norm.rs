// ABOUTME: The Go NormTree producer and structural hash: the normalized shape of a function or method,
// ABOUTME: and properties: invariant under renames, layout, literals, and moves; sensitive to structure.
mod support;

use cupel_core::syntax::{NormNode, NormTree, Symbols};
use proptest::prelude::*;
use support::{produce, tree};

/// `(label child …)`, or `label` for a leaf.
fn sexp(node: &NormNode) -> String {
    if node.children.is_empty() {
        node.label.clone()
    } else {
        let children: Vec<String> = node.children.iter().map(sexp).collect();
        format!("({} {})", node.label, children.join(" "))
    }
}

/// The normalized tree and hash of the only symbol in `source`, a Go file at `path`.
fn normalize_at(path: &str, source: &str) -> (String, String) {
    let dir = tree(&[(path, source)]);
    let norm: NormTree = produce("go", dir.path());
    let symbols: Symbols = produce("go", dir.path());
    assert_eq!(norm.symbols.len(), 1, "{source}");
    assert_eq!(norm.symbols[0].symbol, symbols.symbols[0].key());
    (
        sexp(&norm.symbols[0].root),
        norm.symbols[0].hash.to_string(),
    )
}

fn hash(source: &str) -> String {
    normalize_at("money.go", source).1
}

#[test]
fn identifiers_types_and_literals_become_markers_and_operators_and_calls_stay() {
    let (shape, hash) = normalize_at(
        "money.go",
        "package money\n\nfunc f(a Money) int64 {\n\t// a comment\n\treturn a.Cents(1) + 2\n}\n",
    );
    assert_eq!(
        shape,
        "(function_declaration (parameter_list (parameter_declaration $id $id)) $id \
         (block (statement_list (return_statement (expression_list (binary_expression \
         (call_expression (selector_expression $id Cents) (argument_list $lit)) + $lit))))))"
    );
    assert!(hash.starts_with("sh_") && hash.len() == 19, "{hash}");
}

#[test]
fn a_method_keeps_its_receiver_shape_but_not_its_receiver_names() {
    let (pointer, _) = normalize_at(
        "money.go",
        "package money\n\nfunc (m *Money) Add(c int64) { m.cents += c }\n",
    );
    assert_eq!(
        pointer,
        "(method_declaration (parameter_list (parameter_declaration $id (pointer_type * $id))) \
         (parameter_list (parameter_declaration $id $id)) \
         (block (statement_list (assignment_statement (expression_list (selector_expression $id $id)) += \
         (expression_list $id)))))"
    );
    let renamed = hash("package money\n\nfunc (w *Wallet) Plus(n int64) { w.total += n }\n");
    let value = hash("package money\n\nfunc (m Money) Add(c int64) { m.cents += c }\n");
    let original = hash("package money\n\nfunc (m *Money) Add(c int64) { m.cents += c }\n");
    assert_eq!(renamed, original);
    assert_ne!(value, original);
}

#[test]
fn package_qualified_calls_keep_the_function_name() {
    let (shape, _) = normalize_at(
        "money.go",
        "package money\n\nfunc f(s string) string { return strings.ToUpper(s) }\n",
    );
    assert!(
        shape.contains("(call_expression (selector_expression $id ToUpper) (argument_list $id))"),
        "{shape}"
    );
}

#[test]
fn moving_a_file_keeps_its_hashes() {
    let source = "package money\n\nfunc F(a int64) int64 { return a * 2 }\n";
    assert_eq!(
        normalize_at("money.go", source).1,
        normalize_at("internal/deep/place.go", source).1
    );
}

#[test]
fn hashes_change_with_structure() {
    let base = hash(
        "package p\n\nfunc f(a, b int64) int64 {\n\tif a > b {\n\t\treturn a - b\n\t}\n\treturn g(a)\n}\n",
    );
    for changed in [
        "package p\n\nfunc f(a, b int64) int64 {\n\tif a < b {\n\t\treturn a - b\n\t}\n\treturn g(a)\n}\n",
        "package p\n\nfunc f(a, b int64) int64 {\n\tif a > b {\n\t\treturn a + b\n\t}\n\treturn g(a)\n}\n",
        "package p\n\nfunc f(a, b int64) int64 {\n\tif a > b {\n\t\treturn a - b\n\t}\n\treturn h(a)\n}\n",
        "package p\n\nfunc f(a, b int64) int64 {\n\tif a > b {\n\t\treturn a - b\n\t} else if a == 0 {\n\t\treturn 0\n\t}\n\treturn g(a)\n}\n",
        "package p\n\nfunc f(a int64) int64 {\n\tif a > 0 {\n\t\treturn a - 1\n\t}\n\treturn g(a)\n}\n",
    ] {
        assert_ne!(hash(changed), base, "{changed}");
    }
}

/// A function whose names, literals, and layout are all chosen by the caller. Go inserts
/// semicolons at line ends, so line breaks go only where a statement or list may break.
fn template(names: &[String; 4], literal: u32, space: &str, newline: &str, wrap: &str) -> String {
    let [function, a, b, local] = names;
    format!(
        "package p\n\nfunc {function}({a} int64,{wrap}{b} int64) int64 {{{newline}\
         {local} := {a}{space}*{space}{literal}{newline}\
         if {local} > {b} {{ return min({local}, {b}) }}{newline}\
         return {b}{space}-{space}{local}{newline}}}\n"
    )
}

fn identifier() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,8}".prop_filter("not a keyword", |name| {
        !matches!(
            name.as_str(),
            "break"
                | "case"
                | "chan"
                | "const"
                | "continue"
                | "default"
                | "defer"
                | "else"
                | "fallthrough"
                | "for"
                | "func"
                | "go"
                | "goto"
                | "if"
                | "import"
                | "interface"
                | "map"
                | "package"
                | "range"
                | "return"
                | "select"
                | "struct"
                | "switch"
                | "type"
                | "var"
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
        space in prop::sample::select(vec![" ", "  ", "\t"]),
        newline in prop::sample::select(vec!["\n", "\n\n", "\n\t\t", "\n// note\n"]),
        wrap in prop::sample::select(vec![" ", "\n\t", " /* b */ "]),
    ) {
        let base = template(
            &["f".to_owned(), "a".to_owned(), "b".to_owned(), "c".to_owned()],
            7,
            " ",
            "\n",
            " ",
        );
        prop_assert_eq!(hash(&template(&names, literal, space, newline, wrap)), hash(&base));
    }
}
