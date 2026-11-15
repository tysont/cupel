# CUPEL_DECISION_222: A compile-time assertion in a test is a strong assertion of that test

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 6 (Q-go-3, Q-rs-2) · Amends [032](CUPEL_DECISION_032.md) and [070](CUPEL_DECISION_070.md)

## Context
The v0.2.9 validation found compile-time conformance tests reported as `test.no-assertion`:

- hashicorp/golang-lru (Go), `expirable/expirable_lru_test.go:126`: `func TestLRUInterface(_ *testing.T) { var _ simplelru.LRUCache[int, int] = &LRU[int, int]{} }`. The test fails to compile, and so fails, when `LRU` stops implementing the interface (NOTES Q-go-3).
- clap (Rust), `clap_builder/src/builder/arg_group.rs:590` and two tests in `clap_builder/src/builder/tests.rs`: `fn foo<T: Send + Sync>(_: T) {} foo(ArgGroup::new("test"));`, commented "This test will *fail to compile* if ArgGroup is not Send + Sync" (NOTES Q-rs-2, "same answer as Go's `var _ I = T{}`").

Rust's other compile-time form, `const _: () = assert!(…)` (or `const _: () = { assert!(…); };`), appears in bytes (`src/bytes.rs:1353`, `src/bytes_mut.rs:95`) at module level, outside every test. raft has package-level `var _ ConfigurationStore = (*MockFSMConfigStore)(nil)` (`testing.go:73`) and two more in non-test files. The owner's ruling (2026-11-14): "(var _ I = T{} / const _: () = assert!(...)): counts as asserting."

Open: where such an assertion counts (the test function holding it, the test file, or the symbol it is about), and how strong it is.

## Options
**Where.** (a) In the test function whose body holds it: `TestFrames` attaches every assertion to the innermost case containing it, and `test.no-assertion` is a finding about one case. (b) In every test of the file: a package-level `var _` in a `_test.go` file would make every test of that file assert, though none of them runs anything it checks. (c) On the symbol whose conformance it checks, for no-assertion purposes: `test.no-assertion` is about tests, never about the code under test, so there is nothing to exempt. Chosen: (a). A compile-time assertion outside every test (package or module level, in a source or test file) is no test's assertion, as any assertion outside a case is; it is compiled with the package, not run by a test, and no finding concerns it.

**Strength.** (a) Weak truthiness, as `assert!(…)` without a comparison: the test would still be reported, as `test.weak-assertion`, against the ruling's intent. (b) `custom`/`strong`: the compiler checks exactly the conformance written, a type implementing an interface or a trait bound, or a constant expression. Chosen: (b).

## Decision
1. **Go.** A `var _ T = expr` declaration (blank name, a declared type, a value) in a test function's body is the assertion idiom `static`, `custom`/`strong`, of that test: `expr` must be assignable to `T`, so the test compiles only when it implements the interface.
2. **Rust.** In a test, each of these is the idiom `static`, `custom`/`strong`:
   - `const _: () = assert!(…);` and an `assert!`-family macro that is a statement of a `const _: () = { … };` block: the assertion is the macro invocation, evaluated at compile time;
   - a call of a function the test's own body defines with an empty body and a bounded type parameter (`fn foo<T: Send + Sync>(_: T) {}` then `foo(ArgGroup::new("test"))`): the call compiles only when the argument's type satisfies the bound.
3. Outside every test these declare no assertion of any test, as before.

## Consequences
- golang-lru's `TestLRUInterface` and clap's three `*_send_sync` tests are no longer `test.no-assertion` and, asserting strongly, not `test.weak-assertion` either.
- A local function with a bound but a non-empty body, or one the test does not define, is an ordinary call and asserts as helper calls do (ADR-070, ADR-214).
- bytes' module-level `const _` and raft's package-level `var _` change nothing: no test holds them.
