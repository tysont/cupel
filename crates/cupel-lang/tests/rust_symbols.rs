// ABOUTME: The Rust Symbols producer against the Rust fixture repository: every function, method,
// ABOUTME: nested function, and named closure, with namespace, visibility, kind, and lines.
mod support;

use cupel_core::syntax::{SymbolKind, Symbols, Visibility};
use support::{fixture, lines, produce, tree};

type Row = (&'static str, SymbolKind, Visibility, (u32, u32));

fn rows(symbols: &Symbols, file: &str) -> Vec<(String, SymbolKind, Visibility, (u32, u32))> {
    symbols
        .symbols
        .iter()
        .filter(|symbol| symbol.file == file)
        .map(|s| (s.display.clone(), s.kind, s.visibility, lines(s.span)))
        .collect()
}

fn expect(symbols: &Symbols, file: &str, expected: &[Row]) {
    let expected: Vec<(String, SymbolKind, Visibility, (u32, u32))> = expected
        .iter()
        .map(|(d, k, v, l)| ((*d).to_owned(), *k, *v, *l))
        .collect();
    assert_eq!(rows(symbols, file), expected, "{file}");
}

use SymbolKind::{Closure, Function, Method};
use Visibility::{Internal, Private, Public};

/// The 1-based line of the first line of `file` in the Rust fixture that contains `needle`.
/// The crate root gains a `mod` line with every fixture module, so its lines are looked up.
fn line_of(file: &str, needle: &str) -> u32 {
    let text = std::fs::read_to_string(fixture("rust").join(file)).unwrap();
    let index = text.lines().position(|line| line.contains(needle)).unwrap();
    u32::try_from(index + 1).unwrap()
}

#[test]
fn the_crate_root_has_top_level_functions_of_each_visibility() {
    let symbols: Symbols = produce("rust", &fixture("rust"));
    let at = |needle: &str| {
        let line = line_of("src/lib.rs", needle);
        (line, line + 2)
    };
    let rows = [
        ("balance", Function, Public, at("pub fn balance")),
        ("audit_limit", Function, Internal, at("fn audit_limit")),
        ("scale", Function, Private, at("fn scale")),
    ];
    expect(&symbols, "src/lib.rs", &rows);
}

#[test]
fn impl_blocks_trait_impls_and_trait_defaults_are_methods_on_their_type() {
    let symbols: Symbols = produce("rust", &fixture("rust"));
    expect(
        &symbols,
        "src/account.rs",
        &[
            ("account::Account::new", Method, Public, (11, 16)),
            ("account::Account::deposit", Method, Public, (18, 20)),
            ("account::Account::statement", Method, Public, (22, 28)),
            (
                "account::Account::statement::line",
                Function,
                Private,
                (23, 25),
            ),
            (
                "account::Account::statement::header",
                Closure,
                Private,
                (26, 26),
            ),
            ("account::Account::fmt", Method, Public, (32, 34)),
            ("account::Audited::is_clean", Method, Public, (40, 42)),
        ],
    );
}

#[test]
fn module_files_take_their_namespace_from_their_path() {
    let symbols: Symbols = produce("rust", &fixture("rust"));
    expect(
        &symbols,
        "src/report/mod.rs",
        &[("report::summary", Function, Public, (3, 5))],
    );
    expect(
        &symbols,
        "src/report/format.rs",
        &[("report::format::render", Function, Public, (1, 3))],
    );
}

#[test]
fn symbols_carry_their_name_namespace_language_and_body() {
    let symbols: Symbols = produce("rust", &fixture("rust"));
    let render = symbols
        .symbols
        .iter()
        .find(|s| s.display == "report::format::render")
        .unwrap();
    assert_eq!(render.name, "render");
    assert_eq!(render.namespace, "report::format");
    assert_eq!(render.lang.as_str(), "rust");
    assert_eq!((render.span.col, render.span.end_col), (1, 2));
    let body = render.body.unwrap();
    assert_eq!(
        (body.line, body.col, body.end_line, body.end_col),
        (1, 43, 3, 2)
    );
}

#[test]
fn inline_modules_generic_impls_and_restricted_visibility() {
    let dir = tree(&[(
        "src/lib.rs",
        "pub struct Stack<T>(Vec<T>);\n\
         impl<T: Clone> Stack<T> {\n    pub(super) fn peek(&self) -> Option<T> { self.0.last().cloned() }\n}\n\
         mod tests {\n    #[test]\n    fn peeks() {}\n}\n\
         pub(self) fn hidden() {}\n",
    )]);
    let symbols: Symbols = produce("rust", dir.path());
    expect(
        &symbols,
        "src/lib.rs",
        &[
            ("Stack::peek", Method, Internal, (3, 3)),
            ("tests::peeks", Function, Private, (7, 7)),
            ("hidden", Function, Private, (9, 9)),
        ],
    );
}

#[test]
fn files_outside_src_are_namespaced_by_their_whole_path() {
    let dir = tree(&[("tests/ledger.rs", "fn checks() {}\n")]);
    let symbols: Symbols = produce("rust", dir.path());
    expect(
        &symbols,
        "tests/ledger.rs",
        &[("tests::ledger::checks", Function, Private, (1, 1))],
    );
}
