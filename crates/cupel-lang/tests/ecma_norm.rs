// ABOUTME: The JavaScript and TypeScript NormTree producers: normalized shapes, a JS function and its
// ABOUTME: TS-annotated twin normalizing alike, and hash invariance under renames, literals, layout, moves.
mod support;

use cupel_core::syntax::{NormTree, Symbols};
use proptest::prelude::*;
use support::{assert_distinct, fixture, normalize_at, produce};

fn js(source: &str) -> (String, String) {
    normalize_at("javascript", "src/a.js", source)
}

fn ts(source: &str) -> (String, String) {
    normalize_at("typescript", "src/a.ts", source)
}

#[test]
fn identifiers_and_literals_become_markers_and_operators_and_call_names_stay() {
    let (shape, hash) =
        js("function f(a) {\n  // a comment\n  return this.items.push(a) + `n=${a}`;\n}\n");
    assert_eq!(
        shape,
        "(function_declaration (formal_parameters $id) (statement_block (return_statement \
         (binary_expression (call_expression (member_expression (member_expression this $id) push) \
         (arguments $id)) + $lit))))"
    );
    assert!(hash.starts_with("sh_") && hash.len() == 19, "{hash}");
}

#[test]
fn constructors_keep_their_class_name_and_word_operators_are_kept() {
    let (shape, _) = js("const f = (a) => new Error(typeof a);\n");
    assert_eq!(
        shape,
        "(variable_declarator = (arrow_function (formal_parameters $id) => \
         (new_expression Error (arguments (unary_expression typeof $id)))))"
    );
}

#[test]
fn a_javascript_function_and_its_typescript_annotated_twin_normalize_alike() {
    let javascript: NormTree = produce("javascript", &fixture("javascript"));
    let typescript: NormTree = produce("typescript", &fixture("typescript"));
    let js_symbols: Symbols = produce("javascript", &fixture("javascript"));
    let ts_symbols: Symbols = produce("typescript", &fixture("typescript"));
    let tree_of = |norm: &NormTree, symbols: &Symbols, display: &str| {
        let key = symbols
            .symbols
            .iter()
            .find(|s| s.display == display)
            .unwrap_or_else(|| panic!("no symbol {display}"))
            .key();
        norm.symbols
            .iter()
            .find(|s| s.symbol == key)
            .unwrap()
            .clone()
    };
    for display in [
        "twins.netTotal",
        "twins.sumAfterCharge",
        "twins.netTotalLaidOut",
        "decide.classify",
        "decide.parseCents",
        "math.add",
        "math.range",
    ] {
        let javascript = tree_of(&javascript, &js_symbols, display);
        let typescript = tree_of(&typescript, &ts_symbols, display);
        assert_eq!(
            javascript.root.shape_json(),
            typescript.root.shape_json(),
            "{display}"
        );
        // The language is part of the hash (ADR-025): equal trees, different identities.
        assert_ne!(javascript.hash, typescript.hash, "{display}");
    }
}

#[test]
fn type_annotations_generics_decorators_and_modifiers_do_not_change_a_typescript_hash() {
    let plain = ts("class A {\n  m(a, b = 1) {\n    const c = a.get(b);\n    return c;\n  }\n}\n");
    let typed = ts(
        "class A {\n  @memo()\n  public override m<T>(a: Map<number, T>, b: number = 1): T | undefined {\n    const c: T | undefined = a.get<T>(b);\n    return c;\n  }\n}\n",
    );
    assert_eq!(typed, plain);
    let javascript =
        js("class A {\n  m(a, b = 1) {\n    const c = a.get(b);\n    return c;\n  }\n}\n");
    assert_eq!(javascript.0, plain.0);
}

#[test]
fn a_tsx_component_and_its_jsx_twin_normalize_alike() {
    let jsx = normalize_at(
        "javascript",
        "src/badge.jsx",
        "export function Badge({ label }) {\n  return <span className=\"badge\">{label}</span>;\n}\n",
    );
    let tsx = normalize_at(
        "typescript",
        "src/badge.tsx",
        "export function Badge({ label }: { label: string }): JSX.Element {\n  return <span className=\"badge\">{label}</span>;\n}\n",
    );
    assert_eq!(jsx.0, tsx.0);
}

