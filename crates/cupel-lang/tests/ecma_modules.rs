// ABOUTME: The JavaScript and TypeScript ModuleGraph producers: Node resolution of ESM, CommonJS, and
// ABOUTME: dynamic imports, package `imports`/`exports`/`main`, tsconfig `paths`, builtins, and externals.
mod support;

use cupel_core::syntax::{EdgeKind, ModuleGraph};
use support::{Edge, edge_rows, fixture, kind, kinds, produce, tree, under};

fn roots(graph: &ModuleGraph) -> Vec<&str> {
    graph.roots.iter().map(|r| r.root.as_str()).collect()
}

const JAVASCRIPT_EDGES: &[Edge<'static>] = &[
    ("bin/cli.js", 2, "../src/math.js", "src/math.js", true),
    (
        "src/deps_cycle/alpha.js",
        1,
        "./beta.js",
        "src/deps_cycle/beta.js",
        true,
    ),
    (
        "src/deps_cycle/beta.js",
        1,
        "./alpha.js",
        "src/deps_cycle/alpha.js",
        true,
    ),
    (
        "src/deps_layers/domain/order.js",
        1,
        "../infra/store.js",
        "src/deps_layers/infra/store.js",
        true,
    ),
    (
        "src/index.js",
        1,
        "./shapes/geometry.js",
        "src/shapes/geometry.js",
        true,
    ),
    (
        "src/ledger.js",
        1,
        "node:fs/promises",
        "external:node:fs",
        false,
    ),
    ("src/ledger.js", 2, "path", "external:node:path", false),
    ("src/ledger.js", 3, "lodash/fp", "external:lodash", false),
    ("src/ledger.js", 4, "./math.js", "src/math.js", true),
    (
        "src/ledger.js",
        5,
        "#money/cents",
        "src/money/cents.js",
        false,
    ),
    (
        "src/ledger.js",
        6,
        "@ledger/shapes",
        "packages/shapes/lib/index.js",
        false,
    ),
    (
        "src/ledger.js",
        7,
        "@ledger/shapes/square",
        "packages/shapes/lib/square.js",
        false,
    ),
    ("src/ledger.js", 8, "./generated.js", "unresolved", true),
    ("src/ledger.js", 12, "./report.cjs", "src/report.cjs", true),
    ("src/report.cjs", 1, "./format.cjs", "src/format.cjs", true),
    (
        "test/decide.test.js",
        1,
        "node:assert/strict",
        "external:node:assert",
        false,
    ),
    (
        "test/decide.test.js",
        2,
        "node:test",
        "external:node:test",
        false,
    ),
    (
        "test/decide.test.js",
        3,
        "../src/decide.js",
        "src/decide.js",
        true,
    ),
    ("test/format.spec.js", 1, "chai", "external:chai", false),
    ("test/format.spec.js", 2, "sinon", "external:sinon", false),
    (
        "test/format.spec.js",
        3,
        "../src/format.cjs",
        "src/format.cjs",
        true,
    ),
    (
        "test/math.quality.test.js",
        1,
        "vitest",
        "external:vitest",
        false,
    ),
    (
        "test/math.quality.test.js",
        2,
        "../src/math.js",
        "src/math.js",
        true,
    ),
    (
        "test/math.quality.test.js",
        3,
        "../src/decide.js",
        "src/decide.js",
        true,
    ),
    ("test/math.test.js", 1, "vitest", "external:vitest", false),
    (
        "test/math.test.js",
        2,
        "../src/math.js",
        "src/math.js",
        true,
    ),
    (
        "test/math.test.js",
        3,
        "../src/decide.js",
        "src/decide.js",
        true,
    ),
    (
        "test/properties.test.js",
        1,
        "fast-check",
        "external:fast-check",
        false,
    ),
    (
        "test/properties.test.js",
        2,
        "vitest",
        "external:vitest",
        false,
    ),
    (
        "test/properties.test.js",
        3,
        "../src/math.js",
        "src/math.js",
        true,
    ),
    (
        "test/report.test.js",
        1,
        "../src/report.cjs",
        "src/report.cjs",
        true,
    ),
];

const JAVASCRIPT_SHAPES_EDGES: &[Edge<'static>] = &[(
    "packages/shapes/lib/index.js",
    1,
    "./square.js",
    "packages/shapes/lib/square.js",
    true,
)];

