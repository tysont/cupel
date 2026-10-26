// ABOUTME: The value module: it declares its index child and re-exports the trait, the trait module's only
// ABOUTME: importer; outside the narrowed scope, as serde_json's `src/value/mod.rs` is.
mod index;

pub use self::index::Index;

pub struct Value;
