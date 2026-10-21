// ABOUTME: The Go ModuleGraph producer: imports resolved through go.mod module paths and go.work
// ABOUTME: members to package files, the standard library and required modules as external, the rest unresolved.
mod support;

use cupel_core::syntax::{EdgeKind, ModuleGraph};
use support::{Edge, EdgeRow, edge_rows, fixture, kinds, produce, row, tree, under};

/// Whether an edge starts or ends in the `deps_` fixtures, which `static.deps` reads and
/// `the_deps_fixtures_are_a_package_cycle_and_a_layered_import` pins separately.
fn is_deps(row: &EdgeRow) -> bool {
    row.1.contains("deps_") || row.4.contains("deps_")
}

/// The fixture's rows outside the `deps_` fixtures.
fn without_deps(rows: Vec<EdgeRow>) -> Vec<EdgeRow> {
    rows.into_iter().filter(|row| !is_deps(row)).collect()
}

const FIXTURE_EDGES: &[Edge<'static>] = &[
    ("cmd/ledger/main.go", 4, "fmt", "external:std", false),
    ("cmd/ledger/main.go", 5, "os", "external:std", false),
];

/// The command imports the root package, which is an edge to each of its files.
const PACKAGE_FILES: [&str; 9] = [
    "clones_settlement.go",
    "complexity.go",
    "crap_scores.go",
    "decide.go",
    "ledger.go",
    "pricing.go",
    "rates.go",
    "references.go",
    "twins.go",
];

const PACKAGE_EDGES: &[Edge<'static>] = &[
    ("complexity.go", 3, "fmt", "external:std", false),
    ("decide.go", 4, "errors", "external:std", false),
    ("decide.go", 5, "strconv", "external:std", false),
    ("decide.go", 6, "strings", "external:std", false),
    ("ledger.go", 4, "fmt", "external:std", false),
    (
        "ledger.go",
        6,
        "github.com/google/uuid",
        "external:github.com/google/uuid",
        false,
    ),
    (
        "ledger.go",
        8,
        "example.com/ledger/gen",
        "unresolved",
        false,
    ),
    (
        "ledger.go",
        9,
        "example.com/ledger/report",
        "report/format.go",
        false,
    ),
    (
        "ledger.go",
        10,
        "example.com/shapes",
        "shapes/geometry.go",
        false,
    ),
    (
        "ledger_property_test.go",
        4,
        "testing",
        "external:std",
        false,
    ),
    (
        "ledger_property_test.go",
        5,
        "testing/quick",
        "external:std",
        false,
    ),
    (
        "ledger_quality_test.go",
        4,
        "testing",
        "external:std",
        false,
    ),
    (
        "ledger_quality_test.go",
        6,
        "github.com/stretchr/testify/assert",
        "external:github.com/stretchr/testify",
        false,
    ),
    ("ledger_test.go", 4, "os", "external:std", false),
    ("ledger_test.go", 5, "testing", "external:std", false),
    ("pricing.go", 3, "log", "external:std", false),
    ("pricing_test.go", 4, "testing", "external:std", false),
    (
        "pricing_test.go",
        6,
        "github.com/gkampitakis/go-snaps/snaps",
        "external:github.com/gkampitakis/go-snaps",
        false,
    ),
    (
        "pricing_test.go",
        7,
        "github.com/stretchr/testify/assert",
        "external:github.com/stretchr/testify",
        false,
    ),
    (
        "pricing_test.go",
        8,
        "github.com/stretchr/testify/mock",
        "external:github.com/stretchr/testify",
        false,
    ),
    (
        "pricing_test.go",
        9,
        "github.com/stretchr/testify/require",
        "external:github.com/stretchr/testify",
        false,
    ),
    ("report/format.go", 3, "fmt", "external:std", false),
];

