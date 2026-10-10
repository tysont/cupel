// ABOUTME: Python: one grammar for `.py`, the queries and tables under this directory, and the
// ABOUTME: producers it registers, including the NormTree override for receivers and docstrings.
mod normalize;

use std::path::Path;
use std::sync::{Arc, LazyLock};

use cupel_core::syntax::{
    Decisions, MutationSites, NormTree, Symbols, TestConventions, TestFrames,
};
use cupel_core::{LangFamily, LangId, ProducerRegistry, Stability};

use crate::conventions::GlobTestConventions;
use crate::language::Language;
use crate::parse::ParseCache;
use crate::producer::QueryProducer;
use crate::spec::LanguageSpec;
use crate::{decisions, mutations, symbols, testframes};

use crate::spec::QueryFile;

static SPEC: LazyLock<LanguageSpec> = LazyLock::new(|| {
    LanguageSpec::new(
        "python",
        &[
            (QueryFile::Symbols, include_str!("queries/symbols.scm")),
            (QueryFile::Decisions, include_str!("queries/decisions.scm")),
            (QueryFile::Mutations, include_str!("queries/mutations.scm")),
            (QueryFile::Tests, include_str!("queries/tests.scm")),
        ],
        include_str!("tables.toml"),
    )
});

#[derive(Clone)]
pub struct Python {
    cache: Arc<ParseCache>,
}

impl Python {
    pub fn new(cache: Arc<ParseCache>) -> Self {
        Self { cache }
    }
}

impl Language for Python {
    fn id(&self) -> LangId {
        LangId::from("python")
    }

    fn family(&self) -> LangFamily {
        LangFamily::from("python")
    }

    fn extensions(&self) -> &[&str] {
        &[".py"]
    }

    fn grammar(&self, _: &Path) -> tree_sitter::Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn spec(&self) -> &LanguageSpec {
        &SPEC
    }

    fn stability(&self) -> Stability {
        Stability::Experimental
    }

    /// The generic producers, registered one by one rather than through `generic::register`
    /// because `NormTree` is Python's own: `self`/`cls` receivers and docstrings are positional
    /// rules the `[normalize]` table cannot express (see `normalize`).
    fn register(&self, reg: &mut ProducerRegistry) {
        let language: Arc<dyn Language> = Arc::new(self.clone());
        let cache = &self.cache;
        reg.register::<Symbols, _>(QueryProducer::new(
            Arc::clone(&language),
            Arc::clone(cache),
            symbols::extract,
        ));
        reg.register::<Decisions, _>(QueryProducer::new(
            Arc::clone(&language),
            Arc::clone(cache),
            decisions::extract,
        ));
        reg.register::<NormTree, _>(QueryProducer::new(
            Arc::clone(&language),
            Arc::clone(cache),
            normalize::extract,
        ));
        reg.register::<MutationSites, _>(QueryProducer::new(
            Arc::clone(&language),
            Arc::clone(cache),
            mutations::extract,
        ));
        reg.register::<TestFrames, _>(QueryProducer::new(
            Arc::clone(&language),
            Arc::clone(cache),
            testframes::extract,
        ));
        reg.register::<TestConventions, _>(GlobTestConventions::new(language));
    }
}
