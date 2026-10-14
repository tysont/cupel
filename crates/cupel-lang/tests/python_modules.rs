// ABOUTME: The Python ModuleGraph producer: absolute and relative imports through `__init__.py` packages,
// ABOUTME: src and flat layouts, workspace projects, pytest base directories, externals, and misses.
mod support;

use cupel_core::syntax::{EdgeKind, ModuleGraph};
use support::{Edge, EdgeRow, edge_rows, fixture, kind, kinds, produce, row, tree, under};

const FIXTURE_EDGES: &[Edge<'static>] = &[
    (
        "src/deps_cycle/alpha.py",
        1,
        ".beta.BETA",
        "src/deps_cycle/beta.py",
        true,
    ),
    (
        "src/deps_cycle/beta.py",
        1,
        ".alpha",
        "src/deps_cycle/alpha.py",
        true,
    ),
    (
        "src/deps_layers/domain/order.py",
        1,
        "..infra.store.Store",
        "src/deps_layers/infra/store.py",
        true,
    ),
    (
        "src/deps_layers/port.py",
        1,
        "typing.Protocol",
        "external:typing",
        false,
    ),
    (
        "src/deps_layers/port.py",
        3,
        ".domain.order.Order",
        "src/deps_layers/domain/order.py",
        true,
    ),
    (
        "src/ledger/__init__.py",
        5,
        ".account.Account",
        "src/ledger/account.py",
        true,
    ),
    (
        "src/ledger/__init__.py",
        5,
        ".account.Audited",
        "src/ledger/account.py",
        true,
    ),
    (
        "src/ledger/__init__.py",
        6,
        ".generated.schema",
        "unresolved",
        true,
    ),
    (
        "src/ledger/account.py",
        1,
        "typing.Protocol",
        "external:typing",
        false,
    ),
    (
        "src/ledger/account.py",
        3,
        "ledger.report.format.render",
        "src/ledger/report/format.py",
        false,
    ),
    ("src/ledger/cli.py", 1, "sys", "external:sys", false),
    (
        "src/ledger/cli.py",
        3,
        "ledger.balance",
        "src/ledger/__init__.py",
        false,
    ),
    (
        "src/ledger/report/__init__.py",
        1,
        "units.cents",
        "libs/units/src/units/__init__.py",
        false,
    ),
    (
        "src/ledger/report/__init__.py",
        3,
        ".format",
        "src/ledger/report/format.py",
        true,
    ),
    (
        "src/ledger/report/format.py",
        1,
        "requests",
        "external:requests",
        false,
    ),
    ("tests/conftest.py", 1, "pytest", "external:pytest", false),
    (
        "tests/conftest.py",
        3,
        "ledger.Account",
        "src/ledger/__init__.py",
        false,
    ),
    (
        "tests/test_ledger.py",
        1,
        "unittest.mock.patch",
        "external:unittest",
        false,
    ),
    (
        "tests/test_ledger.py",
        3,
        "pytest",
        "external:pytest",
        false,
    ),
    (
        "tests/test_ledger.py",
        5,
        "ledger.Account",
        "src/ledger/__init__.py",
        false,
    ),
    (
        "tests/test_ledger.py",
        6,
        "ledger.decide.parse_cents",
        "src/ledger/decide.py",
        false,
    ),
    (
        "tests/test_ledger.py",
        6,
        "ledger.decide.require_positive",
        "src/ledger/decide.py",
        false,
    ),
    (
        "tests/test_ledger.py",
        7,
        "ledger.report.format.publish",
        "src/ledger/report/format.py",
        false,
    ),
    (
        "tests/test_ledger.py",
        7,
        "ledger.report.format.render",
        "src/ledger/report/format.py",
        false,
    ),
    (
        "tests/test_pricing.py",
        1,
        "unittest",
        "external:unittest",
        false,
    ),
    (
        "tests/test_pricing.py",
        2,
        "unittest.mock",
        "external:unittest",
        false,
    ),
    (
        "tests/test_pricing.py",
        4,
        "ledger.pricing.discounted",
        "src/ledger/pricing.py",
        false,
    ),
    (
        "tests/test_pricing.py",
        4,
        "ledger.pricing.tally",
        "src/ledger/pricing.py",
        false,
    ),
];

