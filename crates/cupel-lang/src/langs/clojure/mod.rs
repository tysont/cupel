// ABOUTME: Clojure: one grammar for `.clj` and `.cljc`, the queries and tables under this directory, and the
// ABOUTME: producers it registers: a NormTree that keeps called functions, and namespace resolution (ADR-109).
mod normalize;
pub mod project;
mod resolve;

use std::path::Path;
use std::sync::{Arc, LazyLock};

use cupel_core::syntax::{ModuleGraph, NormTree, ProjectInfo};
use cupel_core::{FactKind, LangFamily, LangId, ProducerRegistry, Stability};

use crate::imports::ImportGraph;
use crate::language::Language;
use crate::parse::ParseCache;
use crate::producer::QueryProducer;
use crate::project::ProjectRoots;
use crate::spec::{LanguageSpec, QueryFile};

static SPEC: LazyLock<LanguageSpec> = LazyLock::new(|| {
    LanguageSpec::new(
        "clojure",
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
pub struct Clojure {
    cache: Arc<ParseCache>,
}

impl Clojure {
    pub fn new(cache: Arc<ParseCache>) -> Self {
        Self { cache }
    }
}

impl Language for Clojure {
    fn id(&self) -> LangId {
        LangId::from("clojure")
    }

    fn family(&self) -> LangFamily {
        LangFamily::from("clojure")
    }

    /// Clojure and portable Clojure; ClojureScript (`.cljs`) runs elsewhere and is not claimed.
    fn extensions(&self) -> &[&str] {
        &[".clj", ".cljc"]
    }

    fn grammar(&self, _: &Path) -> tree_sitter::Language {
        tree_sitter_clojure_orchard::LANGUAGE.into()
    }

    fn spec(&self) -> &LanguageSpec {
        &SPEC
    }

    fn stability(&self) -> Stability {
        Stability::Experimental
    }

    /// The generic producers, except that `NormTree` is Clojure's own: what a list calls is its
    /// first element, which the `[normalize]` table cannot name (see `normalize`).
    fn register(&self, reg: &mut ProducerRegistry) {
        let language: Arc<dyn Language> = Arc::new(self.clone());
        let cache = &self.cache;
        crate::generic::register_except(&language, cache, reg, &[FactKind::NormTree]);
        reg.register::<NormTree, _>(QueryProducer::new(
            Arc::clone(&language),
            Arc::clone(cache),
            normalize::extract,
        ));
        // Project roots come from `deps.edn` files on disk, which queries cannot see.
        reg.register::<ProjectInfo, _>(ProjectRoots::new(
            Arc::clone(&language),
            &project::ECOSYSTEM,
            &super::ecosystems::KNOWN,
        ));
        // Which file defines a namespace depends on the classpath's roots, which queries
        // cannot see.
        reg.register::<ModuleGraph, _>(ImportGraph::new(
            language,
            Arc::clone(cache),
            resolve::graph,
        ));
    }
}
