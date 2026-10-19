// ABOUTME: The Python NormTree producer and structural hash: annotations, decorators, docstrings, and
// ABOUTME: `self`/`cls` receivers normalize away; renames, literals, layout, and moves keep the hash.
mod support;

use cupel_core::syntax::{NormTree, Symbols};
use proptest::prelude::*;
use support::{assert_distinct, produce, sexp, tree};

/// (display, normalized tree, hash) of every symbol in `source`, a Python file at `path`.
fn normalize_all(path: &str, source: &str) -> Vec<(String, String, String)> {
    let dir = tree(&[(path, source)]);
    let norm: NormTree = produce("python", dir.path());
    let symbols: Symbols = produce("python", dir.path());
    assert_eq!(norm.symbols.len(), symbols.symbols.len(), "{source}");
    norm.symbols
        .iter()
        .zip(&symbols.symbols)
        .map(|(normalized, symbol)| {
            assert_eq!(normalized.symbol, symbol.key());
            (
                symbol.display.clone(),
                sexp(&normalized.root),
                normalized.hash.to_string(),
            )
        })
        .collect()
}

/// The normalized tree and hash of the only symbol in `source`.
fn normalize_at(path: &str, source: &str) -> (String, String) {
    let mut all = normalize_all(path, source);
    assert_eq!(all.len(), 1, "{source}");
    let (_, shape, hash) = all.remove(0);
    (shape, hash)
}

fn hash(source: &str) -> String {
    normalize_at("m.py", source).1
}

/// The hash of the symbol `display` in `source`.
fn hash_of(source: &str, display: &str) -> String {
    normalize_all("m.py", source)
        .into_iter()
        .find(|(d, _, _)| d == display)
        .unwrap_or_else(|| panic!("no symbol {display}"))
        .2
}

#[test]
fn identifiers_and_literals_become_markers_and_operators_and_calls_stay() {
    let (shape, hash) = normalize_at(
        "m.py",
        "def f(a: int) -> int:\n    # a comment\n    return max(a, 1) + 2\n",
    );
    assert_eq!(
        shape,
        "(function_definition (parameters $id) (block (return_statement \
         (binary_operator (call max (argument_list $id $lit)) + $lit))))"
    );
    assert!(hash.starts_with("sh_") && hash.len() == 19, "{hash}");
}

#[test]
fn method_names_word_operators_and_f_strings() {
    let (shape, _) = normalize_at(
        "m.py",
        "def f(xs, y):\n    xs.items.append(f\"{y!r}\")\n    return y is not None and y not in xs\n",
    );
    assert_eq!(
        shape,
        "(function_definition (parameters $id $id) (block (expression_statement (call \
         (attribute (attribute $id $id) append) (argument_list $lit))) (return_statement \
         (boolean_operator (comparison_operator $id is not $lit) and (comparison_operator \
         $id not in $id)))))"
    );
}

#[test]
fn annotations_defaults_decorators_and_docstrings_do_not_change_the_hash() {
    let plain = hash("def f(a, b=\"x\"):\n    return a + len(b)\n");
    for variant in [
        "def f(a: int, b: str = \"x\") -> int:\n    return a + len(b)\n",
        "@cache\ndef f(a, b=\"x\"):\n    return a + len(b)\n",
        "def f(a, b=\"x\"):\n    \"\"\"Add the length of b.\"\"\"\n    return a + len(b)\n",
        "def f(a, b='y'):\n    '''Docs.'''\n\n    return a + len(b)\n",
    ] {
        assert_eq!(hash(variant), plain, "{variant}");
    }
}

#[test]
fn a_method_hashes_like_the_free_function_with_its_body_once_self_or_cls_is_dropped() {
    let source = "def total(entries, fee):\n    return sum(entries) - fee\n\n\n\
                  class Till:\n    def total(self, entries, fee):\n        return sum(entries) - fee\n\n    \
                  @classmethod\n    def tally(cls, entries, fee):\n        return sum(entries) - fee\n\n    \
                  @staticmethod\n    def fixed(entries, fee):\n        return sum(entries) - fee\n\n    \
                  @staticmethod\n    def odd(first, entries, fee):\n        return sum(entries) - fee\n";
    let free = hash_of(source, "m.total");
    assert_eq!(hash_of(source, "m.Till.total"), free);
    assert_eq!(hash_of(source, "m.Till.tally"), free);
    assert_eq!(hash_of(source, "m.Till.fixed"), free);
    assert_ne!(hash_of(source, "m.Till.odd"), free);
}

/// Python's `NormTree` override replaces the table-driven one (`generic::register_except`);
/// every syntactic fact must still have exactly one producer, so the override neither doubles
/// `NormTree` nor drops another generic producer.
#[test]
fn python_has_one_producer_for_every_syntactic_fact() {
    let languages = cupel_lang::LanguageRegistry::builtin();
    let mut producers = cupel_core::ProducerRegistry::default();
    languages.register_producers(&mut producers);
    let python = cupel_lang::lang(languages.get(&"python".into()).unwrap().as_ref());
    let not_one: Vec<(cupel_core::FactKind, usize)> = cupel_core::FactKind::syntactic()
        .iter()
        .map(|kind| (*kind, producers.producers_for(*kind, &python).count()))
        .filter(|(_, count)| *count != 1)
        .collect();
    assert_eq!(not_one, []);
}

