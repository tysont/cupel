// ABOUTME: The Rust ApiSurface and Roots producers: public items with normalized signatures, API
// ABOUTME: differences between two versions, and symbols live by convention (main, exports, tests, hooks).
mod support;

use cupel_core::syntax::{ApiItemKind, ApiSurface, RootReason, Roots, Symbols};
use support::{assert_every_case_but_these_is_a_test_root, fixture, items, produce, reasons, tree};

use ApiItemKind::{Constant, Function, Interface, Method, Module, Type, Variable};

fn item(path: &str, kind: ApiItemKind, signature: &str) -> (String, ApiItemKind, String) {
    (path.to_owned(), kind, signature.to_owned())
}

#[test]
fn the_crate_root_exports_modules_constants_statics_and_functions() {
    let api: ApiSurface = produce("rust", &fixture("rust"));
    assert_eq!(
        items(&api, "src/lib.rs"),
        [
            item("account", Module, ""),
            item("decide", Module, ""),
            item("pricing", Module, ""),
            item("report", Module, ""),
            item("twins", Module, ""),
            item("MAX_ENTRIES", Constant, "usize"),
            item("CURRENCY", Variable, "&str"),
            item("balance", Function, "(_: &[i64]) -> i64"),
        ]
    );
}

#[test]
fn types_traits_and_their_methods_have_paths_and_signatures() {
    let api: ApiSurface = produce("rust", &fixture("rust"));
    assert_eq!(
        items(&api, "src/account.rs"),
        [
            item("account::Account", Type, ""),
            item("account::Account::new", Method, "(_: &str) -> Self"),
            item("account::Account::deposit", Method, "(&mut self, _: i64)"),
            item("account::Account::statement", Method, "(&self) -> String"),
            item("account::Audited", Interface, ""),
            item(
                "account::Audited::audit_trail",
                Method,
                "(&self) -> Vec<String>"
            ),
            item("account::Audited::is_clean", Method, "(&self) -> bool"),
        ]
    );
}

#[test]
fn signatures_ignore_layout_and_names_and_keep_generics_and_where_clauses() {
    let compact = "pub fn get<T: Clone>(items: &[T], at: usize) -> Option<&'static str> where T: Copy { None }\n";
    let spread = "pub fn get<\n    T:Clone,\n>(\n    list: &[T],\n    index: usize, // where\n) -> Option<&'static str>\nwhere\n    T: Copy,\n{\n    None\n}\n";
    let signature = "<T: Clone>(_: &[T], _: usize) -> Option<&'static str> where T: Copy";
    assert_eq!(
        public_items(&[compact, spread]),
        [("get", signature), ("get", signature)]
            .map(|(path, signature)| vec![(path.to_owned(), signature.to_owned())])
    );
}

#[test]
fn removing_a_public_parameter_changes_the_api_surface() {
    let before = "pub fn pay(cents: i64, memo: &str) {}\nfn hidden(a: i64) {}\n";
    let after = "pub fn pay(cents: i64) {}\nfn hidden() {}\n";
    assert_eq!(
        public_items(&[before, after]),
        [("pay", "(_: i64, _: &str)"), ("pay", "(_: i64)")]
            .map(|(path, signature)| vec![(path.to_owned(), signature.to_owned())])
    );
}

/// The (path, signature) of each public item of each `src/lib.rs` source, one crate each.
fn public_items(sources: &[&str]) -> Vec<Vec<(String, String)>> {
    sources
        .iter()
        .map(|source| {
            let dir = tree(&[("src/lib.rs", source)]);
            let api: ApiSurface = produce("rust", dir.path());
            api.items
                .iter()
                .map(|item| (item.path.clone(), item.signature.clone()))
                .collect()
        })
        .collect()
}

#[test]
fn restricted_visibility_and_trait_impls_are_not_public_api() {
    let dir = tree(&[(
        "src/lib.rs",
        "pub(crate) fn a() {}\npub struct S;\nimpl std::fmt::Debug for S {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { Ok(()) }\n}\nimpl S {\n    fn private(&self) {}\n    pub(super) fn restricted(&self) {}\n}\nmod inner {\n    pub fn reachable() {}\n}\n",
    )]);
    let api: ApiSurface = produce("rust", dir.path());
    assert_eq!(
        items(&api, "src/lib.rs"),
        [
            item("S", Type, ""),
            item("inner::reachable", Function, "()")
        ]
    );
}

#[test]
fn main_exports_tests_and_linker_hooks_are_roots() {
    let root = fixture("rust");
    let roots: Roots = produce("rust", &root);
    let symbols: Symbols = produce("rust", &root);
    assert_eq!(reasons(&roots, &symbols, "main"), [RootReason::EntryPoint]);
    assert_eq!(
        reasons(&roots, &symbols, "ledger_version"),
        [RootReason::Export, RootReason::FrameworkHook]
    );
    assert_eq!(reasons(&roots, &symbols, "balance"), [RootReason::Export]);
    assert!(reasons(&roots, &symbols, "scale").is_empty());
    assert_eq!(
        reasons(&roots, &symbols, "pricing::tests::tally_reports_overdraft"),
        [RootReason::Test]
    );
}

#[test]
fn every_test_case_in_the_fixture_is_a_test_root() {
    assert_every_case_but_these_is_a_test_root("rust", 26, &["discounts_never_raise_the_price"]);
}

#[test]
fn an_async_runtime_main_is_an_entry_point() {
    let dir = tree(&[("src/main.rs", "#[tokio::main]\nasync fn main() {}\n")]);
    let roots: Roots = produce("rust", dir.path());
    let symbols: Symbols = produce("rust", dir.path());
    assert_eq!(reasons(&roots, &symbols, "main"), [RootReason::EntryPoint]);
}
