// ABOUTME: Go: one grammar for `.go`, the queries and tables under this directory, and the
// ABOUTME: producers it registers.
use std::path::Path;
use std::sync::{Arc, LazyLock};

use cupel_core::{LangFamily, LangId, ProducerRegistry, Stability};

use crate::language::Language;
use crate::parse::ParseCache;
use crate::spec::LanguageSpec;

use crate::spec::QueryFile;

static SPEC: LazyLock<LanguageSpec> = LazyLock::new(|| {
    LanguageSpec::new(
        "go",
        &[(QueryFile::Symbols, include_str!("queries/symbols.scm"))],
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

    fn register(&self, reg: &mut ProducerRegistry) {
        let language: Arc<dyn Language> = Arc::new(self.clone());
        crate::generic::register(&language, &self.cache, reg);
    }
}
