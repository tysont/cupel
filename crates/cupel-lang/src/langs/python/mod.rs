// ABOUTME: Python: one grammar for `.py`, the queries and tables under this directory, and the
// ABOUTME: producers it registers.
use std::path::Path;
use std::sync::{Arc, LazyLock};

use cupel_core::{LangFamily, LangId, ProducerRegistry, Stability};

use crate::language::Language;
use crate::parse::ParseCache;
use crate::spec::LanguageSpec;

static SPEC: LazyLock<LanguageSpec> =
    LazyLock::new(|| LanguageSpec::new("python", &[], include_str!("tables.toml")));

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

    fn register(&self, reg: &mut ProducerRegistry) {
        let language: Arc<dyn Language> = Arc::new(self.clone());
        crate::generic::register(&language, &self.cache, reg);
    }
}
