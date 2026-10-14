// ABOUTME: The Rust ModuleGraph producer: `mod` trees, `use` paths (lists, globs, aliases, `self`,
// ABOUTME: `super`), workspace members and path dependencies, external crates, and unresolved imports.
mod support;

use cupel_core::syntax::{EdgeKind, ModuleGraph};
use support::{Edge, EdgeRow, edge_rows, fixture, kind, kinds, produce, row, tree, under};

/// Whether an edge starts or ends in the `deps_` fixtures, which `static.deps` reads and
/// `the_deps_fixtures_are_a_module_cycle_and_a_layered_tree` pins separately.
fn is_deps(row: &EdgeRow) -> bool {
    row.1.contains("deps_") || row.4.contains("deps_")
}

/// The fixture's rows outside the `deps_` fixtures.
fn without_deps(rows: Vec<EdgeRow>) -> Vec<EdgeRow> {
    rows.into_iter().filter(|row| !is_deps(row)).collect()
}

const FIXTURE_EDGES: &[Edge<'static>] = &[
    ("src/account.rs", 1, "std::fmt", "external:std", false),
    (
        "src/account.rs",
        3,
        "serde::Serialize",
        "external:serde",
        false,
    ),
    (
        "src/account.rs",
        5,
        "crate::report::format::render",
        "src/report/format.rs",
        false,
    ),
    ("src/lib.rs", 3, "account", "src/account.rs", true),
    ("src/lib.rs", 4, "decide", "src/decide.rs", true),
    ("src/lib.rs", 6, "generated", "unresolved", true),
    ("src/lib.rs", 7, "pricing", "src/pricing.rs", true),
    ("src/lib.rs", 8, "report", "src/report/mod.rs", true),
    ("src/lib.rs", 9, "twins", "src/twins.rs", true),
    ("src/lib.rs", 11, "account::Account", "src/account.rs", true),
    ("src/main.rs", 1, "ledger::balance", "src/lib.rs", false),
    (
        "src/pricing.rs",
        33,
        "rstest::fixture",
        "external:rstest",
        false,
    ),
    ("src/pricing.rs", 35, "super::*", "src/pricing.rs", true),
    (
        "src/report/mod.rs",
        1,
        "format",
        "src/report/format.rs",
        true,
    ),
    (
        "src/report/mod.rs",
        3,
        "shapes::square",
        "shapes/src/lib.rs",
        false,
    ),
    ("tests/ledger.rs", 1, "ledger::Account", "src/lib.rs", false),
    (
        "tests/ledger.rs",
        2,
        "ledger::pricing::discounted",
        "src/pricing.rs",
        false,
    ),
];

#[test]
fn the_fixture_graph_is_exactly_its_mod_and_use_edges() {
    let graph: ModuleGraph = produce("rust", &fixture("rust"));
    let roots: Vec<&str> = graph.roots.iter().map(|r| r.root.as_str()).collect();
    assert_eq!(roots, [".", "shapes"]);
    // The integration test `QUALITY` has a test of its own.
    let others: Vec<EdgeRow> = edge_rows(&graph)
        .into_iter()
        .filter(|row| row.1 != QUALITY)
        .collect();
    assert_eq!(without_deps(others), under(".", FIXTURE_EDGES));
}

const QUALITY: &str = "tests/quality_signals.rs";

#[test]
fn an_integration_test_imports_through_the_library_crate() {
    let graph: ModuleGraph = produce("rust", &fixture("rust"));
    let quality = |line: u32, specifier: &str, target: &str| {
        row(".", (QUALITY, line, specifier, target, false))
    };
    let edges: Vec<EdgeRow> = edge_rows(&graph)
        .into_iter()
        .filter(|row| row.1 == QUALITY)
        .collect();
    assert_eq!(
        edges,
        [
            quality(1, "ledger::balance", "src/lib.rs"),
            quality(2, "ledger::decide::classify", "src/decide.rs"),
            quality(3, "ledger::pricing::discounted", "src/pricing.rs"),
            quality(3, "ledger::pricing::tally", "src/pricing.rs"),
            quality(4, "ledger::Account", "src/lib.rs"),
        ]
    );
}

