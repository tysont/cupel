// ABOUTME: The Python ModuleGraph producer: absolute and relative imports through `__init__.py` packages,
// ABOUTME: src and flat layouts, workspace projects, pytest base directories, externals, and misses.
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

#[test]
fn the_fixture_graph_is_exactly_its_import_edges() {
    let graph: ModuleGraph = produce("python", &fixture("python"));
    let roots: Vec<&str> = graph.roots.iter().map(|r| r.root.as_str()).collect();
    assert_eq!(roots, [".", "libs/units"]);
    let (init, account) = ("src/ledger/__init__.py", "src/ledger/account.py");
    let (report, format) = (
        "src/ledger/report/__init__.py",
        "src/ledger/report/format.py",
    );
    let (cli, conftest) = ("src/ledger/cli.py", "tests/conftest.py");
    let (ledger, pricing) = ("tests/test_ledger.py", "tests/test_pricing.py");
    let (decide, units) = ("src/ledger/decide.py", "libs/units/src/units/__init__.py");
    let r = |from: &str, line: u32, specifier: &str, target: &str, relative: bool| {
        row(".", from, line, specifier, target, relative)
    };
    assert_eq!(
        rows(&graph),
        [
            r(init, 5, ".account.Account", account, true),
            r(init, 5, ".account.Audited", account, true),
            r(init, 6, ".generated.schema", "unresolved", true),
            r(account, 1, "typing.Protocol", "external:typing", false),
            r(account, 3, "ledger.report.format.render", format, false),
            r(cli, 1, "sys", "external:sys", false),
            r(cli, 3, "ledger.balance", init, false),
            r(report, 1, "units.cents", units, false),
            r(report, 3, ".format", format, true),
            r(format, 1, "requests", "external:requests", false),
            r(conftest, 1, "pytest", "external:pytest", false),
            r(conftest, 3, "ledger.Account", init, false),
            r(ledger, 1, "unittest.mock.patch", "external:unittest", false),
            r(ledger, 3, "pytest", "external:pytest", false),
            r(ledger, 5, "ledger.Account", init, false),
            r(ledger, 6, "ledger.decide.parse_cents", decide, false),
            r(ledger, 6, "ledger.decide.require_positive", decide, false),
            r(ledger, 7, "ledger.report.format.publish", format, false),
            r(ledger, 7, "ledger.report.format.render", format, false),
            r(pricing, 1, "unittest", "external:unittest", false),
            r(pricing, 2, "unittest.mock", "external:unittest", false),
            r(
                pricing,
                4,
                "ledger.pricing.discounted",
                "src/ledger/pricing.py",
                false
            ),
            r(
                pricing,
                4,
                "ledger.pricing.tally",
                "src/ledger/pricing.py",
                false
            ),
        ]
    );
}

#[test]
fn a_flat_layout_resolves_packages_submodules_aliases_wildcards_and_misses() {
    let dir = tree(&[
        ("pyproject.toml", "[project]\nname = \"shop\"\n"),
        ("shop/__init__.py", "from .cart import Cart\n"),
        ("shop/prices.py", "def total():\n    pass\n"),
        (
            "shop/cart.py",
            "from .prices import total\n\
             from . import prices, missing\n\
             from .. import elsewhere\n\
             import shop.prices as p\n\
             from shop import cart\n\
             from shop.prices import *\n\
             import shop.absent\n\
             import numpy.linalg, os\n\
             from __future__ import annotations\n",
        ),
    ]);
    let graph: ModuleGraph = produce("python", dir.path());
    let r = |line: u32, specifier: &str, target: &str, relative: bool| {
        row(".", "shop/cart.py", line, specifier, target, relative)
    };
    assert_eq!(
        rows(&graph),
        [
            row(
                ".",
                "shop/__init__.py",
                1,
                ".cart.Cart",
                "shop/cart.py",
                true
            ),
            r(1, ".prices.total", "shop/prices.py", true),
            r(2, ".missing", "shop/__init__.py", true),
            r(2, ".prices", "shop/prices.py", true),
            r(3, "..elsewhere", "unresolved", true),
            r(4, "shop.prices", "shop/prices.py", false),
            r(5, "shop.cart", "shop/cart.py", false),
            r(6, "shop.prices.*", "shop/prices.py", false),
            r(7, "shop.absent", "unresolved", false),
            r(8, "numpy.linalg", "external:numpy", false),
            r(8, "os", "external:os", false),
        ]
    );
}

