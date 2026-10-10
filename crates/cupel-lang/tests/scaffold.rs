// ABOUTME: Tests for the generic producer scaffold: files outside the language are ignored, broken
// ABOUTME: files become `lang.parse-error` findings instead of panics, and cancellation stops work.
use std::fs;
use std::path::Path;
use std::sync::Arc;

use cupel_core::syntax::{Span, SymbolDef, SymbolKind, Symbols, Visibility};
use cupel_core::{Cancellation, Ctx, Error, Event, ProduceCtx, Producer, Scope, Severity, Writer};
use cupel_lang::{LanguageRegistry, QueryProducer, Source};

/// One symbol per file, named after the file, so the test sees which files were extracted.
fn one_symbol_per_file(source: &Source<'_>) -> Symbols {
    Symbols {
        symbols: vec![SymbolDef {
            name: source.parsed.file.clone(),
            namespace: String::new(),
            display: source.parsed.file.clone(),
            kind: SymbolKind::Function,
            visibility: Visibility::Public,
            lang: source.language.id(),
            file: source.parsed.file.clone(),
            span: Span {
                line: 1,
                col: 1,
                end_line: 1,
                end_col: 1,
            },
            body: None,
        }],
    }
}

fn produce_ctx(root: &Path, cancel: Cancellation) -> ProduceCtx {
    let writer = Writer::new(root, false).unwrap().confined();
    ProduceCtx {
        ctx: Ctx {
            root: writer.root().to_path_buf(),
        },
        config: serde_json::Map::new(),
        writer,
        cancel,
    }
}

fn rust_producer(registry: &LanguageRegistry) -> QueryProducer<Symbols> {
    let rust = registry.get(&"rust".into()).unwrap();
    QueryProducer::new(
        Arc::clone(rust),
        Arc::clone(registry.cache()),
        one_symbol_per_file,
    )
}

fn scope(files: &[&str]) -> Scope {
    Scope {
        files: files.iter().map(|f| (*f).to_owned()).collect(),
    }
}

#[test]
fn the_producer_extracts_only_its_languages_files_in_scope_order() {
    let dir = tempfile::tempdir().unwrap();
    for file in ["b.rs", "a.rs", "c.py"] {
        fs::write(dir.path().join(file), "fn f() {}\n").unwrap();
    }
    let registry = LanguageRegistry::builtin();
    let producer = rust_producer(&registry);
    let mut events = Vec::new();

    let fact = producer
        .produce(
            &produce_ctx(dir.path(), Cancellation::new()),
            &scope(&["a.rs", "b.rs", "c.py"]),
            &mut events,
        )
        .unwrap();

    let names: Vec<&str> = fact.symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["a.rs", "b.rs"]);
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn a_file_that_does_not_parse_is_a_parse_error_finding_and_still_extracted() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("ok.rs"), "fn f() {}\n").unwrap();
    fs::write(
        dir.path().join("broken.rs"),
        "fn ok() {}\n\nfn f() {\n    let x = 1 +;\n}\n",
    )
    .unwrap();
    let registry = LanguageRegistry::builtin();
    let producer = rust_producer(&registry);
    let mut events = Vec::new();

    let fact = producer
        .produce(
            &produce_ctx(dir.path(), Cancellation::new()),
            &scope(&["broken.rs", "ok.rs"]),
            &mut events,
        )
        .unwrap();

    assert_eq!(fact.symbols.len(), 2);
    let [Event::Finding(finding)] = events.as_slice() else {
        panic!("one finding expected: {events:?}");
    };
    assert_eq!(finding.kind, "lang.parse-error");
    assert_eq!(finding.check.as_str(), "lang");
    assert_eq!(finding.severity, Severity::Warn);
    let location = finding.location.as_ref().unwrap();
    assert_eq!(location.file, "broken.rs");
    assert_eq!((location.line, location.col), (4, Some(16)));
    assert!(finding.title.contains("broken.rs"), "{}", finding.title);
    assert_eq!(finding.evidence.0["lang"], "rust");
}

#[test]
fn unreadable_and_non_utf8_files_are_findings_not_panics() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bytes.rs"), [0xff, 0xfe, 0x00, 0x80]).unwrap();
    let registry = LanguageRegistry::builtin();
    let producer = rust_producer(&registry);
    let mut events = Vec::new();

    let fact = producer
        .produce(
            &produce_ctx(dir.path(), Cancellation::new()),
            &scope(&["bytes.rs", "missing.rs"]),
            &mut events,
        )
        .unwrap();

    assert!(fact.symbols.is_empty());
    let files: Vec<String> = events
        .iter()
        .map(|event| match event {
            Event::Finding(finding) => {
                assert_eq!(finding.kind, "lang.parse-error");
                finding.location.as_ref().unwrap().file.clone()
            }
            other => panic!("unexpected event {other:?}"),
        })
        .collect();
    assert_eq!(files, ["bytes.rs", "missing.rs"]);
}

#[test]
fn a_cancelled_run_stops_the_producer() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.rs"), "fn f() {}\n").unwrap();
    let registry = LanguageRegistry::builtin();
    let producer = rust_producer(&registry);
    let cancel = Cancellation::new();
    cancel.cancel();

    let result = producer.produce(
        &produce_ctx(dir.path(), cancel),
        &scope(&["a.rs"]),
        &mut Vec::new(),
    );

    assert_eq!(result, Err(Error::Cancelled));
}
