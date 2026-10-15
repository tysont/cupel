// ABOUTME: Root of cupel-mutate, the mutation engine (design §9.3, ADR-080): `engine.mutation` produces
// ABOUTME: `MutationOutcomes` from `MutationSites`, running each mutant with the registered runner in an overlay.
//! The mutation engine.
//!
//! **Dependency rule:** this crate depends on `cupel-core` and `cupel-store` (to read the outcomes
//! of earlier runs) and nothing else from Cupel; it names no language, tool, or check. Only
//! `cupel-cli` and `cupel-fixtures` call [`register`].

mod engine;
mod equivalent;
mod execute;
mod history;
pub mod mutant;
pub mod overlay;
mod plan;
pub mod schemata;
pub mod settings;

pub use engine::{ID, MutationEngine, register};
