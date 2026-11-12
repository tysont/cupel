// ABOUTME: Report parsers, one per format a runner writes (design §8.7): each turns real tool output into
// ABOUTME: `TestResult`s relative to the directory the runner ran in; runner producers choose one.
//! Runner report formats.
//!
//! A format parser knows a format, not a tool: [`junit`] reads what `pytest --junitxml` and
//! the `JUnit` profile of `cargo nextest` write, and [`jest_json`] reads both `jest --json` and
//! `vitest --reporter=json`. Each is tested against real output committed under
//! `cupel-fixtures/captures/`.

use std::path::Path;

pub mod jest_json;
pub mod junit;
pub mod libtest;
pub mod listing;
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

/// Failure `messages` joined, without their stack frames (`    at f (file:1:2)`): a message's own
/// lines, such as a property's counterexample, are what its output tail must keep.
pub(crate) fn without_frames(messages: &[String]) -> String {
    messages
        .iter()
        .flat_map(|message| message.lines())
        .filter(|line| !line.trim_start().starts_with("at "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `name` relative to `dir`, `/`-separated; `name` itself when it is not below `dir`.
pub(crate) fn relative(name: &str, dir: &Path) -> String {
    Path::new(name).strip_prefix(dir).map_or_else(
        |_| name.to_owned(),
        |path| {
            path.components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        },
    )
}