#[test]
fn a_parameter_called_self_is_kept_outside_a_class() {
    assert_distinct(
        &[
            "def f(self, a):\n    return a\n",
            "def f(a):\n    return a\n",
        ]
        .map(hash),
    );
}

#[test]
fn every_symbol_gets_a_tree_in_symbol_order_and_nested_symbols_stay_in_their_parent() {
    let all = normalize_all(
        "m.py",
        "def outer():\n    def inner():\n        return 1\n    return inner() + 1\n\n\ndef other():\n    pass\n",
    );
    let names: Vec<&str> = all.iter().map(|(d, _, _)| d.as_str()).collect();
    assert_eq!(names, ["m.outer", "m.outer.inner", "m.other"]);
    assert!(
        all[0]
            .1
            .contains("(function_definition $id parameters (block (return_statement $lit)))"),
        "{}",
        all[0].1
    );
}

#[test]
fn moving_a_file_keeps_its_hashes() {
    let source = "def f(a):\n    return a * 2\n";
    assert_eq!(
        normalize_at("m.py", source).1,
        normalize_at("src/deep/pkg/place.py", source).1
    );
}

#[test]
fn hashes_change_with_structure() {
    let sources = [
        "def f(a, b):\n    if a > b:\n        return a - b\n    return g(a)\n",
        "def f(a, b):\n    if a < b:\n        return a - b\n    return g(a)\n",
        "def f(a, b):\n    if a > b:\n        return a + b\n    return g(a)\n",
        "def f(a, b):\n    if a > b:\n        return a - b\n    return h(a)\n",
        "def f(a, b):\n    if a > b:\n        return a - b\n    elif a:\n        return 0\n    return g(a)\n",
        "def f(a, b):\n    if a > b and b:\n        return a - b\n    return g(a)\n",
        "def f(a):\n    if a > 0:\n        return a - 1\n    return g(a)\n",
    ];
    assert_distinct(&sources.map(hash));
    assert_ne!(
        hash("def f(a):\n    return a is None\n"),
        hash("def f(a):\n    return a is not None\n")
    );
    assert_ne!(
        hash("def f(a, b):\n    return a and b\n"),
        hash("def f(a, b):\n    return a or b\n")
    );
}

/// A function whose names, literals, and layout are all chosen by the caller.
fn template(names: &[String; 4], literal: u32, indent: &str, space: &str, blank: &str) -> String {
    let [function, a, b, local] = names;
    format!(
        "def {function}({a},{space}{b}):{blank}\n\
         {indent}{local}{space}={space}{a}{space}*{space}{literal}  # scaled\n{blank}\
         {indent}if {local}{space}>{space}{b}:\n\
         {indent}{indent}return min({local},{space}{b})\n{blank}\
         {indent}else:\n\
         {indent}{indent}return {b}{space}-{space}{local}\n"
    )
}

fn identifier() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,8}".prop_filter("not a keyword or builtin", |name| {
        !matches!(
            name.as_str(),
            "and"
                | "as"
                | "assert"
                | "async"
                | "await"
                | "break"
                | "case"
                | "class"
                | "cls"
                | "continue"
                | "def"
                | "del"
                | "elif"
                | "else"
                | "except"
                | "exec"
                | "finally"
                | "for"
                | "from"
                | "global"
                | "if"
                | "import"
                | "in"
                | "is"
                | "lambda"
                | "match"
                | "min"
                | "nonlocal"
                | "not"
                | "or"
                | "pass"
                | "print"
                | "raise"
                | "return"
                | "self"
                | "try"
                | "type"
                | "while"
                | "with"
                | "yield"
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Renaming the function, its parameters, and its locals; changing a literal; and changing
    /// the indentation unit, spacing, comments, and blank lines never change the hash.
    #[test]
    fn the_hash_is_invariant_under_renames_literals_and_layout(
        names in prop::array::uniform4(identifier())
            .prop_filter("distinct names", |n| {
                n[0] != n[1] && n[0] != n[2] && n[0] != n[3] && n[1] != n[2] && n[1] != n[3] && n[2] != n[3]
            }),
        literal in 0u32..1000,
        indent in prop::sample::select(vec!["    ", "  ", "\t", "        "]),
        space in prop::sample::select(vec![" ", "  ", "\t"]),
        blank in prop::sample::select(vec!["", "\n", "\n    \n"]),
    ) {
        let base = template(
            &["f".to_owned(), "a".to_owned(), "b".to_owned(), "c".to_owned()],
            7,
            "    ",
            " ",
            "",
        );
        prop_assert_eq!(hash(&template(&names, literal, indent, space, blank)), hash(&base));
    }
}
