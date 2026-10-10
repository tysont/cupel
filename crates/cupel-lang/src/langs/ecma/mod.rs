// ABOUTME: JavaScript and TypeScript: one implementation with two language IDs in the `ecma` family,
// ABOUTME: sharing queries and tables; TypeScript appends its own query files and parses `.tsx` as TSX.
use std::path::Path;
use std::sync::{Arc, LazyLock};

use cupel_core::{LangFamily, LangId, ProducerRegistry, Stability};

use crate::language::Language;
use crate::parse::ParseCache;
use crate::spec::LanguageSpec;

const TABLES: &str = include_str!("tables.toml");

static JAVASCRIPT: LazyLock<LanguageSpec> =
    LazyLock::new(|| LanguageSpec::new("javascript", &[], TABLES));

static TYPESCRIPT: LazyLock<LanguageSpec> =
    LazyLock::new(|| LanguageSpec::new("typescript", &[], TABLES));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dialect {
    JavaScript,
    TypeScript,
}

#[derive(Clone)]
pub struct Ecma {
    dialect: Dialect,
    cache: Arc<ParseCache>,
}

impl Ecma {
    pub fn javascript(cache: Arc<ParseCache>) -> Self {
        Self {
            dialect: Dialect::JavaScript,
            cache,
        }
    }

    pub fn typescript(cache: Arc<ParseCache>) -> Self {
        Self {
            dialect: Dialect::TypeScript,
            cache,
        }
    }
}

impl Language for Ecma {
    fn id(&self) -> LangId {
        LangId::from(match self.dialect {
            Dialect::JavaScript => "javascript",
            Dialect::TypeScript => "typescript",
        })
    }

    fn family(&self) -> LangFamily {
        LangFamily::from("ecma")
    }

    fn extensions(&self) -> &[&str] {
        match self.dialect {
            Dialect::JavaScript => &[".js", ".jsx", ".mjs", ".cjs"],
            Dialect::TypeScript => &[".ts", ".mts", ".cts", ".tsx"],
        }
    }

    /// The JavaScript grammar reads JSX itself; TypeScript has separate grammars with and
    /// without JSX, because `<T>x` means a type assertion in one and an element in the other.
    fn grammar(&self, path: &Path) -> tree_sitter::Language {
        match self.dialect {
            Dialect::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Dialect::TypeScript if path.extension().is_some_and(|ext| ext == "tsx") => {
                tree_sitter_typescript::LANGUAGE_TSX.into()
            }
            Dialect::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }
    }

    fn spec(&self) -> &LanguageSpec {
        match self.dialect {
            Dialect::JavaScript => &JAVASCRIPT,
            Dialect::TypeScript => &TYPESCRIPT,
        }
    }

    fn stability(&self) -> Stability {
        Stability::Experimental
    }

    fn register(&self, reg: &mut ProducerRegistry) {
        let language: Arc<dyn Language> = Arc::new(self.clone());
        crate::generic::register(&language, &self.cache, reg);
    }
}