const USE_TREE_EDGES: &[Edge<'static>] = &[
    ("src/cli.rs", 1, "nested", "src/cli/nested.rs", true),
    (
        "src/cli/nested.rs",
        4,
        "super::super::parse",
        "src/cli.rs",
        true,
    ),
    (
        "src/cli/nested.rs",
        5,
        "self::helper",
        "src/cli/nested.rs",
        true,
    ),
    ("src/main.rs", 1, "cli", "src/cli.rs", true),
    ("src/main.rs", 2, "crate::cli", "src/cli.rs", false),
    (
        "src/main.rs",
        2,
        "crate::cli::nested::Other",
        "src/cli/nested.rs",
        false,
    ),
    (
        "src/main.rs",
        2,
        "crate::cli::nested::deep",
        "src/cli/nested.rs",
        false,
    ),
    ("src/main.rs", 2, "crate::cli::parse", "src/cli.rs", false),
    ("src/main.rs", 3, "alloc", "external:alloc", false),
    ("src/main.rs", 4, "my_dep::Thing", "external:my-dep", false),
    ("src/main.rs", 5, "nowhere::x", "unresolved", false),
];

#[test]
fn use_lists_aliases_self_super_chains_and_extern_crates() {
    let dir = tree(&[
        (
            "Cargo.toml",
            "[package]\nname = \"app\"\n\n[dependencies]\nmy-dep = \"1\"\n",
        ),
        (
            "src/main.rs",
            "mod cli;\nuse crate::cli::{self, parse as p, nested::{deep, Other}};\nextern crate alloc;\nuse my_dep::Thing;\nuse nowhere::x;\nfn main() {}\n",
        ),
        ("src/cli.rs", "pub mod nested;\npub fn parse() {}\n"),
        (
            "src/cli/nested.rs",
            "pub fn deep() {}\npub struct Other;\nmod inner {\n    use super::super::parse;\n    use self::helper;\n}\n",
        ),
    ]);
    let graph: ModuleGraph = produce("rust", dir.path());
    assert_eq!(edge_rows(&graph), under(".", USE_TREE_EDGES));
}

#[test]
fn workspace_member_globs_and_renamed_path_dependencies_resolve_into_their_crates() {
    let dir = tree(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
        (
            "crates/core/Cargo.toml",
            "[package]\nname = \"bank-core\"\n",
        ),
        ("crates/core/src/lib.rs", "pub mod money;\n"),
        ("crates/core/src/money.rs", "pub struct Cents;\n"),
        (
            "crates/app/Cargo.toml",
            "[package]\nname = \"app\"\n\n[dependencies]\ncore2 = { path = \"../core\", package = \"bank-core\" }\n",
        ),
        (
            "crates/app/src/lib.rs",
            "use bank_core::money::Cents;\nuse core2::money;\n",
        ),
    ]);
    let graph: ModuleGraph = produce("rust", dir.path());
    let roots: Vec<&str> = graph.roots.iter().map(|r| r.root.as_str()).collect();
    assert_eq!(roots, ["crates/app", "crates/core"]);
    assert_eq!(
        edge_rows(&graph),
        [
            row(
                "crates/app",
                (
                    "crates/app/src/lib.rs",
                    1,
                    "bank_core::money::Cents",
                    "crates/core/src/money.rs",
                    false
                )
            ),
            row(
                "crates/app",
                (
                    "crates/app/src/lib.rs",
                    2,
                    "core2::money",
                    "crates/core/src/money.rs",
                    false
                )
            ),
            row(
                "crates/core",
                (
                    "crates/core/src/lib.rs",
                    1,
                    "money",
                    "crates/core/src/money.rs",
                    true
                )
            ),
        ]
    );
}

#[test]
fn files_outside_any_package_are_grouped_under_the_repository_root() {
    let dir = tree(&[
        ("loose.rs", "use std::io;\nmod sibling;\n"),
        ("sibling.rs", ""),
    ]);
    let graph: ModuleGraph = produce("rust", dir.path());
    assert_eq!(
        edge_rows(&graph),
        under(
            ".",
            &[
                ("loose.rs", 1, "std::io", "external:std", false),
                ("loose.rs", 2, "sibling", "sibling.rs", true),
            ]
        )
    );
}

