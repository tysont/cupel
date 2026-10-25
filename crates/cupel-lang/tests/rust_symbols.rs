// ABOUTME: The Rust Symbols producer against the Rust fixture repository: every function, method,
// ABOUTME: nested function, and named closure, with namespace, visibility, kind, and lines.
mod support;

use cupel_core::syntax::{SymbolKind, Symbols, Visibility};
use support::{assert_symbols, fixture, produce, tree};

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
    assert_symbols(&symbols, "src/lib.rs", &rows);
}

#[test]
fn impl_blocks_trait_impls_and_trait_defaults_are_methods_on_their_type() {
    let symbols: Symbols = produce("rust", &fixture("rust"));
    assert_symbols(
        &symbols,
        "src/account.rs",
        &[
            ("account::Account::new", Method, Public, (14, 19)),
            ("account::Account::deposit", Method, Public, (21, 23)),
            ("account::Account::statement", Method, Public, (25, 31)),
            (
                "account::Account::statement::line",
                Function,
                Private,
                (26, 28),
            ),
            (
                "account::Account::statement::header",
                Closure,
                Private,
                (29, 29),
            ),
            ("account::Account::fmt", Method, Public, (35, 37)),
            ("account::Audited::is_clean", Method, Public, (43, 45)),
        ],
    );
}

#[test]
fn module_files_take_their_namespace_from_their_path() {
    let symbols: Symbols = produce("rust", &fixture("rust"));
    assert_symbols(
        &symbols,
        "src/report/mod.rs",
        &[
            ("report::summary", Function, Public, (5, 7)),
            ("report::floor_area", Function, Public, (9, 11)),
        ],
    );
    assert_symbols(
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
    assert_symbols(
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
    assert_symbols(
        &symbols,
        "tests/ledger.rs",
        &[("tests::ledger::checks", Function, Private, (1, 1))],
    );
}

#[test]
fn methods_of_an_impl_for_a_reference_or_a_primitive_are_methods_on_that_type() {
    let dir = tree(&[(
        "src/lib.rs",
        "pub struct Reader<R>(R);\n\
         pub struct Number(u64);\n\
         impl<'a, R> Iterator for &'a mut Reader<R> {\n    type Item = u8;\n    fn next(&mut self) -> Option<u8> { None }\n}\n\
         impl std::fmt::Display for &Number {\n    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, \"{}\", self.0) }\n}\n\
         impl Float for f32 {\n    fn pow10(self) -> f32 { self }\n}\n\
         impl Float for &f64 {\n    fn pow10(self) -> f64 { *self }\n}\n",
    )]);
    let symbols: Symbols = produce("rust", dir.path());
    assert_symbols(
        &symbols,
        "src/lib.rs",
        &[
            ("Reader::next", Method, Public, (5, 5)),
            ("Number::fmt", Method, Public, (8, 8)),
            ("f32::pow10", Method, Public, (11, 11)),
            ("f64::pow10", Method, Public, (14, 14)),
        ],
    );
}
