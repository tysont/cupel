// ABOUTME: The compiled-in languages, one module per language directory (design §8.4.2), and the
// ABOUTME: registry that holds them; `javascript` and `typescript` share the `ecma` module.
pub mod ecma;
pub mod ecosystems;
pub mod go;
pub mod python;
pub mod rust;

use std::sync::Arc;

use crate::language::{Language, LanguageRegistry};
use crate::parse::ParseCache;

impl LanguageRegistry {
    /// The five launch languages, sharing one parse cache.
    pub fn builtin() -> Self {
        let cache = Arc::new(ParseCache::default());
        let languages: Vec<Arc<dyn Language>> = vec![
            Arc::new(ecma::Ecma::javascript(Arc::clone(&cache))),
            Arc::new(ecma::Ecma::typescript(Arc::clone(&cache))),
            Arc::new(rust::Rust::new(Arc::clone(&cache))),
            Arc::new(go::Go::new(Arc::clone(&cache))),
            Arc::new(python::Python::new(Arc::clone(&cache))),
        ];
        Self::of(languages, cache)
    }
}
