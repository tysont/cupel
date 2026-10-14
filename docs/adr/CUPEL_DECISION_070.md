# CUPEL_DECISION_070: Rust assertions that pin a value, assertion helpers, and process assertions

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P2.10

## Context

On the first Cupel dogfood, `tests.quality` reported 94 `test.weak-assertion` and 61 `test.no-assertion` findings on Cupel's Rust tests. Many were the Rust tables misjudging what a test asserts, not weak tests:

- `assert!(a == b)` was a weak truthiness assertion, while Python's `assert a == b` is a strong equality (`[tests.assertions] equal`).
- `assert!(x.is_none())` was weak, while Python's `assert x is None` is a strong equality (`is-none`). `assert!(v.is_empty())` holds for exactly one value of a collection type, which pins it just as `is_none` does.
- Calls of assertion helpers (`assert_fixture(…)`, `assert_spellings(…)`, `assert_round_trip(…)`) were not assertions at all, while Python counts any other `assert_*` method as a strong custom assertion. Tests that assert only through a well-named helper read as asserting nothing.
- `assert_cmd`'s `Command::assert()` chains (`.assert().code(2).stdout(…)`) were not assertions.

## Decision

The Rust `tests.scm` and `tables.toml` gain, before the generic macro patterns (earliest pattern wins):

- `assert!(…)` with a top-level `==` → idiom `equal` (equality, strong); with a top-level `!=` → `not-equal` (equality, weak). A comparison inside a closure or nested parentheses is not top-level and leaves the assertion weak truthiness.
- `assert!(…)` whose top-level tokens call `is_none` or `is_empty`, not preceded by `!` → idiom `one-value` (equality, strong). `is_some`/`is_ok` stay `is-present` (not-null, weak), `is_err` stays weak throws, and a negated predicate stays weak truthiness.
- A call of a function whose name starts with `assert_` (plain or path-qualified) → an assertion named by the function, which is not a key of the table and so takes `[tests].unknown_assertion` (custom, strong), as Python's unlisted `assert_*` methods do.
- A method call `.assert()` (assert_cmd) → idiom `process` (custom, strong).

The Rust fixture gains a test asserting with `assert!(… == …)` and one asserting through a local `assert_balanced` helper; neither may be reported.

## Consequences

- A helper must be named `assert_…` to count; a helper named otherwise (`expect`, `usage`) still leaves its callers reading as assertion-free, which is the naming the convention asks for.
- `assert!(v.is_empty())` is now as strong as `assert_eq!(v, [])`; `assert!(!v.is_empty())` is weak.
- The judgement stays in the language's tables (ADR-027); `tests.quality` is unchanged.
