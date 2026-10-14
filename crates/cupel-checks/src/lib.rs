// ABOUTME: Root of cupel-checks: Cupel's native checks, one module per check, and `register`, the one
// ABOUTME: call that adds them all to a `CheckRegistry` (ADR-001).
//! Cupel's native checks.
//!
//! A check is a pure function from facts and configuration to findings and a score (design
//! §8.3.1): it declares the fact kinds it `needs` and `wants` in its [`Manifest`], plans over a
//! [`Scope`], and runs over the [`Facts`] the scheduler serves it — exactly the declared kinds,
//! no more.
//!
//! # What a check may not do
//!
//! This crate depends on `cupel-core` alone, so it cannot name a language, a grammar, or a
//! tool. `clippy.toml` beside this crate's manifest forbids the rest through
//! `disallowed-methods` and `disallowed-types`, which `make lint` turns into errors:
//!
//! - `std::process` (`Command`, `exit`): processes belong to producers (design §8.5);
//! - `std::fs` (reading, writing, listing, `File`, `OpenOptions`): checks read facts, and the
//!   only place they may write is `CheckCtx::writer`, confined to the run's scratch directory;
//! - `std::env` (variables, arguments, the working directory): configuration arrives in
//!   `CheckCtx::config`, already merged with its per-language override.
//!
//! If a check "needs to know" something about a language, the answer is a semantic field on a
//! fact (plan §2.8), never a match on a language ID.
//!
//! # Adding a check
//!
//! One module here, one line in [`register`], fixture code in each language's fixture
//! repository, and a golden per supported language under `cupel-fixtures/golden/<check>/`;
//! the matrix test names the goldens that are missing.
//!
//! [`Manifest`]: cupel_core::Manifest
//! [`Scope`]: cupel_core::Scope
//! [`Facts`]: cupel_core::Facts

mod finding;
mod metrics;
mod settings;
mod static_clones;
mod static_complexity;
mod static_crap;
mod static_dead_code;
mod static_deps;
mod tests_quality;

use cupel_core::CheckRegistry;

/// Adds every native check to `registry`, in the order `describe` and the envelope list them.
///
/// # Panics
///
/// When a check is already registered under one of these IDs: native check IDs are unique.
pub fn register(registry: &mut CheckRegistry) {
    // Alphabetical by check ID: describe and the envelope list checks in this order.
    registry
        .register(Box::new(static_clones::StaticClones::new()))
        .expect("native check IDs are unique");
    registry
        .register(Box::new(static_complexity::StaticComplexity::new()))
        .expect("native check IDs are unique");
    registry
        .register(Box::new(static_crap::StaticCrap::new()))
        .expect("native check IDs are unique");
    registry
        .register(Box::new(static_dead_code::StaticDeadCode::new()))
        .expect("native check IDs are unique");
    registry
        .register(Box::new(static_deps::StaticDeps::new()))
        .expect("native check IDs are unique");
    registry
        .register(Box::new(tests_quality::TestsQuality::new()))
        .expect("native check IDs are unique");
}
