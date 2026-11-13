// ABOUTME: A JavaScript or TypeScript package's API is what its package.json entries expose (ADR-176): items
// ABOUTME: no `exports` target, `main`, or `types` reaches through re-exports are internal; other languages keep theirs.
mod support;

use cupel_core::syntax::ApiSurface;
use support::{fixture, produce, tree};

/// The paths of the items `lang` reports under the tree of `files`, internal ones marked `!`.
fn exposure(lang: &str, files: &[(&str, &str)]) -> Vec<String> {
    let dir = tree(files);
    let api: ApiSurface = produce(lang, dir.path());
    api.items
        .iter()
        .map(|item| {
            let mark = if item.internal { "!" } else { "" };
            format!("{mark}{}", item.path)
        })
        .collect()
}

/// Molten's `apps/agent`: private, with an `exports` map of subpaths and conditions naming
/// entry files, none under `tools/`.
#[test]
fn an_exports_map_exposes_its_entries_and_what_they_re_export() {
    let manifest = r#"{
      "name": "agent",
      "private": true,
      "exports": {
        ".": "./src/index.ts",
        "./server": { "types": "./src/server.ts", "import": { "node": "./src/server.ts" } }
      }
    }"#;
    let paths = exposure(
        "typescript",
        &[
            ("apps/agent/package.json", manifest),
            (
                "apps/agent/src/index.ts",
                "export { shared as exposed } from './tools/shared';\nexport function entry(): number { return 1; }\n",
            ),
            ("apps/agent/src/server.ts", "export * from './routes';\n"),
            (
                "apps/agent/src/routes.ts",
                "export function route(): void {}\n",
            ),
            (
                "apps/agent/src/tools/shared.ts",
                "export function shared(): void {}\nexport function hidden(): void {}\n",
            ),
            (
                "apps/agent/src/tools/extension-authoring.ts",
                "export function validateCustomManifest(x: string): boolean { return x === ''; }\n",
            ),
        ],
    );
    assert_eq!(
        paths,
        [
            "entry",
            "routes.route",
            "!tools.extension-authoring.validateCustomManifest",
            "tools.shared.shared",
            "!tools.shared.hidden",
        ]
    );
}

/// Without an `exports` map, `main` and `types` are the entries; built output under `dist`
/// stands for the source under `src`.
#[test]
fn main_and_types_are_the_entries_without_an_exports_map() {
    let files = |manifest: &'static str| {
        exposure(
            "typescript",
            &[
                ("package.json", manifest),
                ("src/index.ts", "export * from './a';\n"),
                ("src/a.ts", "export const A = 1;\n"),
                ("src/b.ts", "export const B = 2;\n"),
            ],
        )
    };
    let entries = ["a.A", "!b.B"].map(str::to_owned);
    assert_eq!(files(r#"{ "main": "./src/index.ts" }"#), entries);
    assert_eq!(files(r#"{ "types": "./dist/index.d.ts" }"#), entries);
    assert_eq!(files(r#"{ "typings": "dist/esm/index.d.ts" }"#), entries);
    assert_eq!(files(r#"{ "exports": "./dist/index.js" }"#), entries);
    assert_eq!(files(r#"{ "name": "none" }"#), ["!a.A", "!b.B"]);
}

/// A wildcard subpath exposes every file its target matches.
#[test]
fn a_wildcard_target_exposes_every_file_it_matches() {
    let paths = exposure(
        "typescript",
        &[
            ("package.json", r#"{ "exports": { "./*": "./src/*.ts" } }"#),
            ("src/a.ts", "export const A = 1;\n"),
            ("src/deep/b.ts", "export const B = 2;\n"),
            ("scripts/c.ts", "export const C = 3;\n"),
        ],
    );
    assert_eq!(paths, ["!scripts.c.C", "a.A", "deep.b.B"]);
}

/// The nearest manifest decides: a workspace member without entries exposes nothing, though
/// the root's entries name files of their own, and a file under no manifest is internal.
#[test]
fn the_nearest_manifest_is_the_package_and_no_manifest_exposes_nothing() {
    let paths = exposure(
        "javascript",
        &[
            ("package.json", r#"{ "main": "index.js" }"#),
            ("index.js", "export const ROOT = 1;\n"),
            ("packages/tool/package.json", r#"{ "name": "tool" }"#),
            ("packages/tool/src/run.js", "export function run() {}\n"),
        ],
    );
    assert_eq!(paths, ["ROOT", "!run.run"]);
    let bare = exposure("typescript", &[("lib.ts", "export const X = 1;\n")]);
    assert_eq!(bare, ["!lib.X"]);
}

/// Other languages have no notion of entries here: every public item is API, as before.
#[test]
fn other_languages_keep_every_public_item() {
    for lang in ["rust", "go", "python", "java"] {
        let api: ApiSurface = produce(lang, &fixture(lang));
        assert!(!api.items.is_empty(), "{lang}");
        assert!(api.items.iter().all(|item| !item.internal), "{lang}");
    }
}
