// ABOUTME: Go: one grammar for `.go`, the queries and tables under this directory, and the
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
        "go",
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
pub struct Go {
    cache: Arc<ParseCache>,
}

impl Go {
    pub fn new(cache: Arc<ParseCache>) -> Self {
        Self { cache }
    }
}

impl Language for Go {
    fn id(&self) -> LangId {
        LangId::from("go")
    }

    fn family(&self) -> LangFamily {
        LangFamily::from("go")
    }

    fn extensions(&self) -> &[&str] {
        &[".go"]
    }

    fn grammar(&self, _: &Path) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
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
        // Module resolution needs go.mod and go.work, which queries cannot see.
        reg.register::<ModuleGraph, _>(ImportGraph::new(
            language,
            Arc::clone(&self.cache),
            resolve::graph,
        ));
    }
}
