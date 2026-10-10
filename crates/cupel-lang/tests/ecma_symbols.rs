// ABOUTME: The JavaScript and TypeScript Symbols producers against their fixture repositories: functions,
// ABOUTME: generators, class members, named closures, CommonJS exports, default exports, and test callbacks.
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
use Visibility::{Private, Public};

#[test]
fn exported_functions_and_generators_are_public_functions() {
    for (lang, file) in [("javascript", "src/math.js"), ("typescript", "src/math.ts")] {
        let symbols: Symbols = produce(lang, &fixture(lang));
        expect(
            &symbols,
            file,
            &[
                ("math.add", Function, Public, (1, 3)),
                ("math.range", Function, Public, (5, 9)),
            ],
        );
    }
}

#[test]
fn class_members_take_their_visibility_from_hash_names_and_modifiers() {
    let javascript: Symbols = produce("javascript", &fixture("javascript"));
    expect(
        &javascript,
        "src/shapes/geometry.js",
        &[
            ("shapes.geometry.Circle.constructor", Method, Public, (2, 4)),
            ("shapes.geometry.Circle.area", Method, Public, (6, 8)),
            ("shapes.geometry.Circle.unit", Method, Public, (10, 12)),
            ("shapes.geometry.Circle.#isValid", Method, Private, (14, 16)),
            ("shapes.geometry.scale", Closure, Public, (19, 19)),
            ("shapes.geometry.perimeter", Function, Private, (21, 26)),
            (
                "shapes.geometry.perimeter.twice",
                Function,
                Private,
                (22, 24),
            ),
        ],
    );
    let typescript: Symbols = produce("typescript", &fixture("typescript"));
    expect(
        &typescript,
        "src/shapes/geometry.ts",
        &[
            ("shapes.geometry.Shape.describe", Method, Public, (4, 6)),
            (
                "shapes.geometry.Circle.constructor",
                Method,
                Public,
                (10, 12),
            ),
            ("shapes.geometry.Circle.area", Method, Public, (14, 16)),
            ("shapes.geometry.Circle.unit", Method, Public, (18, 20)),
            ("shapes.geometry.Circle.isValid", Method, Private, (22, 24)),
            ("shapes.geometry.Circle.#cache", Method, Private, (26, 26)),
            ("shapes.geometry.scale", Closure, Public, (29, 30)),
            ("shapes.geometry.perimeter", Function, Private, (32, 37)),
            (
                "shapes.geometry.perimeter.twice",
                Function,
                Private,
                (33, 35),
            ),
        ],
    );
}

#[test]
fn an_anonymous_default_export_is_a_public_function_called_default() {
    for (lang, file, span) in [
        ("javascript", "src/plugin.js", (1, 3)),
        ("typescript", "src/plugin.ts", (5, 7)),
    ] {
        let symbols: Symbols = produce(lang, &fixture(lang));
        expect(
            &symbols,
            file,
            &[("plugin.default", Function, Public, span)],
        );
    }
}

#[test]
fn commonjs_export_assignments_are_public_closures() {
    let symbols: Symbols = produce("javascript", &fixture("javascript"));
    expect(
        &symbols,
        "src/format.cjs",
        &[("format.money", Closure, Public, (1, 3))],
    );
    let dir = tree(&[(
        "lib/rates.cjs",
        "module.exports.daily = (rate) => rate / 365;\nwidget.render = function () {};\n",
    )]);
    let symbols: Symbols = produce("javascript", dir.path());
    expect(
        &symbols,
        "lib/rates.cjs",
        &[("lib.rates.daily", Closure, Public, (1, 1))],
    );
}

#[test]
fn files_outside_src_are_namespaced_by_their_whole_path() {
    let symbols: Symbols = produce("javascript", &fixture("javascript"));
    expect(
        &symbols,
        "bin/cli.js",
        &[("bin.cli.main", Function, Private, (4, 6))],
    );
}

