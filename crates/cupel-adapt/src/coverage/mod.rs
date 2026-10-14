// ABOUTME: Coverage report parsers, one per format (LCOV, Cobertura XML, Istanbul JSON, Go coverprofile, JaCoCo
// ABOUTME: XML), format recognition, and placing a report's paths in the repository (design §8.7, ADR-079).
//! Coverage reports.
//!
//! A format parser knows a format, not a tool, and touches neither processes nor files: each
//! `parse_*` takes a report's text and returns a [`CoverageMap`](cupel_core::coverage::CoverageMap) whose paths are exactly as the
//! report spells them. [`parse`] recognizes the format from the text. [`placed`] then makes
//! the paths relative to the repository root, given where the tool ran. Each parser is tested
//! against real reports committed under `cupel-fixtures/captures/coverage/`.
//!
//! ```text
//! let map = cupel_adapt::coverage::parse_lcov(&text)?;          // paths as written
//! let map = cupel_adapt::coverage::placed(map, &[ran_in], "web"); // relative to the repository
//! ```

mod c8;
mod cargo_llvm_cov;
mod cobertura;
mod coverage_py;
mod coverprofile;
mod error;
mod format;
mod fresh;
mod go_coverprofile;
mod istanbul;
mod jacoco;
mod jest_coverage;
mod lcov;
mod pytest_cov;
mod tool;
mod vitest_coverage;

pub use cobertura::parse_cobertura;
pub use coverprofile::parse_coverprofile;
pub use error::ReportError;
pub use format::{Format, parse, placed};
pub use istanbul::parse_istanbul;
pub use jacoco::parse_jacoco;
pub use lcov::parse_lcov;
