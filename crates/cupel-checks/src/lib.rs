// ABOUTME: Root of cupel-checks: Cupel's native checks, one module per check, each submitting itself,
// ABOUTME: and `register`, the one call that adds them all to a `CheckRegistry` (ADR-001, ADR-074).
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
//! A check is one module, named after its ID with `.` and `-` as `_` (`static.dead-code` lives
//! in `static_dead_code.rs`), that submits itself to the registry (ADR-074):
//!
//! ```text
//! inventory::submit!(crate::NativeCheck(|| Box::new(StaticDeadCode::new())));
//! ```
//!
//! Beside the module file, a new check adds its `mod <name>;` line here and nothing else in
//! this file, fixture code in each language's fixture repository, and its goldens under
//! `cupel-fixtures/golden/<check-id>/`; the matrix test names the goldens that are missing.
//! Tests and `describe` derive everything else from the registry. A test fails if this file
//! names a registered check anywhere but its `mod` line.
//!
//! [`Manifest`]: cupel_core::Manifest
//! [`Scope`]: cupel_core::Scope
//! [`Facts`]: cupel_core::Facts

mod conc_race;
mod finding;
mod gen_differential;
mod gen_metamorphic;
mod gen_property;
mod manifest;
mod metrics;
mod outcome;
mod perf_bench;
mod perf_footprint;
mod settings;
mod static_analyze;
mod static_api_surface;
mod static_clones;
mod static_complexity;
mod static_crap;
mod static_dead_code;
mod static_deps;
mod tests_assertion_density;
mod tests_coverage;
mod tests_flaky;
mod tests_mutation;
mod tests_quality;
mod tests_weakening;

use cupel_core::{Check, CheckRegistry};

/// A native check, submitted by its own module with `inventory::submit!`; [`register`] builds
/// every submission.
pub struct NativeCheck(pub fn() -> Box<dyn Check>);

inventory::collect!(NativeCheck);

/// Adds every native check to `registry`, in the order `describe` and the envelope list them:
/// by check ID.
///
/// # Panics
///
/// When a check is already registered under one of these IDs: native check IDs are unique.
pub fn register(registry: &mut CheckRegistry) {
    let mut checks: Vec<Box<dyn Check>> = inventory::iter::<NativeCheck>
        .into_iter()
        .map(|native| (native.0)())
        .collect();
    checks.sort_by(|a, b| a.manifest().id.cmp(&b.manifest().id));
    for check in checks {
        registry
            .register(check)
            .expect("native check IDs are unique");
    }
}
