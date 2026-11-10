// ABOUTME: JavaScript and TypeScript constructs the compilers accept and the upstream grammars rejected (ADR-161):
// ABOUTME: a bare `&` in JSX text and attribute strings, and `unique` as an identifier; none is a parse error.
use std::fs;

use cupel_core::{Cancellation, Ctx, Event, ProduceCtx, Scope, Writer};
use cupel_lang::{LanguageRegistry, parse_scope};

/// The files of `files` (name, text) that the language `lang` reports as `lang.parse-error`, parsed from a fresh
/// directory.
fn parse_errors(lang: &str, files: &[(&str, &str)]) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in files {
        fs::write(dir.path().join(name), text).unwrap();
    }
    let registry = LanguageRegistry::builtin();
    let language = registry.get(&lang.into()).unwrap();
    let writer = Writer::new(dir.path(), false).unwrap().confined();
    let ctx = ProduceCtx::new(
        Ctx::new(writer.root().to_path_buf()),
        writer,
        Cancellation::new(),
    );
    let scope = Scope::of_files(files.iter().map(|(name, _)| (*name).to_owned()).collect());
    let mut events = Vec::new();
    parse_scope(
        registry.cache(),
        &ctx,
        &scope,
        language.as_ref(),
        &mut events,
    )
    .unwrap();
    events
        .iter()
        .filter_map(|event| match event {
            Event::Finding(finding) if finding.kind == "lang.parse-error" => {
                finding.location.as_ref().map(|at| at.file.clone())
            }
            _ => None,
        })
        .collect()
}

/// JSX with a bare `&`: in text, in a double- and a single-quoted attribute, beside character references.
const JSX: [(&str, &str); 5] = [
    ("text", "export const a = <p>SMS & Voice</p>;\n"),
    ("ampersands", "export const b = <p>A &&B & C;</p>;\n"),
    (
        "double",
        "export const c = <link href=\"https://x/css?a=1&family=Inter&display=swap\" />;\n",
    ),
    ("single", "export const d = <a title='x&y'>z</a>;\n"),
    (
        "references",
        "export const e = <p>R&amp;D &#169; &copy;</p>;\n",
    ),
];

/// The files of `JSX`'s cases, each with `extension`, that `lang` reports as `lang.parse-error`.
fn jsx_parse_errors(lang: &str, extension: &str) -> Vec<String> {
    let names: Vec<String> = JSX
        .iter()
        .map(|(name, _)| format!("{name}.{extension}"))
        .collect();
    let files: Vec<(&str, &str)> = names
        .iter()
        .zip(JSX)
        .map(|(name, (_, text))| (name.as_str(), text))
        .collect();
    parse_errors(lang, &files)
}

#[test]
fn a_bare_ampersand_in_tsx_is_not_a_parse_error() {
    assert_eq!(jsx_parse_errors("typescript", "tsx"), Vec::<String>::new());
}

#[test]
fn a_bare_ampersand_in_jsx_is_not_a_parse_error() {
    assert_eq!(jsx_parse_errors("javascript", "jsx"), Vec::<String>::new());
}

#[test]
fn unique_is_an_identifier_unless_it_types_a_unique_symbol() {
    let source =
        "export function f(unique: number[]) {\n  for (let i = 0; i < unique.length; i++) {}\n}\n";
    let symbol =
        "declare const s: unique symbol;\nexport type T = { readonly k: unique symbol };\n";
    let files = [
        ("identifier.ts", source),
        ("identifier.tsx", source),
        ("symbol.ts", symbol),
        ("symbol.tsx", symbol),
    ];
    assert_eq!(parse_errors("typescript", &files), Vec::<String>::new());
}