#[test]
fn moving_a_file_keeps_its_hashes() {
    let source = "export function f(a) { return a * 2; }\n";
    assert_eq!(
        normalize_at("javascript", "src/a.js", source).1,
        normalize_at("javascript", "packages/other/lib/deep/place.mjs", source).1
    );
}

#[test]
fn hashes_change_with_structure() {
    let sources = [
        "function f(a, b) { if (a > b) { return a - b; } return g(a); }\n",
        "function f(a, b) { if (a < b) { return a - b; } return g(a); }\n",
        "function f(a, b) { if (a > b) { return a + b; } return g(a); }\n",
        "function f(a, b) { if (a > b) { return a - b; } return h(a); }\n",
        "function f(a, b) { if (a > b) { return a - b; } else if (a === 0) { return 0; } return g(a); }\n",
        "function f(a, b) { if (a > b) { return a - b; } return g?.(a); }\n",
        "function f(a, b) { if (a > b) { return a - b; } return typeof g(a); }\n",
    ];
    assert_distinct(&sources.map(|source| js(source).1));
}

/// A function whose names, literals, layout, and annotations are chosen by the caller.
fn template(names: &[String; 4], literal: u32, space: &str, newline: &str, typed: bool) -> String {
    let [function, a, b, local] = names;
    let annotation = if typed { ": number" } else { "" };
    format!(
        "function {function}({a}{annotation},{space}{b}{annotation}){annotation} {{{newline}\
         const {local}{annotation} = {a}{space}*{space}{literal};{newline}\
         if ({local} > {b}) {{ return Math.min({local}, {b}); }}{newline}\
         return {b} - {local}{newline}}}\n"
    )
}

/// Reserved words and contextual keywords of JavaScript and TypeScript, which are not names.
const KEYWORDS: &[&str] = &[
    "as",
    "do",
    "if",
    "in",
    "is",
    "of",
    "for",
    "let",
    "new",
    "try",
    "var",
    "case",
    "else",
    "enum",
    "null",
    "this",
    "true",
    "type",
    "void",
    "with",
    "async",
    "await",
    "break",
    "catch",
    "class",
    "const",
    "false",
    "super",
    "throw",
    "while",
    "yield",
    "delete",
    "export",
    "import",
    "public",
    "return",
    "static",
    "switch",
    "typeof",
    "default",
    "extends",
    "finally",
    "package",
    "private",
    "continue",
    "debugger",
    "function",
    "abstract",
    "readonly",
    "interface",
    "protected",
    "implements",
    "instanceof",
    "undefined",
    "declare",
    "namespace",
    "module",
    "keyof",
    "infer",
    "unique",
    "never",
    "unknown",
    "any",
    "get",
    "set",
    "from",
    "require",
    "global",
    "satisfies",
    "override",
    "accessor",
    "asserts",
    "constructor",
];

fn identifier() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,8}".prop_filter("not a keyword", |name| !KEYWORDS.contains(&name.as_str()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Renaming the function, its parameters, and its locals; changing a literal; changing
    /// whitespace and line breaks; and, in TypeScript, adding annotations never change the hash.
    #[test]
    fn the_hash_is_invariant_under_renames_literals_layout_and_annotations(
        names in prop::array::uniform4(identifier())
            .prop_filter("distinct names", |n| {
                n[0] != n[1] && n[0] != n[2] && n[0] != n[3] && n[1] != n[2] && n[1] != n[3] && n[2] != n[3]
            }),
        literal in 0u32..1000,
        space in prop::sample::select(vec![" ", "  ", "\t", " \n    "]),
        newline in prop::sample::select(vec!["\n", "\n\n", " ", "\n        "]),
        typed in any::<bool>(),
    ) {
        let base = |typed| template(
            &["f".to_owned(), "a".to_owned(), "b".to_owned(), "c".to_owned()],
            7,
            " ",
            "\n",
            typed,
        );
        prop_assert_eq!(js(&template(&names, literal, space, newline, false)).1, js(&base(false)).1);
        prop_assert_eq!(ts(&template(&names, literal, space, newline, typed)).1, ts(&base(false)).1);
    }
}