const TYPESCRIPT_EDGES: &[Edge<'static>] = &[
    ("bin/cli.ts", 2, "../src/math", "src/math.ts", true),
    (
        "src/__tests__/pricing.test.ts",
        1,
        "@jest/globals",
        "external:@jest/globals",
        false,
    ),
    (
        "src/__tests__/pricing.test.ts",
        2,
        "../pricing",
        "src/pricing.ts",
        true,
    ),
    (
        "src/deps_cycle/alpha.ts",
        1,
        "./beta",
        "src/deps_cycle/beta.ts",
        true,
    ),
    (
        "src/deps_cycle/beta.ts",
        1,
        "./alpha",
        "src/deps_cycle/alpha.ts",
        true,
    ),
    (
        "src/deps_layers/domain/order.ts",
        1,
        "../infra/store",
        "src/deps_layers/infra/store.ts",
        true,
    ),
    (
        "src/deps_layers/port.ts",
        1,
        "./domain/order",
        "src/deps_layers/domain/order.ts",
        true,
    ),
    (
        "src/index.ts",
        1,
        "./shapes/geometry",
        "src/shapes/geometry.ts",
        true,
    ),
    ("src/ledger.ts", 1, "node:fs", "external:node:fs", false),
    ("src/ledger.ts", 2, "zod", "external:zod", false),
    ("src/ledger.ts", 3, "fs", "external:node:fs", false),
    ("src/ledger.ts", 4, "./math", "src/math.ts", true),
    (
        "src/ledger.ts",
        5,
        "@app/shapes/geometry",
        "src/shapes/geometry.ts",
        false,
    ),
    ("src/ledger.ts", 6, "./shapes", "src/shapes/index.ts", true),
    ("src/ledger.ts", 7, "./plugin.js", "src/plugin.ts", true),
    (
        "src/ledger.ts",
        8,
        "@ledger/shapes",
        "packages/shapes/src/index.ts",
        false,
    ),
    ("src/ledger.ts", 9, "./generated", "unresolved", true),
    (
        "src/math.quality.test.ts",
        1,
        "vitest",
        "external:vitest",
        false,
    ),
    ("src/math.quality.test.ts", 2, "./math", "src/math.ts", true),
    ("src/math.test.ts", 1, "vitest", "external:vitest", false),
    ("src/math.test.ts", 2, "./math", "src/math.ts", true),
    (
        "src/shapes/index.ts",
        1,
        "./geometry",
        "src/shapes/geometry.ts",
        true,
    ),
    ("src/shapes/index.ts", 2, "../math", "src/math.ts", true),
    ("src/widget.test.tsx", 1, "vitest", "external:vitest", false),
    ("src/widget.test.tsx", 2, "./widget", "src/widget.tsx", true),
    (
        "test/decide.test.ts",
        1,
        "node:assert/strict",
        "external:node:assert",
        false,
    ),
    (
        "test/decide.test.ts",
        2,
        "node:test",
        "external:node:test",
        false,
    ),
    (
        "test/decide.test.ts",
        3,
        "../src/decide",
        "src/decide.ts",
        true,
    ),
    (
        "test/properties.test.ts",
        1,
        "fast-check",
        "external:fast-check",
        false,
    ),
    (
        "test/properties.test.ts",
        2,
        "vitest",
        "external:vitest",
        false,
    ),
    (
        "test/properties.test.ts",
        3,
        "../src/math",
        "src/math.ts",
        true,
    ),
];

#[test]
fn the_javascript_fixture_graph_is_exactly_its_imports_requires_and_dynamic_imports() {
    let graph: ModuleGraph = produce("javascript", &fixture("javascript"));
    assert_eq!(roots(&graph), [".", "packages/shapes"]);
    let mut expected = under(".", JAVASCRIPT_EDGES);
    expected.extend(under("packages/shapes", JAVASCRIPT_SHAPES_EDGES));
    assert_eq!(edge_rows(&graph), expected);
}

#[test]
fn the_typescript_fixture_graph_resolves_paths_aliases_directories_and_js_extensions() {
    let graph: ModuleGraph = produce("typescript", &fixture("typescript"));
    assert_eq!(roots(&graph), [".", "packages/shapes"]);
    assert_eq!(edge_rows(&graph), under(".", TYPESCRIPT_EDGES));
}

const PROBED_EDGES: &[Edge<'static>] = &[
    ("src/main.cjs", 1, "./util", "src/util.cjs", true),
    ("src/main.cjs", 2, "./lib", "src/lib/index.mjs", true),
    ("src/main.cjs", 3, "./both", "src/both.js", true),
    (
        "src/main.cjs",
        4,
        "./vendored",
        "src/vendored/dist/entry.js",
        true,
    ),
    ("src/main.cjs", 5, "./lib/missing", "unresolved", true),
    (
        "src/main.cjs",
        6,
        "https://example.com/x.js",
        "unresolved",
        false,
    ),
];

