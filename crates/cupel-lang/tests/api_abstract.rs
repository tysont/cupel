// ABOUTME: Which ApiSurface items are abstract though not interfaces (ADR-178): abstract classes and
// ABOUTME: type aliases in each language that has them; concrete classes, records, and functions are not.
mod support;

use cupel_core::syntax::ApiSurface;
use support::{produce, tree};

/// The `(path, abstract)` of each type-kind item `source` exports, as file `path` of `lang`.
fn abstract_types(lang: &str, path: &str, source: &str) -> Vec<(String, bool)> {
    let dir = tree(&[(path, source)]);
    let api: ApiSurface = produce(lang, dir.path());
    api.items
        .iter()
        .filter(|item| item.kind == cupel_core::syntax::ApiItemKind::Type)
        .map(|item| (item.path.clone(), item.is_abstract))
        .collect()
}

fn rows(rows: &[(&str, bool)]) -> Vec<(String, bool)> {
    rows.iter()
        .map(|(path, is_abstract)| ((*path).to_owned(), *is_abstract))
        .collect()
}

#[test]
fn typescript_abstract_classes_and_type_aliases_are_abstract() {
    let source = "export abstract class Shape { abstract area(): number; }\n\
                  export class Circle {}\n\
                  export type Id = string;\n\
                  export enum Color { Red }\n\
                  export interface Port { send(): void; }\n";
    assert_eq!(
        abstract_types("typescript", "src/shapes.ts", source),
        rows(&[
            ("shapes.Shape", true),
            ("shapes.Circle", false),
            ("shapes.Id", true),
            ("shapes.Color", false),
        ])
    );
}

#[test]
fn java_abstract_classes_are_abstract() {
    let source = "package shapes;\n\
                  public abstract class Shape { public abstract double area(); }\n";
    let concrete = "package shapes;\npublic final class Circle {}\n";
    assert_eq!(
        abstract_types("java", "src/main/java/shapes/Shape.java", source),
        rows(&[("shapes.Shape", true)])
    );
    assert_eq!(
        abstract_types("java", "src/main/java/shapes/Circle.java", concrete),
        rows(&[("shapes.Circle", false)])
    );
}

#[test]
fn python_rust_and_go_type_aliases_are_abstract() {
    assert_eq!(
        abstract_types(
            "python",
            "shapes.py",
            "type Id = str\n\nclass Circle:\n    pass\n"
        ),
        rows(&[("shapes.Id", true), ("shapes.Circle", false)])
    );
    assert_eq!(
        abstract_types(
            "rust",
            "src/lib.rs",
            "pub type Id = String;\npub struct Circle;\n"
        ),
        rows(&[("Id", true), ("Circle", false)])
    );
    assert_eq!(
        abstract_types(
            "go",
            "shapes/shapes.go",
            "package shapes\n\ntype ID = string\n\ntype Circle struct{}\n"
        ),
        rows(&[("shapes.ID", true), ("shapes.Circle", false)])
    );
}
