// ABOUTME: Clojure: one grammar for `.clj` and `.cljc`, the queries and tables under this directory, and the
// ABOUTME: producers it registers: a NormTree that keeps called functions, and namespace resolution (ADR-109).
mod normalize;
pub mod project;
mod resolve;

use std::path::Path;
use std::sync::{Arc, LazyLock};

use cupel_core::{LangFamily, LangId, ProducerRegistry, Stability};

use crate::language::Language;
use crate::parse::ParseCache;
use crate::project::Ecosystem;
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

    fn ecosystem(&self) -> Option<&'static Ecosystem> {
        Some(&project::ECOSYSTEM)
    }

    /// The generic producers, Clojure's own `NormTree` (what a list calls is its first element,
    /// which no `[normalize]` table can name), and the module graph, which reads `deps.edn`.
    fn register(&self, reg: &mut ProducerRegistry) {
        let language: Arc<dyn Language> = Arc::new(self.clone());
        crate::generic::register(&language, &self.cache, reg);
        normalize::register(&language, &self.cache, reg);
        resolve::register(language, &self.cache, reg);
    }
}
