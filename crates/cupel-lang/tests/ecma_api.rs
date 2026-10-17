// ABOUTME: The JavaScript and TypeScript ApiSurface and Roots producers: exported declarations with
// ABOUTME: normalized (typed) signatures, API differences, and roots: main, exports, tests, default exports.
mod support;

use cupel_core::syntax::{ApiItemKind, ApiSurface, RootReason, Roots, Symbols};
use support::{assert_every_case_is_a_test_root, fixture, items, produce, reasons, tree};

use ApiItemKind::{Constant, Function, Interface, Method, Module, Type, Variable};

fn item(path: &str, kind: ApiItemKind, signature: &str) -> (String, ApiItemKind, String) {
    (path.to_owned(), kind, signature.to_owned())
}

#[test]
fn javascript_exports_functions_classes_methods_constants_variables_and_modules() {
    let api: ApiSurface = produce("javascript", &fixture("javascript"));
    assert_eq!(
        items(&api, "src/math.js"),
        [
            item("math.add", Function, "(_, _)"),
            item("math.range", Function, "(_, _)"),
        ]
    );
    assert_eq!(
        items(&api, "src/shapes/geometry.js"),
        [
            item("shapes.geometry.Circle", Type, ""),
            item("shapes.geometry.Circle.constructor", Method, "(_)"),
            item("shapes.geometry.Circle.area", Method, "()"),
            item("shapes.geometry.Circle.unit", Method, "()"),
            item("shapes.geometry.scale", Function, "(_, _)"),
        ]
    );
    assert_eq!(
        items(&api, "src/index.js"),
        [
            item("geometry", Module, ""),
            item("MAX_ENTRIES", Constant, ""),
            item("currency", Variable, ""),
        ]
    );
    assert_eq!(
        items(&api, "src/plugin.js"),
        [item("plugin.default", Function, "(_)")]
    );
    assert_eq!(
        items(&api, "src/format.cjs"),
        [item("format.money", Function, "(_)")]
    );
    assert!(items(&api, "src/report.cjs").is_empty());
}

#[test]
fn typescript_signatures_keep_types_and_drop_parameter_names() {
    let api: ApiSurface = produce("typescript", &fixture("typescript"));
    assert_eq!(
        items(&api, "src/math.ts"),
        [
            item("math.add", Function, "(_: number, _: number): number"),
            item(
                "math.range",
                Function,
                "(_: number, _: number): Generator<number>"
            ),
        ]
    );
    assert_eq!(
        items(&api, "src/shapes/geometry.ts"),
        [
            item("shapes.geometry.Shape", Type, ""),
            item("shapes.geometry.Shape.area", Method, "(): number"),
            item("shapes.geometry.Shape.describe", Method, "(): string"),
            item("shapes.geometry.Circle", Type, ""),
            item(
                "shapes.geometry.Circle.constructor",
                Method,
                "(readonly _: number)"
            ),
            item("shapes.geometry.Circle.area", Method, "(): number"),
            item("shapes.geometry.Circle.unit", Method, "(): Circle"),
            item(
                "shapes.geometry.scale",
                Function,
                "(_: Circle, _: number): Circle"
            ),
        ]
    );
}

#[test]
fn typescript_exports_interfaces_aliases_enums_namespaces_and_export_assignments() {
    let api: ApiSurface = produce("typescript", &fixture("typescript"));
    assert_eq!(
        items(&api, "src/plugin.ts"),
        [
            item("plugin.Registry", Interface, ""),
            item(
                "plugin.Registry.register",
                Method,
                "(_: string, _: { version: number }): void"
            ),
            item("plugin.default", Function, "(_: Registry): void"),
        ]
    );
    assert_eq!(
        items(&api, "src/index.ts"),
        [
            item("geometry", Module, ""),
            item("MAX_ENTRIES", Constant, "number"),
            item("currency", Variable, "string"),
            item("Cents", Type, ""),
            item("Currency", Type, ""),
        ]
    );
    assert_eq!(
        items(&api, "src/legacy.ts"),
        [
            item("legacy.Legacy.round", Function, "(_: number): number"),
            item("legacy.Legacy", Module, ""),
        ]
    );
    assert_eq!(
        items(&api, "src/widget.tsx"),
        [item(
            "widget.Badge",
            Function,
            "(_: { label: string; count: number })"
        )]
    );
}

