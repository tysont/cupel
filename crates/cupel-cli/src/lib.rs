// ABOUTME: The `cupel` command line as a library: every module but `main`, so the binary and in-process
// ABOUTME: tests call the same `run` (ADR-076 §8). Only the invocation's entry points are public.
//! The `cupel` command line. The binary's `main` reads the process once
//! ([`Environment::from_process`]), calls [`run`], and writes the [`Output`]; tests may build an
//! [`Environment`] themselves and call [`run`] in-process.

mod against;
mod app;
mod baseline;
mod catalog;
mod check;
mod cli;
mod describe;
mod doctor;
mod error;
mod events;
mod explain;
mod format;
mod init;
mod install;
mod page;
mod persist;
mod problem;
mod project;
mod refusal;
mod registry;
mod render;
mod resources;
mod run;
mod suppress;
mod survey;
mod tests_verb;

pub use app::{Environment, Output, capture_panics, run};
