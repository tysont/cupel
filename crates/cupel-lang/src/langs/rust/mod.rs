// ABOUTME: Rust: one grammar for `.rs`, the queries and tables under this directory, and the
// ABOUTME: producers it registers, including hand-written module resolution.
pub mod project;
mod resolve;

use std::path::Path;
use std::sync::{Arc, LazyLock};

use cupel_core::syntax::ModuleGraph;
use cupel_core::{LangFamily, LangId, ProducerRegistry, Stability};

use crate::imports::ImportGraph;
use crate::language::Language;
use crate::parse::ParseCache;
use crate::project::Ecosystem;
use crate::spec::LanguageSpec;

use crate::spec::QueryFile;

static SPEC: LazyLock<LanguageSpec> = LazyLock::new(|| {
    LanguageSpec::new(
        "rust",
        &[
            (QueryFile::Symbols, include_str!("queries/symbols.scm")),
            (QueryFile::Decisions, include_str!("queries/decisions.scm")),
            (QueryFile::Mutations, include_str!("queries/mutations.scm")),
            (QueryFile::Tests, include_str!("queries/tests.scm")),
            (QueryFile::Imports, include_str!("queries/imports.scm")),
            (QueryFile::Api, include_str!("queries/api.scm")),
        ],
        include_str!("tables.toml"),
    )
});

#[derive(Clone)]
pub struct Rust {
    cache: Arc<ParseCache>,
}

impl Rust {
    pub fn new(cache: Arc<ParseCache>) -> Self {
        Self { cache }
    }
}

impl Language for Rust {
    fn id(&self) -> LangId {
        LangId::from("rust")
    }

    fn family(&self) -> LangFamily {
        LangFamily::from("rust")
    }

    fn extensions(&self) -> &[&str] {
        &[".rs"]
    }

    fn grammar(&self, _: &Path) -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn spec(&self) -> &LanguageSpec {
        &SPEC
    }

    fn stability(&self) -> Stability {
        Stability::Experimental
    }

    fn ecosystem(&self) -> Option<&'static Ecosystem> {
        Some(&project::ECOSYSTEM)
    }

    fn register(&self, reg: &mut ProducerRegistry) {
        let language: Arc<dyn Language> = Arc::new(self.clone());
        crate::generic::register(&language, &self.cache, reg);
        // Module resolution needs Cargo manifests and the `mod` tree, which queries cannot see.
        reg.register::<ModuleGraph, _>(ImportGraph::new(
            language,
            Arc::clone(&self.cache),
            resolve::graph,
        ));
    }
}
