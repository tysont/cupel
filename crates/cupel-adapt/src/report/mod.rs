// ABOUTME: Report parsers, one per format a runner writes (design §8.7): each turns real tool output into
// ABOUTME: `TestResult`s relative to the directory the runner ran in; runner producers choose one.
//! Runner report formats.
//!
//! A format parser knows a format, not a tool: [`junit`] reads what `pytest --junitxml` and
//! the `JUnit` profile of `cargo nextest` write, and [`jest_json`] reads both `jest --json` and
//! `vitest --reporter=json`. Each is tested against real output committed under
//! `cupel-fixtures/captures/`.

pub mod jest_json;
pub mod junit;
pub mod libtest;
pub mod test2json;
pub mod unittest;

/// A duration in milliseconds, rounded; `None` when it is not a finite, non-negative number.
pub(crate) fn milliseconds(duration: f64) -> Option<u64> {
    if !duration.is_finite() || duration < 0.0 {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a finite, non-negative duration in milliseconds fits u64"
    )]
    Some(duration.round() as u64)
}
