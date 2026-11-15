# CUPEL_DECISION_225: `unreachable!` in a test's local impl is no assertion

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 11 (Q-rs-6)

## Context
On tokio-rs/bytes (Rust), `test_deref_bufmut_forwards` (`tests/test_buf_mut.rs:275`) defines a local `struct Special` and `unsafe impl BufMut for Special` whose `remaining_mut`, `chunk_mut`, and `advance_mut` are `unreachable!(…)`, then calls `put_u8` four ways. The test checks that each call reaches the specialized `put_u8` and none of the defaults: a default would reach an `unreachable!` and panic. `tests.quality` reports it as `test.no-assertion`. NOTES Q-rs-6 recommended it stays so: the check is indirect, and a reader cannot see from the test's body what is checked. The owner's ruling (2026-11-14): "unreachable! in a local impl is not an assertion."

## Options
(a) Count `unreachable!` (or `panic!`) anywhere in a test as an assertion: every test that builds a local double with unimplemented methods would read as asserting. (b) Count it only in a local impl of a test: still an indirect check whose subject is not written in the test. (c) No assertion, as today.

Chosen: (c), which needs no change: Rust's `tests.scm` matches no `unreachable!`, and ADR-221 makes only a `panic!` that an `if` in the test guards an assertion.

## Decision
1. `unreachable!(…)`, `unimplemented!(…)`, and `todo!(…)` are no assertion anywhere in a Rust test, in its body or in a local item it defines.
2. A test whose only failures sit in a local impl's methods is `test.no-assertion`, as bytes' `test_deref_bufmut_forwards` is.

## Consequences
- No behavior changes. A test in `crates/cupel-lang/tests/rust_tests.rs` pins it, so a later pattern for guarded failures (ADR-221) or for macros cannot make such a test assert by accident.
- The remedy the finding suggests stands: assert on what the specialized method recorded, or suppress the finding with the reason.
