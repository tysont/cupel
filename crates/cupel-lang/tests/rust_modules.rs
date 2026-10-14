// ABOUTME: The Rust ModuleGraph producer: `mod` trees, `use` paths (lists, globs, aliases, `self`,
// ABOUTME: `super`), workspace members and path dependencies, external crates, and unresolved imports.
mod support;

use cupel_core::syntax::{ImportTarget, ModuleGraph};
use support::{fixture, produce, tree};

/// (root, from, line, specifier, target, relative) for every edge.
type Row = (String, String, u32, String, String, bool);

fn rows(graph: &ModuleGraph) -> Vec<Row> {
    graph
        .roots
        .iter()
        .flat_map(|root| {
            root.edges.iter().map(|edge| {
                (
                    root.root.clone(),
                    edge.from.clone(),
                    edge.line,
                    edge.specifier.clone(),
                    match &edge.target {
                        ImportTarget::File { path } => path.clone(),
                        ImportTarget::External { package } => format!("external:{package}"),
                        ImportTarget::Unresolved => "unresolved".to_owned(),
                    },
                    edge.relative,
                )
            })
        })
        .collect()
}

fn row(root: &str, from: &str, line: u32, specifier: &str, target: &str, relative: bool) -> Row {
    (
        root.to_owned(),
        from.to_owned(),
        line,
        specifier.to_owned(),
        target.to_owned(),
        relative,
    )
}

/// Whether an edge starts or ends in the `deps_` fixtures, which `static.deps` reads and
/// `the_deps_fixtures_are_a_module_cycle_and_a_layered_tree` pins separately.
fn is_deps(row: &Row) -> bool {
    row.1.contains("deps_") || row.4.contains("deps_")
}

/// The fixture's rows outside the `deps_` fixtures.
fn without_deps(rows: Vec<Row>) -> Vec<Row> {
    rows.into_iter().filter(|row| !is_deps(row)).collect()
}

#[test]
fn the_fixture_graph_is_exactly_its_mod_and_use_edges() {
    let graph: ModuleGraph = produce("rust", &fixture("rust"));
    let roots: Vec<&str> = graph.roots.iter().map(|r| r.root.as_str()).collect();
    assert_eq!(roots, [".", "shapes"]);
    // The integration test `QUALITY` has a test of its own.
    let others: Vec<Row> = rows(&graph)
        .into_iter()
        .filter(|row| row.1 != QUALITY)
        .collect();
    assert_eq!(
        without_deps(others),
        [
            row(".", "src/account.rs", 1, "std::fmt", "external:std", false),
            row(
                ".",
                "src/account.rs",
                3,
                "serde::Serialize",
                "external:serde",
                false
            ),
            row(
                ".",
                "src/account.rs",
                5,
                "crate::report::format::render",
                "src/report/format.rs",
                false
            ),
            row(".", "src/lib.rs", 3, "account", "src/account.rs", true),
            row(".", "src/lib.rs", 4, "decide", "src/decide.rs", true),
            row(".", "src/lib.rs", 6, "generated", "unresolved", true),
            row(".", "src/lib.rs", 7, "pricing", "src/pricing.rs", true),
            row(".", "src/lib.rs", 8, "report", "src/report/mod.rs", true),
            row(".", "src/lib.rs", 9, "twins", "src/twins.rs", true),
            row(
                ".",
                "src/lib.rs",
                11,
                "account::Account",
                "src/account.rs",
                true
            ),
            row(
                ".",
                "src/main.rs",
                1,
                "ledger::balance",
                "src/lib.rs",
                false
            ),
            row(
                ".",
                "src/pricing.rs",
                33,
                "rstest::fixture",
                "external:rstest",
                false
            ),
            row(
                ".",
                "src/pricing.rs",
                35,
                "super::*",
                "src/pricing.rs",
                true
            ),
            row(
                ".",
                "src/report/mod.rs",
                1,
                "format",
                "src/report/format.rs",
                true
            ),
            row(
                ".",
                "src/report/mod.rs",
                3,
                "shapes::square",
                "shapes/src/lib.rs",
                false
            ),
            row(
                ".",
                "tests/ledger.rs",
                1,
                "ledger::Account",
                "src/lib.rs",
                false
            ),
            row(
                ".",
                "tests/ledger.rs",
                2,
                "ledger::pricing::discounted",
                "src/pricing.rs",
                false
            ),
        ]
    );
}

