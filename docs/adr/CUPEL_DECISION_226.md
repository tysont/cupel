# CUPEL_DECISION_226: A function unused on purpose is neither dead code nor uncovered

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 11 (Q-rs-7) · Amends [079](CUPEL_DECISION_079.md)

## Context
V21 (v0.2.9, `crates/cupel-lang/src/langs/rust/queries/api.scm`) made a Rust function whose name starts with `_` a root, so `static.dead-code` no longer reports tokio-rs/bytes' `fn _assert_trait_object(_b: &dyn Buf) {}` (`src/buf/buf_impl.rs:2968`, and its `BufMut` twin): rustc's leading-underscore convention says the function is unused on purpose, and this one exists only so the build fails when `Buf` stops being object-safe. `tests.coverage` still reports both as `coverage.uncovered-symbol` (out/bytes/survey.json, 2 findings): literally true, since nothing calls them, but no test could or should. NOTES Q-rs-7 recommended exempting them the same way. The owner's ruling (2026-11-14): "_-prefixed functions are exempt from uncovered findings as well as dead code."

V21 marked the functions `@root.hook`, so their `Roots` reason is `framework-hook`, the reason Go's `init` and `TestMain`, Rust's `#[no_mangle]` functions, and other runtime-called hooks share. Those run, and a test can and should cover them. `cupel-checks` must name no language, so the coverage check needs a language-neutral fact that says "never meant to run".

## Options
(a) `tests.coverage` skips every root that is a `framework-hook`: it would stop reporting `init` functions and FFI entry points nobody tests. (b) `tests.coverage` skips symbols named with a leading `_`: a naming rule of one language inside a language-free check, and wrong for Python, where `_name` is merely private. (c) A root reason of its own, `unused`: "unused on purpose", set by the language's `api.scm` with a capture `@root.unused`; `static.dead-code` treats it as every non-export root, and `tests.coverage` reads `Roots` (wanted, not needed) and leaves `unused` roots out. Chosen: (c).

## Decision
1. `RootReason` gains `unused` (serialized `"unused"`): a function its language marks as unused on purpose, which exists for the compiler alone and which nothing calls or runs. `api.scm` captures it as `@root.unused`, beside `@root.entry-point` and `@root.hook`.
2. Rust's `api.scm` captures a function whose name starts with `_` as `@root.unused` instead of `@root.hook`. No other language sets it.
3. `static.dead-code` roots `unused` functions as it roots every reason but `export`; nothing changes there.
4. `tests.coverage` wants `Roots`. A symbol that is an `unused` root is neither scored nor reported: no `coverage.uncovered-symbol`, `coverage.unloaded-file`, or `coverage.uncovered-branch`, and its lines are not in the line or branch ratios, as a test file's symbols are not. Without `Roots` (a language with no `[roots]` table) every symbol is measured, as before.

## Consequences
- bytes' two `_assert_trait_object` functions are no longer `coverage.uncovered-symbol`, and the line ratio no longer counts their (empty) bodies.
- A `_`-prefixed Rust function that does run (called by a macro, for instance) is no longer measured either: by rustc's convention its name says it is not meant to be used.
- The `Roots` fact's reason enumeration gains `unused` (fact schema golden); the matrix of `tests.coverage` gains `roots` as a wanted fact.
- Fixture: `crates/cupel-fixtures/repos/intentionally-unused/rust`, extended with a coverage report.