#[test]
fn private_members_export_clauses_and_unexported_declarations_are_not_api() {
    let dir = tree(&[(
        "src/a.ts",
        "export class A<T> {\n  private hidden(): void {}\n  protected guarded(): void {}\n  #secret(): void {}\n  public shown(x?: T, ...rest: T[]): void {}\n}\nfunction local(): void {}\nexport { local };\nexport { other } from \"./b\";\nexport const pick = <T,>(items: T[], at: number): T => items[at];\n",
    )]);
    let api: ApiSurface = produce("typescript", dir.path());
    assert_eq!(
        items(&api, "src/a.ts"),
        [
            item("a.A", Type, "<T>"),
            item("a.A.shown", Method, "(_?: T, _: T[]): void"),
            item("a.pick", Function, "<T>(_: T[], _: number): T"),
        ]
    );
}

#[test]
fn removing_a_public_parameter_changes_the_api_surface() {
    for (lang, path, before, after) in [
        (
            "javascript",
            "src/pay.js",
            "export function pay(cents, memo) {}\nfunction hidden(a) {}\n",
            "export function pay(cents) {}\nfunction hidden() {}\n",
        ),
        (
            "typescript",
            "src/pay.ts",
            "export function pay(cents: number, memo: string): void {}\n",
            "export function pay(amount: number): void {}\n",
        ),
    ] {
        let before = tree(&[(path, before)]);
        let after = tree(&[(path, after)]);
        let before: ApiSurface = produce(lang, before.path());
        let after: ApiSurface = produce(lang, after.path());
        assert_eq!(before.items.len(), 1, "{lang}");
        assert_eq!(before.items[0].path, after.items[0].path);
        assert_ne!(before.items[0].signature, after.items[0].signature);
    }
}

#[test]
fn main_exports_tests_and_default_exports_are_roots() {
    let root = fixture("javascript");
    let roots: Roots = produce("javascript", &root);
    let symbols: Symbols = produce("javascript", &root);
    assert_eq!(
        reasons(&roots, &symbols, "bin.cli.main"),
        [RootReason::EntryPoint]
    );
    assert_eq!(
        reasons(&roots, &symbols, "plugin.default"),
        [RootReason::Export, RootReason::FrameworkHook]
    );
    assert_eq!(reasons(&roots, &symbols, "math.add"), [RootReason::Export]);
    assert_eq!(
        reasons(&roots, &symbols, "report.summary"),
        [RootReason::Export]
    );
    assert!(reasons(&roots, &symbols, "shapes.geometry.perimeter").is_empty());
    assert_eq!(
        reasons(&roots, &symbols, "test.math.add.adds two numbers"),
        [RootReason::Test]
    );
    let root = fixture("typescript");
    let roots: Roots = produce("typescript", &root);
    let symbols: Symbols = produce("typescript", &root);
    assert_eq!(
        reasons(&roots, &symbols, "plugin.default"),
        [RootReason::Export, RootReason::FrameworkHook]
    );
    assert_eq!(
        reasons(&roots, &symbols, "bin.cli.main"),
        [RootReason::EntryPoint]
    );
}

#[test]
fn every_test_case_in_the_fixtures_is_a_test_root() {
    assert_every_case_is_a_test_root("javascript", 29);
    assert_every_case_is_a_test_root("typescript", 25);
}

#[test]
fn string_literals_in_signatures_are_single_tokens() {
    let dir = tree(&[(
        "src/kind.ts",
        "export function kind(k: \"in\" | 'out', label: string = `x y`): string {\n  return k;\n}\n",
    )]);
    let api: ApiSurface = produce("typescript", dir.path());
    assert_eq!(
        items(&api, "src/kind.ts"),
        [item(
            "kind.kind",
            Function,
            "(_: \"in\" | 'out', _: string = `x y`): string"
        )]
    );
}