#[test]
fn the_fixture_graph_is_exactly_its_import_edges() {
    let graph: ModuleGraph = produce("python", &fixture("python"));
    let roots: Vec<&str> = graph.roots.iter().map(|r| r.root.as_str()).collect();
    assert_eq!(roots, [".", "libs/units"]);
    let others: Vec<EdgeRow> = edge_rows(&graph)
        .into_iter()
        .filter(|row| row.1 != QUALITY)
        .collect();
    assert_eq!(others, under(".", FIXTURE_EDGES));
}

const QUALITY: &str = "tests/test_quality_signals.py";

#[test]
fn the_quality_test_module_imports_through_the_package() {
    let graph: ModuleGraph = produce("python", &fixture("python"));
    let (init, decide) = ("src/ledger/__init__.py", "src/ledger/decide.py");
    let r = |from: &str, line: u32, specifier: &str, target: &str, relative: bool| {
        row(".", (from, line, specifier, target, relative))
    };
    let edges: Vec<EdgeRow> = edge_rows(&graph)
        .into_iter()
        .filter(|row| row.1 == QUALITY)
        .collect();
    assert_eq!(
        edges,
        [
            r(QUALITY, 1, "pytest", "external:pytest", false),
            r(QUALITY, 3, "ledger.Account", init, false),
            r(QUALITY, 3, "ledger.balance", init, false),
            r(QUALITY, 4, "ledger.decide.classify", decide, false),
            r(QUALITY, 4, "ledger.decide.require_positive", decide, false),
            r(
                QUALITY,
                5,
                "ledger.pricing.discounted",
                "src/ledger/pricing.py",
                false
            ),
            r(
                QUALITY,
                5,
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
        row(".", ("shop/cart.py", line, specifier, target, relative))
    };
    assert_eq!(
        edge_rows(&graph),
        [
            row(
                ".",
                ("shop/__init__.py", 1, ".cart.Cart", "shop/cart.py", true)
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
        edge_rows(&graph),
        under(
            ".",
            &[
                (
                    "tests/test_build.py",
                    1,
                    "helpers",
                    "tests/helpers.py",
                    false
                ),
                ("tests/test_build.py", 2, "app.run", "app.py", false),
            ]
        )
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
        edge_rows(&graph),
        under(
            ".",
            &[
                ("src/ns/sub/mod.py", 1, "..sub", "unresolved", true),
                ("src/ns/sub/mod.py", 2, "...ns", "unresolved", true),
                (
                    "src/ns/tool/__init__.py",
                    1,
                    "ns.sub.mod",
                    "src/ns/sub/mod.py",
                    false
                ),
                ("src/ns/tool/__init__.py", 2, "ns.sub", "unresolved", false),
                (
                    "src/ns/tool/__init__.py",
                    3,
                    "..tool",
                    "src/ns/tool/__init__.py",
                    true
                ),
            ]
        )
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
        edge_rows(&graph),
        under(".", &[("main.py", 1, "core", "external:core", false),])
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
        edge_rows(&graph),
        under(
            ".",
            &[
                ("main.py", 1, "pkg.Account", "pkg/__init__.py", false),
                ("main.py", 1, "pkg.account", "pkg/account.py", false),
                ("main.py", 2, "Pkg", "external:Pkg", false),
            ]
        )
    );
}

#[test]
fn imports_of_names_listed_in_all_are_re_exports() {
    let dir = tree(&[
        ("pyproject.toml", "[project]\nname = \"shop\"\n"),
        (
            "shop/__init__.py",
            "__all__ = [\"Cart\"]\n\nfrom .cart import Cart\nfrom .prices import total\n",
        ),
        ("shop/cart.py", "class Cart:\n    pass\n"),
        ("shop/prices.py", "def total():\n    pass\n"),
    ]);
    assert_eq!(
        kinds(&produce::<ModuleGraph>("python", dir.path())),
        [
            kind("shop/__init__.py", ".cart.Cart", EdgeKind::ReExport),
            kind("shop/__init__.py", ".prices.total", EdgeKind::Import),
        ]
    );
}
