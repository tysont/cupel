# CUPEL_DECISION_224: A Rust method named `assert…` asserts, and debug assertions count unless `debug_assertions = false`

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 5 · Amends [070](CUPEL_DECISION_070.md)

## Context
On clap (Rust), five tests in `examples/tutorial_builder/05_01_assert.rs:23` and its siblings assert only through `Command::debug_assert()`, a clap method that panics when the command is misconfigured; `tests.quality` reported each as `test.no-assertion`. ADR-070 counts a free function named `assert_…` (custom, strong) and assert_cmd's `.assert()` method (custom, strong), but no other method. NOTES Q-rs-1 recommended counting method calls whose name starts with `assert` (`.assert_success()`) but not `debug_assert`. The owner's ruling (2026-11-14): "methods named assert… count; debug_assert counts by default, configurable."

Today the `debug_assert!`, `debug_assert_eq!`, and `debug_assert_ne!` macros already count, with the classes of their `assert` counterparts. Both forms check only in a build with debug assertions on; `cargo test` builds with them by default, `cargo test --release` without.

Open: which names a method needs, how a method assertion is classified, and where the setting lives.

## Options
**Names.** (a) `assert` alone (today). (b) Any method name starting with `assert` (`assert`, `assert_eq`, `assert_success`, `asserts_valid`), and any starting with `debug_assert`, clap's case. Chosen: (b).

**Class.** (a) The macro table's class for the same name (`.assert_eq(…)` equality strong, `.debug_assert()` truthiness weak): a project method's name says nothing about what it checks, and `debug_assert()` takes no condition at all. (b) `custom`/`strong`, as ADR-070 classifies `assert_…` helpers and `.assert()`: the method checks what its project defines. Chosen: (b).

**The setting.** (a) A `[lang.rust]` key: `[lang.<id>]` holds tool and producer choices, and no other key there shapes which assertions count. (b) A `tests.quality` setting that configures `TestFrames` (ADR-147's `SettingSpec::configures`): declared in the manifest, so `describe tests.quality` and the README's settings table list it, validated at plan time, overridable per language as `[checks."tests.quality".lang.rust]`, and handed to the `TestFrames` producer as `ProduceCtx::settings`; `tests.assertion-density`, which reads the same fact, sees the same assertions. Chosen: (b).

## Decision
1. **Method assertions.** In a Rust test, a method call whose name starts with `assert` is the idiom `method`, `custom`/`strong`; one whose name starts with `debug_assert` is the idiom `debug-method`, `custom`/`strong`. assert_cmd's `.assert()` stays the `process` idiom (custom, strong), matched first.
2. **Debug assertions.** `[tests.assertions]` classes in a language's `tables.toml` may say `debug = true`: the assertion is evaluated only in a build with debug assertions. In Rust, `debug_assert`, `debug_assert_eq`, `debug_assert_ne`, and `debug-method` are debug assertions.
3. **The setting.** `tests.quality` declares `debug_assertions`, a boolean, default `true`, configuring `TestFrames`. With `true` (or unset), debug assertions count as before. With `false`, the `TestFrames` producer leaves every debug assertion out of the cases holding it, so a test whose only assertions are debug assertions is `test.no-assertion`. A value that is not a boolean is a configuration error at plan time (`ConfigError::CheckSetting`). Per language: `[checks."tests.quality".lang.rust] debug_assertions = false`.
4. A call that asserts through a helper whose class came from a debug assertion stays an assertion: helpers are resolved before debug assertions are left out, one level as ever.

## Consequences
- clap's `cmd.debug_assert()` tests and tests asserting with project methods such as `.assert_success()` are no longer `test.no-assertion`.
- A project that runs its tests in release mode sets `debug_assertions = false` so that checks the build compiles out do not count.
- `describe tests.quality` and the README's settings table list `debug_assertions`; `tests.quality` is the second check, after `tests.mutation`, to configure a fact through a setting.
- Fixture: `crates/cupel-fixtures/repos/method-assertions/rust`.
