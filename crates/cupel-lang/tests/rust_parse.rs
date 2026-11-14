// ABOUTME: Rust constructs rustc accepts and the upstream grammar rejected (ADR-213): a macro named like a primitive
// ABOUTME: type (snapbox's `str!`, a local `u8!`) is no parse error, and `str` and `u8` keep their meaning as types.
mod support;

use support::parse_errors;

/// Macros named like primitive types: in a `let`, in a call's argument with a raw string, and as a statement.
const MACROS: &str = r##"fn f(x: u8) {
    let m = str!("a");
    check(x, str![[r#"
expected
"#]]);
    u8!(x);
}
"##;

/// `str` and `u8` as types, paths, and a qualified self type, which the patch leaves as they were.
const TYPES: &str = r#"fn g(x: u8) -> &'static str {
    let s: &str = "";
    let m = u8::MAX;
    let t = <str as ToString>::to_string;
    s
}
"#;

#[test]
fn a_macro_named_like_a_primitive_type_is_not_a_parse_error() {
    let files = [("macros.rs", MACROS), ("types.rs", TYPES)];
    assert_eq!(parse_errors("rust", &files), Vec::<String>::new());
}

/// The syntax tree of `source` under Cupel's Rust grammar.
fn sexp(source: &str) -> String {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();
    parser.parse(source, None).unwrap().root_node().to_sexp()
}

#[test]
fn a_macro_named_like_a_primitive_type_names_it_with_an_identifier() {
    assert_eq!(
        sexp("fn f() { let m = str!(\"a\"); u8!(x); }"),
        "(source_file (function_item name: (identifier) parameters: (parameters) body: (block \
         (let_declaration pattern: (identifier) value: (macro_invocation macro: (identifier) \
         (token_tree (string_literal (string_content))))) (expression_statement (macro_invocation \
         macro: (identifier) (token_tree (identifier)))))))"
    );
}

#[test]
fn str_and_u8_stay_primitive_types_where_they_are_types() {
    assert_eq!(
        sexp("fn g(x: u8) { let s: &str = \"\"; let t = <str as ToString>::to_string; }"),
        "(source_file (function_item name: (identifier) parameters: (parameters (parameter \
         pattern: (identifier) type: (primitive_type))) body: (block (let_declaration pattern: \
         (identifier) type: (reference_type type: (primitive_type)) value: (string_literal)) \
         (let_declaration pattern: (identifier) value: (scoped_identifier path: (bracketed_type \
         (qualified_type type: (primitive_type) alias: (type_identifier))) name: (identifier))))))"
    );
}