#[test]
fn test_directories_without_init_import_their_siblings_like_pytest_does() {
    let dir = tree(&[
        ("tests/helpers.py", "def build():\n    pass\n"),
        (
            "tests/test_build.py",
            "import helpers\nfrom app import run\n",
        ),
        ("app.py", "def run():\n    pass\n"),
    ]);
    let graph: ModuleGraph = produce("python", dir.path());
    assert_eq!(
        rows(&graph),
        [
            row(
                ".",
                "tests/test_build.py",
                1,
                "helpers",
                "tests/helpers.py",
                false
            ),
            row(".", "tests/test_build.py", 2, "app.run", "app.py", false),
        ]
    );
}

#[test]
fn namespace_packages_and_parent_packages_resolve_through_src() {
    let dir = tree(&[
        ("pyproject.toml", "[project]\nname = \"ns\"\n"),
        (
            "src/ns/sub/mod.py",
            "from .. import sub\nfrom ... import ns\n",
        ),
        (
            "src/ns/tool/__init__.py",
            "from ns.sub import mod\nfrom ns import sub\nfrom .. import tool\n",
        ),
    ]);
    let graph: ModuleGraph = produce("python", dir.path());
    assert_eq!(
        rows(&graph),
        [
            row(".", "src/ns/sub/mod.py", 1, "..sub", "unresolved", true),
            row(".", "src/ns/sub/mod.py", 2, "...ns", "unresolved", true),
            row(
                ".",
                "src/ns/tool/__init__.py",
                1,
                "ns.sub.mod",
                "src/ns/sub/mod.py",
                false
            ),
            row(
                ".",
                "src/ns/tool/__init__.py",
                2,
                "ns.sub",
                "unresolved",
                false
            ),
            row(
                ".",
                "src/ns/tool/__init__.py",
                3,
                "..tool",
                "src/ns/tool/__init__.py",
                true
            ),
        ]
    );
}

#[test]
fn workspace_members_are_projects_even_when_none_of_their_files_is_in_scope() {
    let dir = tree(&[
        (
            "pyproject.toml",
            "[project]\nname = \"app\"\n\n[tool.uv.workspace]\nmembers = [\"packages/*\"]\n",
        ),
        (
            "packages/core/pyproject.toml",
            "[project]\nname = \"core\"\n",
        ),
        ("packages/web/setup.cfg", "[metadata]\nname = web\n"),
        ("main.py", "import core\n"),
    ]);
    let graph: ModuleGraph = produce("python", dir.path());
    let roots: Vec<&str> = graph.roots.iter().map(|r| r.root.as_str()).collect();
    assert_eq!(roots, [".", "packages/core", "packages/web"]);
    assert_eq!(
        rows(&graph),
        [row(".", "main.py", 1, "core", "external:core", false)]
    );
}

#[test]
fn module_names_are_case_sensitive_even_where_the_file_system_is_not() {
    let dir = tree(&[
        ("pkg/__init__.py", ""),
        ("pkg/account.py", ""),
        ("main.py", "from pkg import Account, account\nimport Pkg\n"),
    ]);
    let graph: ModuleGraph = produce("python", dir.path());
    assert_eq!(
        rows(&graph),
        [
            row(".", "main.py", 1, "pkg.Account", "pkg/__init__.py", false),
            row(".", "main.py", 1, "pkg.account", "pkg/account.py", false),
            row(".", "main.py", 2, "Pkg", "external:Pkg", false),
        ]
    );
}
