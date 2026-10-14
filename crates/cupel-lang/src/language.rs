// ABOUTME: The `Language` trait (design §8.4.1) and the registry of compiled-in languages, found by
// ABOUTME: ID or by file extension; the registry also owns the parse cache its languages share.
use std::path::Path;
use std::sync::Arc;

use cupel_core::{Cancellation, Lang, LangFamily, LangId, ProducerRegistry, Stability};
use rayon::prelude::*;

use crate::langs::{ecma::Ecma, go::Go, python::Python, rust::Rust};
use crate::parse::{ParseCache, ParseProblem, Parsed};
use crate::spec::LanguageSpec;

/// A language: a grammar, a declarative spec, and the producers it registers.
pub trait Language: Send + Sync {
    /// Such as `rust` or `typescript`.
    fn id(&self) -> LangId;
    /// Symbols compare for clones within a family (`javascript` and `typescript` are `ecma`).
    fn family(&self) -> LangFamily;
    /// File extensions with their leading dot, such as `.rs`.
    fn extensions(&self) -> &[&str];
    /// The grammar for `path`; one language may own several (js/jsx, ts, tsx).
    fn grammar(&self, path: &Path) -> tree_sitter::Language;
    /// Queries and tables consumed by the generic producers.
    fn spec(&self) -> &LanguageSpec;
    fn stability(&self) -> Stability;
    /// Register this language's producers. The generic query-driven producers cover most
    /// syntactic facts; a language registers a hand-written producer only where queries
    /// cannot express the fact.
    fn register(&self, reg: &mut ProducerRegistry);
}

/// The language and family pair the support matrix works with.
pub fn lang(language: &dyn Language) -> Lang {
    Lang {
        id: language.id(),
        family: language.family(),
    }
}

/// Whether `path` has one of the language's extensions.
pub fn owns(language: &dyn Language, path: &str) -> bool {
    extension(path).is_some_and(|ext| language.extensions().contains(&ext.as_str()))
}

/// `.rs` for `src/lib.rs`; `None` without an extension.
fn extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{ext}"))
}

/// The compiled-in languages, ordered by ID.
pub struct LanguageRegistry {
    languages: Vec<Arc<dyn Language>>,
    cache: Arc<ParseCache>,
}

impl LanguageRegistry {
    /// The five launch languages, sharing one parse cache.
    pub fn builtin() -> Self {
        let cache = Arc::new(ParseCache::default());
        let mut languages: Vec<Arc<dyn Language>> = vec![
            Arc::new(Ecma::javascript(Arc::clone(&cache))),
            Arc::new(Ecma::typescript(Arc::clone(&cache))),
            Arc::new(Rust::new(Arc::clone(&cache))),
            Arc::new(Go::new(Arc::clone(&cache))),
            Arc::new(Python::new(Arc::clone(&cache))),
        ];
        languages.sort_by_key(|language| language.id());
        Self { languages, cache }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn Language>> {
        self.languages.iter()
    }

    pub fn get(&self, id: &LangId) -> Option<&Arc<dyn Language>> {
        self.languages.iter().find(|language| language.id() == *id)
    }

    /// The language that owns `path` by its extension.
    pub fn for_path(&self, path: &str) -> Option<&Arc<dyn Language>> {
        self.languages
            .iter()
            .find(|language| owns(language.as_ref(), path))
    }

    pub fn ids(&self) -> Vec<LangId> {
        self.languages
            .iter()
            .map(|language| language.id())
            .collect()
    }

    pub fn langs(&self) -> Vec<Lang> {
        self.languages
            .iter()
            .map(|language| lang(language.as_ref()))
            .collect()
    }

    pub fn cache(&self) -> &Arc<ParseCache> {
        &self.cache
    }

    /// Every language registers its producers, in ID order.
    pub fn register_producers(&self, reg: &mut ProducerRegistry) {
        for language in &self.languages {
            language.register(reg);
        }
    }

    /// Parses every file that belongs to a registered language, in parallel, in input order.
    /// Files of no registered language are left out. Once `cancel` fires, files not yet
    /// parsed are reported as problems; callers check the cancellation afterwards.
    pub fn parse_all(
        &self,
        root: &Path,
        files: &[String],
        cancel: &Cancellation,
    ) -> Vec<Result<Parsed, ParseProblem>> {
        files
            .par_iter()
            .filter_map(|file| {
                let language = self.for_path(file)?;
                Some(parse_one(
                    &self.cache,
                    root,
                    file,
                    language.as_ref(),
                    cancel,
                ))
            })
            .collect()
    }
}

/// Parses one file unless the run was cancelled.
pub(crate) fn parse_one(
    cache: &ParseCache,
    root: &Path,
    file: &str,
    language: &dyn Language,
    cancel: &Cancellation,
) -> Result<Parsed, ParseProblem> {
    if cancel.is_cancelled() {
        return Err(ParseProblem {
            file: file.to_owned(),
            lang: language.id(),
            reason: "the run was cancelled".to_owned(),
        });
    }
    cache.parse(root, file, language)
}
