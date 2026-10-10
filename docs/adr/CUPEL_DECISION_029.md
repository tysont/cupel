# CUPEL_DECISION_029: API signatures as normalized token spans, and roots by reason

**Date:** 2026-10-10 · **Status:** accepted; last Consequences bullet superseded by [030](CUPEL_DECISION_030.md) (JavaScript and TypeScript test callbacks are symbols and `test` roots) · **Phase:** P1.8

## Context

Plan P1.8 asks for `ApiSurface` items "with normalized signatures" and `Roots` covering `main`, exported items, test functions, and framework hooks. "Normalized" is undefined: a signature must not change when parameters are renamed or reformatted, and must change when a public parameter is removed or retyped. Roots need a rule for which symbol a capture belongs to and how several reasons for one symbol are reported.

## Options

**Signatures.** Re-printing a signature from typed parts per language means a printer per language. Hashing the `NormTree` of the signature would be stable but unreadable in `static.api-surface` findings. Taking the tokens between the first and last `@api.signature` capture, replacing local names (`@api.local`) by `_`, dropping comments and trailing commas, and joining with one fixed spacing rule is language-neutral, readable (`(_: &[i64]) -> i64`), and stable under layout changes.

**Roots.** One root per symbol with the "strongest" reason loses information `static.dead-code` may want (a test that is also exported); one entry per (symbol, reason) keeps it at the cost of duplicates.

## Decision

- api.scm captures and the token-joining rule are as documented in `cupel_lang::api`. Items' paths use the symbols namespace rules. Rust reports plain-`pub` functions, inherent-impl methods, all items of `pub` traits, types, traits, constants, statics, and modules; restricted visibility, trait-impl methods, and `pub use` re-exports are not reported. Types' signatures are their generic parameters only.
- `Roots` has one `LiveRoot` per (symbol, reason), reasons in the order `entry-point`, `export`, `test`, `framework-hook`: entry points are functions named in `[roots].entry_points` or captured `@root.entry-point`; exports are symbols whose visibility is `public`; tests are symbols whose definition node is a `@test.case` node; hooks are symbols named in `[roots].hooks` or captured `@root.hook`. A capture belongs to the symbol whose definition node it is, else the innermost symbol containing it.

## Consequences

- Field changes of a public struct do not change its `ApiSurface` item; `static.api-surface` (P6.2) sees added and removed functions, methods, and signature changes, and would need a fact change to see fields.
- A test case that is not a symbol (an inline `it(…, () => …)` callback in JavaScript) is not a root; it has no symbol to keep alive.
