// ABOUTME: Root of cupel-plugin: discovers plugins and runs them over the plugin protocol (design §8.6), as
// ABOUTME: checks in the `CheckRegistry` and producers in the `ProducerRegistry` beside the native ones.
//! Checks and producers outside the binary.
//!
//! [`discover`] finds the plugins `[plugins]` names and, unless `--no-path-plugins`, the
//! `cupel-check-*` and `cupel-produce-*` executables on `PATH`, asks each to `describe` itself,
//! and validates what it says (ADR-088). The [`Plugins`] it returns register into the same
//! registries as native code: a check plugin is a [`Check`](cupel_core::Check) that writes its
//! declared facts to files and runs `<plugin> run`, a producer plugin a
//! [`Producer`](cupel_core::Producer) of the kind it provides (ADR-089). The protocol's types are
//! [`cupel_core::plugin`]'s (ADR-087). [`evidence`] validates a finding's evidence against the
//! schema its manifest declares, for the host and for the matrix test (ADR-043).

mod check;
mod command;
mod discover;
pub mod evidence;
mod manifest;
mod output;
mod producer;
mod request;

pub use check::PluginCheck;
pub use discover::{Discovery, Plugins, discover};
pub use producer::providable_kinds;
