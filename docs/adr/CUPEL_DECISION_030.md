# CUPEL_DECISION_030: JavaScript and TypeScript symbols: test callbacks, default exports, and CommonJS exports

**Date:** 2026-10-10 · **Status:** accepted; supersedes the last Consequences bullet of [029](CUPEL_DECISION_029.md) ("a test case that is not a symbol … is not a root") · **Phase:** P1.2–P1.8

## Context

Symbols are the join key for every per-function fact (ADR-014). In Rust, Go, and Python a test case is a named function, so it is a symbol, owns its decisions and mutation sites, and is a `Roots` entry with reason `test` (ADR-029). In JavaScript and TypeScript a test case is an anonymous callback passed to `it`/`test` (vitest, jest, mocha, `node:test`); ADR-029 notes that such a callback "is not a root; it has no symbol to keep alive". Left that way, a JavaScript test's branching and assertions belong to no symbol, `tests.quality` cannot weigh a test by its complexity, and `conformance/roots.toml`'s `test` construct cannot be exhibited at all. Two further ECMAScript definitions have no declared name: `export default function () {}` and the CommonJS assignment `exports.name = function () {}`.

## Options

**Test callbacks.** (a) Leave them anonymous: tests are frames without symbols, unlike every other language. (b) Make the `it(…)` call the symbol: its span includes the title and modifiers, but it is not a function. (c) Make the callback the symbol, a closure named by the test's title, namespaced by enclosing `describe` suites, with `@test.case` on the same callback node so `Roots` joins by node.

**Anonymous default exports.** Skip them, or name them `default`, the name every importer uses.

**CommonJS exports.** Skip them, or treat `exports.x = function …` and `module.exports.x = …` as public closures named `x`, the binding being the assignment.

## Decision

- A callback passed as the second argument of `it`, `test`, `specify`, `xit`, `xtest`, or `fit` (also as `it.skip`, `test.only`, …) right after a string title is a `closure` symbol whose name is the title's text and whose node is the callback. `describe`, `suite`, `context`, `xdescribe`, and `fdescribe` with a string title are namespace scopes, so `describe("add", () => it("adds", …))` in `test/math.test.js` is `test.math.add.adds`. tests.scm puts `@test.case` on the same callback node, so `TestCase.span` equals the symbol's span and test cases are `Roots` with reason `test`. Hooks (`beforeEach`, …), `test.each(table)(…)`, and titles written as template strings are not symbols.
- `export default function () {}` and `export default (…) => …` define a public `function` named `default`; a named default-exported function keeps its name.
- `exports.x = <function>` and `module.exports.x = <function>` define a public `closure` named `x`.

## Consequences

- Test callbacks own their decisions and mutation sites, have structural hashes, and are live roots, as Rust `#[test]` functions are. A title containing the separator `.` makes a display name ambiguous to read; identity uses the structural hash and `SymbolKey`, so nothing depends on parsing displays.
- Renaming a test changes its display name but not its hash, as for any symbol.
- Inline callbacks that are not tests (`items.map((x) => …)`) remain part of their enclosing symbol.
