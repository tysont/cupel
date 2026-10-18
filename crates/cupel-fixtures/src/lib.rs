// ABOUTME: The golden-envelope harness: run the real `cupel` binary, redact volatile fields,
// ABOUTME: and compare the result against a stored golden (ADR-003).
//! Test support for Cupel's golden envelopes.
//!
//! A golden test runs the built `cupel` binary with [`run_cupel`], redacts the fields that vary
//! between runs with [`redact`], and compares the result with `golden/<name>.json` through
//! [`assert_golden`]. `UPDATE_GOLDENS=1 make test` rewrites goldens instead of comparing.
//! With `DAEMON_PARITY=1` (`make parity`), every [`run_cupel`] also runs through a daemon on a
//! copy of its directory and fails unless the two outputs match after redaction (ADR-085 §11);
//! with `MCP_PARITY=1`, every [`run_cupel`] of a verb `cupel serve --mcp` exposes also runs as
//! that tool's call on another copy, and must return the same envelope and stderr (ADR-121).
//! The volatile fields, placeholders, and file format are fixed by ADR-003. Fixture
//! repositories live under `repos/`; [`copy_repo`] copies one into a scratch directory to run in.
//!
//! [`Capture`] reads real runner output committed under `captures/<runner>/<case>/`, which
//! the runner producers' parser tests read instead of running the tools.
//!
//! [`conformance`] checks each language's producers against the per-fact construct
//! inventories in `conformance/` (design §8.3.7); its module docs define the inventory format.
//!
//! [`matrix`] is the matrix test's engine: a golden `golden/<check>/<lang>.json` for every
//! supported (check, language), a `skipped` entry for every other pair, and every check run on
//! exactly its declared facts. [`evidence`] validates finding evidence against manifest schemas
//! (ADR-043).
//!
//! Everything golden about one check lives in its own directory (ADR-074), so checks added in
//! parallel never edit the same file:
//!
//! ```text
//! golden/<check-id>/<lang>.json          # the matrix: `check --only <check-id>` on the fixture
//! golden/<check-id>/describe.json        # `describe <check-id>`
//! golden/<check-id>/survey-<lang>.json   # the check's results, findings, and rankings in `survey`
//! repos/<check-id>/<lang>/               # optional: the check's own fixture repository
//! ```
//!
//! A check with its own fixture repository runs on it instead of the shared `repos/<lang>/`
//! ([`matrix::fixture_repo`]), so its fixture code changes no other check's goldens, the fact
//! goldens, or the language tests. Goldens that render every check at once (`survey/<lang>`,
//! `describe/matrix`, `describe/language-<lang>`) keep only what no one check owns; the tests
//! derive the rest from the registries.

mod captures;
pub mod conformance;
mod golden;
pub mod matrix;
mod mcp_parity;
pub mod parity;
mod redact;
mod repos;
mod run;
mod sarif;

pub use captures::Capture;
pub use cupel_plugin::evidence;
pub use golden::{GoldenError, GoldenMode, Goldens, assert_golden, canonical_json};
pub use redact::{redact, redact_sarif};
pub use repos::{copy_repo, repo_path};
pub use run::{Outcome, run_cupel, run_cupel_measuring};
pub use sarif::sarif_problems;
