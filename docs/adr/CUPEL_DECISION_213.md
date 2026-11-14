# CUPEL_DECISION_213: The Rust grammar is vendored, patched, and regenerated from its upstream tag, as ADR-161 does for JavaScript and TypeScript

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.2.9, item V17 · Applies [161](CUPEL_DECISION_161.md) to Rust

## Context

On clap-rs/clap, Cupel reported `lang.parse-error` for 13 test files (`tests/builder/error.rs:8:22`, `tests/builder/cargo.rs:36:57`, …). rustc and clippy accept all of them. Each uses snapbox's `str!` macro, which is common wherever snapbox is (clap, cargo, trycmd's users):

```rust
let m = str!("a");
assert_data_eq!(actual, str![[r#"
error: …
"#]]);
```

tree-sitter-rust lexes `str` as the `primitive_type` keyword wherever a primitive type can start, and `macro_invocation`'s name is a `scoped_identifier`, an `identifier`, or a reserved identifier. So `str!(…)` and `u8!(…)` in an expression or a statement are a call of `str` with an `ERROR` at the `!`. The same name inside another macro's token tree parses (`assert_data_eq!(x, str![])`), since a token tree is only tokens, and so does a path (`snapbox::str![…]`), since a primitive type cannot follow `::`.

Upstream:
- tree-sitter-rust 0.24.2 (2026-03-27) is the latest crates.io release and the one Cupel uses; its master is that tag.
- No issue or pull request on tree-sitter/tree-sitter-rust reports it (searched 2026-11-14 for `str!`, primitive and macro names, `macro_invocation`).

## Options

- (a) Wait for upstream: no report and no date.
- (b) Read the error away: drop `lang.parse-error` when the error is at a `!` after a primitive type. The tree around it is still wrong (a call, not a macro), and facts read from it would be too.
- (c) Vendor tree-sitter-rust with the rule patched and its parser regenerated, as ADR-161 does for JavaScript and TypeScript.

Chosen: (c).

## Decision

1. **The patch** is `vendor/patches/tree-sitter-rust-0.24.2.patch`, applied to the upstream tag v0.24.2 (`77a3747`). `macro_invocation`'s name also takes a primitive type (`bool`, `str`, `char`, and the numeric types), aliased to `identifier`, so the tree and Cupel's queries see a macro named by an identifier. The grammar already aliases primitive types to `identifier` in expressions, patterns, and paths; the macro name was the one place it did not. Generation reports no new conflict, and a primitive type parses as `primitive_type` wherever it is a type (`let s: &str`, `fn f(x: u8)`, `<str as ToString>`); `u8::MAX` stays a path.
2. **The generated crate** is `vendor/tree-sitter-rust`, wired in through the workspace's `[patch.crates-io]` and left out of the workspace, as the other two are.
   - It is the upstream crate's `include` set at its tag (`LICENSE` (MIT), `bindings/rust`, `grammar.js`, `queries`, `src`, `tree-sitter.json`), its parser regenerated after the patch.
   - The tree-sitter CLI is 0.26.7, the version the tag's `package-lock.json` pins.
   - The published `parser.c` was generated at `b3e615d`, the commit before the version bump: regenerating that commit with 0.26.7 (and with 0.26.6 or 0.26.8) reproduces the published `parser.c`, `grammar.json`, and `node-types.json` byte for byte. Regenerating the tag itself differs from the published file only in the language metadata's patch version (0.24.2, not 0.24.1). The crate's `.cargo_vcs_info.json` names a commit (`e2bee85`) that is not in the upstream repository, but every file of the crate equals the tag's.
3. **`scripts/grammars.sh` rebuilds it** with the other two, so an upgrade is ADR-161's: bump the tag and CLI version, rebase the patch, rerun the script, rerun `crates/cupel-lang/tests/rust_parse.rs` and the Rust fact goldens. When upstream accepts a primitive-named macro, the vendored crate goes.
4. **Checks.**
   - Upstream's corpus passes on the patched grammar: 151 of 151, as on the unpatched tag.
   - `crates/cupel-lang/tests/rust_parse.rs` holds `str!("a")`, `str![[r#"…"#]]` in a call's argument, and `u8!(x)` as a statement, beside the types that must keep their meaning, and pins both trees.
   - `crates/cupel-fixtures/tests/primitive_macro.rs` runs Cupel over `repos/primitive-macro/rust`, a crate whose test uses a local `macro_rules! str`.

## Consequences

- clap's 13 parse errors are 0. No fact golden changed, so code without a primitive-named macro parses as before.
- The repository carries about 7 MB more of generated C, marked `linguist-generated` by the existing `vendor/**` attributes.
- ADR-161's consequences for `[patch.crates-io]` hold for Rust too: builds from this workspace apply it, a crates.io build of the library crates would not. `cupel-mutate`'s Rust schemata use the same patched grammar, since `[patch.crates-io]` applies to the whole workspace.