#[test]
fn test_callbacks_are_closures_named_by_their_title_inside_their_suite() {
    let javascript: Symbols = produce("javascript", &fixture("javascript"));
    expect(
        &javascript,
        "test/math.test.js",
        &[("test.math.add.adds two numbers", Closure, Private, (5, 7))],
    );
    let typescript: Symbols = produce("typescript", &fixture("typescript"));
    expect(
        &typescript,
        "src/math.test.ts",
        &[("math.add.adds two numbers", Closure, Private, (5, 7))],
    );
}

#[test]
fn typescript_namespaces_and_tsx_components_are_symbols() {
    let symbols: Symbols = produce("typescript", &fixture("typescript"));
    expect(
        &symbols,
        "src/legacy.ts",
        &[("legacy.Legacy.round", Function, Public, (2, 4))],
    );
    expect(
        &symbols,
        "src/widget.tsx",
        &[("widget.Badge", Function, Public, (1, 3))],
    );
}

#[test]
fn skipped_and_focused_tests_are_symbols_but_suites_and_inline_callbacks_are_not() {
    let dir = tree(&[(
        "src/cart.spec.js",
        "describe.skip(\"cart\", () => {\n  it.only(\"totals\", function () {\n    [1, 2].map((n) => n * 2);\n  });\n  xit(\"empties\", () => {});\n});\n",
    )]);
    let symbols: Symbols = produce("javascript", dir.path());
    expect(
        &symbols,
        "src/cart.spec.js",
        &[
            ("cart.cart.totals", Closure, Private, (2, 4)),
            ("cart.cart.empties", Closure, Private, (5, 5)),
        ],
    );
}

#[test]
fn exported_function_expressions_and_var_bindings_are_closures() {
    let dir = tree(&[(
        "lib/util.mjs",
        "export const twice = function (n) { return n * 2; };\nvar half = (n) => n / 2;\n",
    )]);
    let symbols: Symbols = produce("javascript", dir.path());
    expect(
        &symbols,
        "lib/util.mjs",
        &[
            ("lib.util.twice", Closure, Public, (1, 1)),
            ("lib.util.half", Closure, Private, (2, 2)),
        ],
    );
}

#[test]
fn names_exported_apart_from_their_definition_are_public_at_the_top_level_only() {
    let symbols: Symbols = produce("javascript", &fixture("javascript"));
    expect(
        &symbols,
        "src/report.cjs",
        &[("report.summary", Function, Public, (3, 5))],
    );
    let dir = tree(&[
        (
            "src/esm.js",
            "function add(a, b) { return a + b; }\nconst sub = (a, b) => a - b;\nfunction mul(a, b) {\n  function add(x) { return x; }\n  return a * b;\n}\nfunction hidden() {}\nexport { add, sub as minus };\nexport default mul;\nexport { hidden } from \"./other.js\";\n",
        ),
        (
            "src/cjs.cjs",
            "function a() {}\nfunction b() {}\nfunction c() {}\nfunction d() {}\nmodule.exports = { a, renamed: b };\nexports.alias = c;\n",
        ),
        (
            "src/whole.cjs",
            "function only() {}\nmodule.exports = only;\n",
        ),
        (
            "src/legacy.ts",
            "function round(n: number): number { return n; }\nexport = round;\n",
        ),
    ]);
    let javascript: Symbols = produce("javascript", dir.path());
    expect(
        &javascript,
        "src/esm.js",
        &[
            ("esm.add", Function, Public, (1, 1)),
            ("esm.sub", Closure, Public, (2, 2)),
            ("esm.mul", Function, Public, (3, 6)),
            ("esm.mul.add", Function, Private, (4, 4)),
            ("esm.hidden", Function, Private, (7, 7)),
        ],
    );
    expect(
        &javascript,
        "src/cjs.cjs",
        &[
            ("cjs.a", Function, Public, (1, 1)),
            ("cjs.b", Function, Public, (2, 2)),
            ("cjs.c", Function, Public, (3, 3)),
            ("cjs.d", Function, Private, (4, 4)),
        ],
    );
    expect(
        &javascript,
        "src/whole.cjs",
        &[("whole.only", Function, Public, (1, 1))],
    );
    let typescript: Symbols = produce("typescript", dir.path());
    expect(
        &typescript,
        "src/legacy.ts",
        &[("legacy.round", Function, Public, (1, 1))],
    );
}