const QUALITY: &str = "tests/quality_signals.rs";

#[test]
fn an_integration_test_imports_through_the_library_crate() {
    let graph: ModuleGraph = produce("rust", &fixture("rust"));
    let quality = |line: u32, specifier: &str, target: &str| {
        row(".", QUALITY, line, specifier, target, false)
    };
    let edges: Vec<Row> = rows(&graph)
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
    assert_eq!(
        rows(&graph),
        [
            row(".", "src/cli.rs", 1, "nested", "src/cli/nested.rs", true),
            row(
                ".",
                "src/cli/nested.rs",
                4,
                "super::super::parse",
                "src/cli.rs",
                true
            ),
            row(
                ".",
                "src/cli/nested.rs",
                5,
                "self::helper",
                "src/cli/nested.rs",
                true
            ),
            row(".", "src/main.rs", 1, "cli", "src/cli.rs", true),
            row(".", "src/main.rs", 2, "crate::cli", "src/cli.rs", false),
            row(
                ".",
                "src/main.rs",
                2,
                "crate::cli::nested::Other",
                "src/cli/nested.rs",
                false
            ),
            row(
                ".",
                "src/main.rs",
                2,
                "crate::cli::nested::deep",
                "src/cli/nested.rs",
                false
            ),
            row(
                ".",
                "src/main.rs",
                2,
                "crate::cli::parse",
                "src/cli.rs",
                false
            ),
            row(".", "src/main.rs", 3, "alloc", "external:alloc", false),
            row(
                ".",
                "src/main.rs",
                4,
                "my_dep::Thing",
                "external:my-dep",
                false
            ),
            row(".", "src/main.rs", 5, "nowhere::x", "unresolved", false),
        ]
    );
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
        rows(&graph),
        [
            row(
                "crates/app",
                "crates/app/src/lib.rs",
                1,
                "bank_core::money::Cents",
                "crates/core/src/money.rs",
                false
            ),
            row(
                "crates/app",
                "crates/app/src/lib.rs",
                2,
                "core2::money",
                "crates/core/src/money.rs",
                false
            ),
            row(
                "crates/core",
                "crates/core/src/lib.rs",
                1,
                "money",
                "crates/core/src/money.rs",
                true
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
        rows(&graph),
        [
            row(".", "loose.rs", 1, "std::io", "external:std", false),
            row(".", "loose.rs", 2, "sibling", "sibling.rs", true),
        ]
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
        rows(&graph),
        [
            row(".", "src/lib.rs", 1, "Account", "unresolved", true),
            row(".", "src/lib.rs", 2, "ledger", "src/ledger.rs", true),
        ]
    );
}

#[test]
fn the_deps_fixtures_are_a_module_cycle_and_a_layered_tree() {
    let graph: ModuleGraph = produce("rust", &fixture("rust"));
    let deps: Vec<Row> = rows(&graph).into_iter().filter(is_deps).collect();
    let (cycle, layers) = ("src/deps_cycle", "src/deps_layers");
    let r = |from: String, line: u32, specifier: &str, target: String| {
        row(".", &from, line, specifier, &target, true)
    };
    let c = |file: &str| format!("{cycle}/{file}");
    let l = |file: &str| format!("{layers}/{file}");
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
            r("src/lib.rs".to_owned(), 32, "deps_cycle", c("mod.rs")),
            r("src/lib.rs".to_owned(), 33, "deps_layers", l("mod.rs")),
        ]
    );
}
