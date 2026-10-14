# CUPEL_DECISION_069: Loops over a table of cases written in place are not logic in tests

**Date:** 2026-10-14 · **Status:** accepted; supersedes the table-driven consequence of [054](CUPEL_DECISION_054.md) · **Phase:** P2.10

## Context

ADR-054 counts every loop in a test as `test.logic` and accepted that Go's table-driven tests are reported (relaxable with `logic_max = 1`). On the first Cupel dogfood, 135 of 162 `test.logic` findings were loops, most of them tables of cases in Rust (`for (args, flag) in [(…), (…)] { assert_eq!(…) }`), the idiom Rust and Go use where other languages have parametrized tests. `static.clones` meanwhile reports near-identical sibling tests whose fix is exactly such a table, so the two checks pulled in opposite directions. The project owner ruled that iteration over a literal array, vector, or slice of cases is table-driven, not logic, and that Go's table-driven case must follow the same rule. `Decision` carried no way to tell such a loop from any other.

## Options

1. Keep counting every loop; users relax `logic_max`. Hides real loops along with tables.
2. A `Decision` span, letting the check see whether a loop contains an assertion. A loop computing expected values around an assertion would still read as a table.
3. A semantic flag on the loop's decision, decided by each language's `decisions.scm`: the loop iterates over a table of cases written in place.

## Decision

Option 3.

- `Decision.table: bool` (`Decisions` version 2): true for a loop over a literal collection written in the loop header, or over a local bound to one by a declaration earlier in the same block; false for every other decision.
- `decisions.scm` marks such loops with the generic capture `@loop.table` on the loop node. Rust: `[…]`, `&[…]`, `vec![…]`, `[…].iter()`/`.into_iter()`, or a `let` local bound to one of the first three. Go: `range` over a composite literal, or over a local declared from one with `:=`. Python: `for` over a list or tuple display, or over a name assigned one. JavaScript and TypeScript: `for … of` an array literal, or a `const`/`let` bound to one. Ranges (`0..3`, `range(3)`), function results, and fields are not tables.
- `tests.quality` counts only loops that are not tables toward `test.logic`; evidence keeps `loops` (now the non-table loops) and gains `tables` (table loops). Branches inside a table loop still count (beyond the guard allowance of ADR-054), so a table whose body decides what to assert is still logic.

## Consequences

- Go's `cases := []struct{…}{…}; for _, tc := range cases { … }` is no longer reported; its guarded `t.Errorf` stays a guard. Each fixture repository keeps a table-driven test that must not fire and gains one loop over computed values that does (`test.logic` per language).
- A table built by a helper function or held in a `const`/`static` item is not recognised as a table; inline the literal or bind it locally.