const SHAPES_EDGES: &[Edge<'static>] = &[("shapes/geometry.go", 3, "math", "external:std", false)];

#[test]
fn the_fixture_graph_is_exactly_its_import_edges() {
    let graph: ModuleGraph = produce("go", &fixture("go"));
    let roots: Vec<&str> = graph.roots.iter().map(|r| r.root.as_str()).collect();
    assert_eq!(roots, [".", "shapes"]);
    let mut expected = under(".", FIXTURE_EDGES);
    expected.extend(PACKAGE_FILES.map(|file| {
        row(
            ".",
            ("cmd/ledger/main.go", 7, "example.com/ledger", file, false),
        )
    }));
    expected.extend(under(".", PACKAGE_EDGES));
    expected.extend(under("shapes", SHAPES_EDGES));
    assert_eq!(without_deps(edge_rows(&graph)), expected);
}

const WORKSPACE_EDGES: &[Edge<'static>] = &[
    ("app/main.go", 4, "bank/util", "app/util/util.go", false),
    ("app/main.go", 5, "embed", "external:std", false),
    (
        "app/main.go",
        6,
        "example.org/lib/money",
        "lib/money/cents.go",
        false,
    ),
    (
        "app/main.go",
        6,
        "example.org/lib/money",
        "lib/money/money.go",
        false,
    ),
    (
        "app/main.go",
        7,
        "github.com/acme/kit/v2/log",
        "external:github.com/acme/kit/v2",
        false,
    ),
    (
        "app/main.go",
        8,
        "github.com/acme/other",
        "unresolved",
        false,
    ),
    ("app/main.go", 9, "bank/empty", "unresolved", false),
];

#[test]
fn work_members_dotless_module_paths_aliases_and_unrequired_modules() {
    let dir = tree(&[
        (
            "go.work",
            "go 1.22\n\nuse ./app\nuse (\n\t./lib // shared code\n)\n",
        ),
        (
            "app/go.mod",
            "module \"bank\"\n\ngo 1.22\n\nrequire github.com/acme/kit/v2 v2.1.0 // indirect\n",
        ),
        (
            "app/main.go",
            "package main\n\nimport (\n\tu \"bank/util\"\n\t_ `embed`\n\t. \"example.org/lib/money\"\n\t\"github.com/acme/kit/v2/log\"\n\t\"github.com/acme/other\"\n\t\"bank/empty\"\n)\n",
        ),
        ("app/util/util.go", "package util\n"),
        ("app/util/util_test.go", "package util\n"),
        ("app/empty/README.md", "nothing here\n"),
        ("lib/go.mod", "module example.org/lib\n"),
        ("lib/money/money.go", "package money\n"),
        ("lib/money/cents.go", "package money\n"),
    ]);
    let graph: ModuleGraph = produce("go", dir.path());
    let roots: Vec<&str> = graph.roots.iter().map(|r| r.root.as_str()).collect();
    assert_eq!(roots, ["app", "lib"]);
    assert_eq!(edge_rows(&graph), under("app", WORKSPACE_EDGES));
}

#[test]
fn files_outside_any_module_are_grouped_under_the_repository_root() {
    let dir = tree(&[(
        "loose.go",
        "package main\n\nimport (\n\t\"net/http\"\n\t\"github.com/acme/kit\"\n)\n",
    )]);
    let graph: ModuleGraph = produce("go", dir.path());
    assert_eq!(
        edge_rows(&graph),
        under(
            ".",
            &[
                ("loose.go", 4, "net/http", "external:std", false),
                ("loose.go", 5, "github.com/acme/kit", "unresolved", false),
            ]
        )
    );
}

#[test]
fn the_deps_fixtures_are_a_package_cycle_and_a_layered_import() {
    let graph: ModuleGraph = produce("go", &fixture("go"));
    let deps: Vec<EdgeRow> = edge_rows(&graph).into_iter().filter(is_deps).collect();
    let module = "example.com/ledger";
    assert_eq!(
        deps,
        [
            (
                "deps_cycle/alpha/alpha.go",
                "deps_cycle/beta",
                "deps_cycle/beta/beta.go"
            ),
            (
                "deps_cycle/beta/beta.go",
                "deps_cycle/alpha",
                "deps_cycle/alpha/alpha.go"
            ),
            (
                "deps_layers/domain/order.go",
                "deps_layers/infra",
                "deps_layers/infra/store.go"
            ),
            (
                "deps_layers/port/port.go",
                "deps_layers/domain",
                "deps_layers/domain/order.go"
            ),
        ]
        .map(|(from, package, target)| row(
            ".",
            (from, 3, &format!("{module}/{package}"), target, false)
        ))
    );
}

#[test]
fn every_go_import_is_an_import_edge() {
    let graph = produce::<ModuleGraph>("go", &fixture("go"));
    let edges = kinds(&graph);
    let others: Vec<&(String, String, EdgeKind)> = edges
        .iter()
        .filter(|(_, _, kind)| *kind != EdgeKind::Import)
        .collect();
    assert_eq!(
        others,
        Vec::<&(String, String, EdgeKind)>::new(),
        "Go has neither module declarations nor re-exports"
    );
    assert_eq!(edges.len(), 38, "every fixture import is listed");
}