#[test]
fn extensions_directories_and_package_main_are_probed_in_node_order() {
    let dir = tree(&[
        ("package.json", "{ \"name\": \"app\" }"),
        (
            "src/main.cjs",
            "const util = require(\"./util\");\nconst lib = require(\"./lib\");\nconst both = require(\"./both\");\nconst vendored = require(\"./vendored\");\nconst missing = require(\"./lib/missing\");\nconst url = require(\"https://example.com/x.js\");\n",
        ),
        ("src/util.cjs", "module.exports = {};\n"),
        ("src/lib/index.mjs", "export default 1;\n"),
        ("src/both.js", "export default 1;\n"),
        ("src/both.ts", "export default 1;\n"),
        (
            "src/vendored/package.json",
            "{ \"main\": \"dist/entry.js\" }",
        ),
        ("src/vendored/dist/entry.js", "export default 1;\n"),
    ]);
    let graph: ModuleGraph = produce("javascript", dir.path());
    assert_eq!(edge_rows(&graph), under(".", PROBED_EDGES));
    let typescript = tree(&[
        (
            "src/a.ts",
            "import b from \"./both\";\nimport c from \"./c.mjs\";\n",
        ),
        ("src/both.js", "export default 1;\n"),
        ("src/both.ts", "export default 1;\n"),
        ("src/c.mts", "export default 1;\n"),
    ]);
    let graph: ModuleGraph = produce("typescript", typescript.path());
    assert_eq!(
        edge_rows(&graph),
        under(
            ".",
            &[
                ("src/a.ts", 1, "./both", "src/both.ts", true),
                ("src/a.ts", 2, "./c.mjs", "src/c.mts", true),
            ]
        )
    );
}

#[test]
fn base_url_exports_patterns_and_missing_workspace_subpaths() {
    let dir = tree(&[
        (
            "package.json",
            "{ \"workspaces\": { \"packages\": [\"libs/*\"] } }",
        ),
        (
            "apps/web/tsconfig.json",
            "{ \"compilerOptions\": { /* rooted */ \"baseUrl\": \"src\", \"paths\": { \"~/*\": [\"missing/*\", \"*\"] } } }",
        ),
        (
            "apps/web/src/page.tsx",
            "import { util } from \"lib/util\";\nimport { other } from \"~/lib/other\";\nimport { gone } from \"~/nowhere\";\nimport { ui } from \"@acme/ui/button\";\nimport { none } from \"@acme/ui/missing\";\nimport { core } from \"@acme/core\";\nimport React from \"react\";\n",
        ),
        ("apps/web/src/lib/util.ts", "export const util = 1;\n"),
        ("apps/web/src/lib/other.ts", "export const other = 1;\n"),
        (
            "libs/ui/package.json",
            "{ \"name\": \"@acme/ui\", \"exports\": { \"./*\": { \"types\": \"./types/*.d.ts\", \"default\": \"./src/*.tsx\" } } }",
        ),
        ("libs/ui/src/button.tsx", "export const ui = 1;\n"),
        (
            "libs/core/package.json",
            "{ \"name\": \"@acme/core\", \"exports\": \"./index.ts\" }",
        ),
        ("libs/core/index.ts", "export const core = 1;\n"),
    ]);
    let graph: ModuleGraph = produce("typescript", dir.path());
    assert_eq!(roots(&graph), [".", "libs/core", "libs/ui"]);
    let page = "apps/web/src/page.tsx";
    assert_eq!(
        edge_rows(&graph),
        under(
            ".",
            &[
                (page, 1, "lib/util", "apps/web/src/lib/util.ts", false),
                (page, 2, "~/lib/other", "apps/web/src/lib/other.ts", false),
                (page, 3, "~/nowhere", "unresolved", false),
                (page, 4, "@acme/ui/button", "libs/ui/src/button.tsx", false),
                (page, 5, "@acme/ui/missing", "unresolved", false),
                (page, 6, "@acme/core", "libs/core/index.ts", false),
                (page, 7, "react", "external:react", false),
            ]
        )
    );
}

#[test]
fn workspace_patterns_match_one_segment_with_wildcards_and_classes() {
    let dir = tree(&[
        (
            "package.json",
            "{ \"workspaces\": [\"libs/[a-m]*\", \"tools/?li\"] }",
        ),
        ("libs/core/package.json", "{ \"name\": \"@acme/core\" }"),
        ("libs/ui/package.json", "{ \"name\": \"@acme/ui\" }"),
        ("tools/cli/package.json", "{ \"name\": \"@acme/cli\" }"),
        ("index.js", "import core from \"@acme/core\";\n"),
    ]);
    let graph: ModuleGraph = produce("javascript", dir.path());
    assert_eq!(roots(&graph), [".", "libs/core", "tools/cli"]);
}

#[test]
fn export_from_statements_are_re_exports_and_imports_are_imports() {
    let dir = tree(&[
        ("package.json", "{ \"name\": \"app\" }"),
        (
            "src/index.js",
            "export { a } from \"./a.js\";\nexport * as b from \"./b.js\";\nimport c from \"./c.js\";\nconst d = require(\"./d.js\");\n",
        ),
        ("src/a.js", "export const a = 1;\n"),
        ("src/b.js", "export const b = 1;\n"),
        ("src/c.js", "export default 1;\n"),
        ("src/d.js", "module.exports = {};\n"),
    ]);
    assert_eq!(
        kinds(&produce::<ModuleGraph>("javascript", dir.path())),
        [
            kind("src/index.js", "./a.js", EdgeKind::ReExport),
            kind("src/index.js", "./b.js", EdgeKind::ReExport),
            kind("src/index.js", "./c.js", EdgeKind::Import),
            kind("src/index.js", "./d.js", EdgeKind::Import),
        ]
    );
}
