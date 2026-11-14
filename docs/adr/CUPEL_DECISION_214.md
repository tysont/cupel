# CUPEL_DECISION_214: A Rust test asserts through a helper it calls by module path when the file that path names defines that helper asserting

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.2.9, item V19 · Details [138](CUPEL_DECISION_138.md) §8 (a Rust test that calls a helper of its file asserts), [207](CUPEL_DECISION_207.md) (a Go test asserts through a method of its package)

## Context
ADR-138 §8 made a Rust call by plain name assert when a function of the same file of that name asserts, and left every other file out: "each integration test is a crate of its own, and a plain call is no assertion until a helper of its file makes it one". Rust's idiom for helpers shared by several test files is a module of the test crate, called by path, and it fell outside that rule on clap-rs/clap:

- `clap_complete/tests/testsuite/bash.rs:253` `register_completion() { common::register_example::<RuntimeBuilder>("static", "exhaustive"); }`, and the seven tests like it in the suite's bash, elvish, fish, and zsh files (8 in all), were `test.no-assertion`. `tests/testsuite/main.rs` makes every file of the directory a module of one test crate (`automod::dir!`), each file has `use crate::common;`, and `tests/testsuite/common.rs` defines `register_example`, which calls `assert!(…)` and `snapbox::assert_subset_eq(…)`.
- The other common shape is Cargo's documented `tests/common/mod.rs`, which each integration test file loads with `mod common;` and calls as `common::setup()`.

A path call does not say what it names: `common::check(…)` may name a module of the crate, an external crate (`snapbox::cmd::compile_example`), or a type's associated function (`Command::new`). The queries have no name resolution, but the resolver sees every file of the language in scope (`resolve_helpers`), so it can look for the file a module path names.

## Options
**Which path calls are helper calls.** (a) A call through a module path is a helper call by its last segment, which asserts only when a function of that name, in the file the path names, asserts; otherwise none (ADR-207 Option a). (b) Every path call into a module of the repository asserts, custom and strong (ADR-207 Option b): every setup function of a shared module (`common::command()`, `common::inputs()`) would assert, hiding tests that assert nothing. (c) Resolve `use` declarations and `mod` items to the module tree Cargo builds: the file a path names exactly, at the cost of a module resolver the TestFrames producer does not have.

**Which file a path names.** (a) The path's segments, without a leading `crate`, `self`, or `super`, as a file below the calling file's directory or the nearest ancestor that holds one: `<path>.rs` or `<path>/mod.rs`. (b) Any function of that name in the calling file's directory tree: a helper of an unrelated module with the same name would count.

## Decision
1. **A call through a module path** (`m::f(…)`, `m::f::<T>(…)`, `crate::m::f(…)`, `super::m::f(…)`, `m::n::f(…)`, other than an `assert_…` name, which is an assertion by convention) is a helper call by its last segment and not an assertion by itself (Option a). Rust's `tests.scm` marks the path `@assertion.module` beside the name's `@assertion.helper`. A function of the calling file of the same name does not resolve it: the path names another module.
2. **The file the path names** is found by Option (a): its segments below `crate`, `self`, and `super`, joined by `/`, as `<path>.rs` or `<path>/mod.rs` in the calling file's directory, else in the nearest ancestor directory that holds one, among the files in scope. That is the file `mod m;` loads from a crate root or a `mod.rs` beside the caller (clap's `tests/testsuite/common.rs`, `tests/common/mod.rs`), and from a module nested below it (`crate::common`, `super::super::common`), and the nearest of them where a `#[path]` attribute loads a shared file from above. The call asserts, with the class of that function's first strong assertion, else its first, when the file defines a function of that name that asserts; otherwise it asserts nothing.
3. **One level,** as ADR-207 §4: a helper's assertions through its own path calls or helper calls are not followed.

## Consequences
- clap's eight `register_completion`- and `register_dynamic_env`-style `test.no-assertion` findings in `clap_complete/tests/testsuite/` go away under a whole-tree run; a test that only calls a setup function of `common` stays `test.no-assertion`.
- A `--changed` run whose scope holds a test but not the module file its helper is in reads that test as asserting nothing, as ADR-207's method helpers and Python and ECMA helpers do.
- A path naming an external crate or a type (`snapbox::…`, `Command::new`) names no file in scope and asserts nothing, as before. A module whose file a `#[path]` attribute puts elsewhere than item 2 looks, or a module declared inline (`mod common { … }`) in another file, is not followed. A call by `super::f(…)` or `crate::f(…)` (a function of the parent module or the crate root) is not a helper call.
- Names are matched by the last segment only: a re-export (`pub use inner::check;`) in the named file is not followed.
