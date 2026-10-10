// ABOUTME: Shared helpers for cupel-lang integration tests: produce a fact for one language over a
// ABOUTME: fixture repository (or a scratch tree) through the registries, as the scheduler will.
#![allow(dead_code)] // Each test crate uses a different subset of these helpers.

use std::path::{Path, PathBuf};

use cupel_core::{
    Cancellation, Ctx, Event, Fact, ProduceCtx, ProducerRegistry, Scope, Writer, syntax::Span,
};
use cupel_lang::{LanguageRegistry, walk};

/// The fixture repository for `lang` in cupel-fixtures.
pub fn fixture(lang: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cupel-fixtures/repos")
        .join(lang)
}

/// Produces `F` for language `lang` over every source file under `root`, panicking on any
/// event (a parse error in a fixture is a bug in the fixture).
pub fn produce<F: Fact>(lang: &str, root: &Path) -> F {
    let (fact, events) = produce_with_events::<F>(lang, root);
    assert!(events.is_empty(), "{events:?}");
    fact
}

pub fn produce_with_events<F: Fact>(lang: &str, root: &Path) -> (F, Vec<Event>) {
    let languages = LanguageRegistry::builtin();
    let mut producers = ProducerRegistry::default();
    languages.register_producers(&mut producers);
    let language = languages.get(&lang.into()).expect("registered language");
    let producer = producers
        .producers_for(F::KIND, &cupel_lang::lang(language.as_ref()))
        .next()
        .unwrap_or_else(|| panic!("no {} producer for {lang}", F::KIND));
    let writer = Writer::new(root, false).unwrap().confined();
    let ctx = ProduceCtx {
        ctx: Ctx {
            root: writer.root().to_path_buf(),
        },
        config: serde_json::Map::new(),
        writer,
        cancel: Cancellation::new(),
    };
    let scope = Scope {
        files: walk(root, &languages),
    };
    let mut events = Vec::new();
    let fact = producer
        .produce(&ctx, &scope, &mut events)
        .unwrap()
        .downcast::<F>()
        .expect("the producer makes its declared fact");
    (fact, events)
}

/// Writes `files` (path, contents) under a fresh temporary directory.
pub fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, contents) in files {
        let path = dir.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }
    dir
}

/// `(start line, end line)` of a span.
pub fn lines(span: Span) -> (u32, u32) {
    (span.line, span.end_line)
}
