// ABOUTME: The golden-envelope harness: run the real `cupel` binary, redact volatile fields,
// ABOUTME: and compare the result against a stored golden (ADR-003).
//! Test support for Cupel's golden envelopes.
//!
//! A golden test runs the built `cupel` binary with [`run_cupel`], redacts the fields that vary
//! between runs with [`redact`], and compares the result with `golden/<name>.json` through
//! [`assert_golden`]. `UPDATE_GOLDENS=1 make test` rewrites goldens instead of comparing.
//! The volatile fields, placeholders, and file format are fixed by ADR-003. Fixture
//! repositories live under `repos/`; [`copy_repo`] copies one into a scratch directory to run in.
//!
//! [`conformance`] checks each language's producers against the per-fact construct
//! inventories in `conformance/` (design §8.3.7); its module docs define the inventory format.
//!
//! [`matrix`] is the matrix test's engine: a golden `golden/<check>/<lang>.json` for every
//! supported (check, language), a `skipped` entry for every other pair, and every check run on
//! exactly its declared facts. [`evidence`] validates finding evidence against manifest schemas
//! (ADR-043).

pub mod conformance;
pub mod evidence;
mod golden;
pub mod matrix;
mod redact;
mod repos;
mod run;

pub use golden::{GoldenError, GoldenMode, Goldens, assert_golden, canonical_json};
pub use redact::redact;
pub use repos::{copy_repo, repo_path};
pub use run::{Outcome, run_cupel};
