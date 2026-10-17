// ABOUTME: The deductive verifiers (ADR-118): one `ProofResults` producer per tool, each verifying the proof files in a
// ABOUTME: language's project roots and reporting every obligation it could not discharge; Dafny for every language.
//! The verifier producers.
//!
//! A verifier runs in every project root `ProjectInfo` names for the language and in scope,
//! over the proof files it reads there ([`dafny`]'s `.dfy` files), and reports what it
//! verified and each obligation that is failed or open.

mod dafny;
