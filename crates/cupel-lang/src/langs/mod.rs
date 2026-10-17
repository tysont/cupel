// ABOUTME: The compiled-in languages, one module per language directory (design §8.4.2), and the
// ABOUTME: registry that holds them; `javascript` and `typescript` share the `ecma` module.
pub mod clojure;
pub mod ecma;
pub mod ecosystems;
pub mod go;
pub mod java;
pub mod python;
pub mod rust;

use std::sync::Arc;

use crate::language::{Language, LanguageRegistry};
use crate::parse::ParseCache;

impl LanguageRegistry {
    /// Every compiled-in language, sharing one new parse cache.
    pub fn builtin() -> Self {
        Self::with_cache(Arc::default())
    }

    /// Every compiled-in language, sharing `cache`, which may outlive the registry (the daemon's,
    /// ADR-085).
    pub fn with_cache(cache: Arc<ParseCache>) -> Self {
        let languages: Vec<Arc<dyn Language>> = vec![
            Arc::new(ecma::Ecma::javascript(Arc::clone(&cache))),
            Arc::new(ecma::Ecma::typescript(Arc::clone(&cache))),
            Arc::new(rust::Rust::new(Arc::clone(&cache))),
            Arc::new(go::Go::new(Arc::clone(&cache))),
            Arc::new(python::Python::new(Arc::clone(&cache))),
            Arc::new(java::Java::new(Arc::clone(&cache))),
            Arc::new(clojure::Clojure::new(Arc::clone(&cache))),
        ];
        Self::of(languages, cache)
    }
}
