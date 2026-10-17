// ABOUTME: The bounded model checkers (ADR-117): one `ModelCheckResults` producer per tool, each checking the proof
// ABOUTME: harnesses of a language's project roots over every input within their bounds; Kani for Rust.
//! The model checker producers.
//!
//! A model checker runs in every project root `ProjectInfo` names for its language and in
//! scope, checks each proof harness it finds there, and reports each harness's verdict, the
//! properties it refuted, and the concrete inputs that refute them ([`kani`]).

mod kani;
