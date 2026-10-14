// ABOUTME: The Go ModuleGraph producer: imports resolved through go.mod module paths and go.work
// ABOUTME: members to package files, the standard library and required modules as external, the rest unresolved.
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

/// An edge in the root `root`; Go imports are never path-relative.
fn row(root: &str, from: &str, line: u32, specifier: &str, target: &str) -> Row {
    (
        root.to_owned(),
        from.to_owned(),
        line,
        specifier.to_owned(),
        target.to_owned(),
        false,
    )
}

#[test]
fn the_fixture_graph_is_exactly_its_import_edges() {
    let graph: ModuleGraph = produce("go", &fixture("go"));
    let roots: Vec<&str> = graph.roots.iter().map(|r| r.root.as_str()).collect();
    assert_eq!(roots, [".", "shapes"]);
    let testify = "external:github.com/stretchr/testify";
    // The command imports the root package, which is an edge to each of its files.
    let package_files = [
        "clones_settlement.go",
        "complexity.go",
        "decide.go",
        "ledger.go",
        "pricing.go",
        "twins.go",
    ]
    .map(|file| row(".", "cmd/ledger/main.go", 7, "example.com/ledger", file));
    let mut expected = vec![
        row(".", "cmd/ledger/main.go", 4, "fmt", "external:std"),
        row(".", "cmd/ledger/main.go", 5, "os", "external:std"),
    ];
    expected.extend(package_files);
    expected.extend([
        row(".", "complexity.go", 3, "fmt", "external:std"),
        row(".", "decide.go", 4, "errors", "external:std"),
        row(".", "decide.go", 5, "strconv", "external:std"),
        row(".", "decide.go", 6, "strings", "external:std"),
        row(".", "ledger.go", 4, "fmt", "external:std"),
        row(
            ".",
            "ledger.go",
            6,
            "github.com/google/uuid",
            "external:github.com/google/uuid",
        ),
        row(".", "ledger.go", 8, "example.com/ledger/gen", "unresolved"),
        row(
            ".",
            "ledger.go",
            9,
            "example.com/ledger/report",
            "report/format.go",
        ),
        row(
            ".",
            "ledger.go",
            10,
            "example.com/shapes",
            "shapes/geometry.go",
        ),
        row(".", "ledger_test.go", 4, "os", "external:std"),
        row(".", "ledger_test.go", 5, "testing", "external:std"),
        row(".", "pricing.go", 3, "log", "external:std"),
        row(".", "pricing_test.go", 4, "testing", "external:std"),
        row(
            ".",
            "pricing_test.go",
            6,
            "github.com/gkampitakis/go-snaps/snaps",
            "external:github.com/gkampitakis/go-snaps",
        ),
        row(
            ".",
            "pricing_test.go",
            7,
            "github.com/stretchr/testify/assert",
            testify,
        ),
        row(
            ".",
            "pricing_test.go",
            8,
            "github.com/stretchr/testify/mock",
            testify,
        ),
        row(
            ".",
            "pricing_test.go",
            9,
            "github.com/stretchr/testify/require",
            testify,
        ),
        row(".", "report/format.go", 3, "fmt", "external:std"),
        row("shapes", "shapes/geometry.go", 3, "math", "external:std"),
    ]);
    assert_eq!(rows(&graph), expected);
}

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
    assert_eq!(
        rows(&graph),
        [
            row("app", "app/main.go", 4, "bank/util", "app/util/util.go"),
            row("app", "app/main.go", 5, "embed", "external:std"),
            row(
                "app",
                "app/main.go",
                6,
                "example.org/lib/money",
                "lib/money/cents.go"
            ),
            row(
                "app",
                "app/main.go",
                6,
                "example.org/lib/money",
                "lib/money/money.go"
            ),
            row(
                "app",
                "app/main.go",
                7,
                "github.com/acme/kit/v2/log",
                "external:github.com/acme/kit/v2"
            ),
            row(
                "app",
                "app/main.go",
                8,
                "github.com/acme/other",
                "unresolved"
            ),
            row("app", "app/main.go", 9, "bank/empty", "unresolved"),
        ]
    );
}

#[test]
fn files_outside_any_module_are_grouped_under_the_repository_root() {
    let dir = tree(&[(
        "loose.go",
        "package main\n\nimport (\n\t\"net/http\"\n\t\"github.com/acme/kit\"\n)\n",
    )]);
    let graph: ModuleGraph = produce("go", dir.path());
    assert_eq!(
        rows(&graph),
        [
            row(".", "loose.go", 4, "net/http", "external:std"),
            row(".", "loose.go", 5, "github.com/acme/kit", "unresolved"),
        ]
    );
}