#[test]
fn module_names_are_case_sensitive_even_where_the_file_system_is_not() {
    let dir = tree(&[
        ("Cargo.toml", "[package]\nname = \"cased\"\n"),
        ("src/lib.rs", "mod Account;\nmod ledger;\n"),
        ("src/account.rs", ""),
        ("src/ledger.rs", ""),
    ]);
    let graph: ModuleGraph = produce("rust", dir.path());
    assert_eq!(
        edge_rows(&graph),
        under(
            ".",
            &[
                ("src/lib.rs", 1, "Account", "unresolved", true),
                ("src/lib.rs", 2, "ledger", "src/ledger.rs", true),
            ]
        )
    );
}

#[test]
fn the_deps_fixtures_are_a_module_cycle_a_layered_tree_and_a_parent_child_tree() {
    let graph: ModuleGraph = produce("rust", &fixture("rust"));
    let deps: Vec<EdgeRow> = edge_rows(&graph).into_iter().filter(is_deps).collect();
    let (cycle, layers) = ("src/deps_cycle", "src/deps_layers");
    let r = |from: String, line: u32, specifier: &str, target: String| {
        row(".", (&from, line, specifier, &target, true))
    };
    let c = |file: &str| format!("{cycle}/{file}");
    let l = |file: &str| format!("{layers}/{file}");
    let t = |file: &str| format!("src/deps_tree/{file}");
    assert_eq!(
        deps,
        [
            r(c("alpha.rs"), 1, "super::beta::BETA", c("beta.rs")),
            r(c("beta.rs"), 1, "super::alpha::ALPHA", c("alpha.rs")),
            r(c("mod.rs"), 1, "alpha", c("alpha.rs")),
            r(c("mod.rs"), 2, "beta", c("beta.rs")),
            r(l("domain/mod.rs"), 1, "money", l("domain/money.rs")),
            r(
                l("domain/mod.rs"),
                3,
                "super::infra::Store",
                l("infra/mod.rs")
            ),
            r(l("mod.rs"), 1, "domain", l("domain/mod.rs")),
            r(l("mod.rs"), 2, "infra", l("infra/mod.rs")),
            r(l("mod.rs"), 3, "port", l("port.rs")),
            r(
                l("port.rs"),
                1,
                "super::domain::money::Cents",
                l("domain/money.rs")
            ),
            r(l("port.rs"), 2, "super::domain::Order", l("domain/mod.rs")),
            r(l("port.rs"), 3, "super::infra::Store", l("infra/mod.rs")),
            r(t("leaf.rs"), 1, "super::LIMIT", t("mod.rs")),
            r(t("mod.rs"), 1, "leaf", t("leaf.rs")),
            r("src/lib.rs".to_owned(), 32, "deps_cycle", c("mod.rs")),
            r("src/lib.rs".to_owned(), 33, "deps_layers", l("mod.rs")),
            r("src/lib.rs".to_owned(), 34, "deps_tree", t("mod.rs")),
        ]
    );
}

#[test]
fn mod_declarations_are_declarations_and_visible_uses_re_exports() {
    let dir = tree(&[
        ("Cargo.toml", "[package]\nname = \"app\"\n"),
        (
            "src/lib.rs",
            "mod child;\npub use child::Thing;\npub(crate) use child::Other;\nuse std::fmt;\n",
        ),
        (
            "src/child.rs",
            "use crate::Error;\npub struct Thing;\npub struct Other;\n",
        ),
    ]);
    assert_eq!(
        kinds(&produce::<ModuleGraph>("rust", dir.path())),
        [
            kind("src/child.rs", "crate::Error", EdgeKind::Import),
            kind("src/lib.rs", "child", EdgeKind::Declaration),
            kind("src/lib.rs", "child::Thing", EdgeKind::ReExport),
            kind("src/lib.rs", "child::Other", EdgeKind::ReExport),
            kind("src/lib.rs", "std::fmt", EdgeKind::Import),
        ]
    );
}
