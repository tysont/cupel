# CUPEL_DECISION_005: Go schemata timing — not in P4

**Date:** 2026-10-15 · **Status:** accepted · **Phase:** P4.1

## Context

Design §9.3 item 4 names Rust and Go as the first languages for mutant schemata, and §14 open question 3 asks whether Go's schemata belong in P4 or P6: a rewritten expression can change Go's type inference, and the fixtures were to be measured before committing. P4.1 built Rust schemata (ADR-082): every gated expression becomes `if cupel_mutant_id() == N { mutated } else { original }`, which works because in Rust `if` is an expression whose branches take the expected type of its place, so the rewrite needs no type information. The question here is whether the same is possible for Go, at what cost, and what it would buy.

## Options

Measured on `repos/tests.mutation/go` (`Discount`: `total >= 100 && member`, `total - 10`, `total * 1`; Go 1.23.2, this machine) by rewriting its five mutants by hand into a schemata build and running `go test` with each `CUPEL_MUTANT`.

**What Go's grammar allows.** `if` is a statement in Go, so an expression site cannot be wrapped in place. Three forms remain. (1) For sites whose value is `bool` (relational, equality, `&&`/`||`), an immediately called literal, `func() bool { if cupelMutantID() == N { return a > b }; return a >= b }()`, keeps short-circuiting and needs no type information. (2) For arithmetic, a function literal must spell its parameter and result types, which only type checking knows; the type-free alternative is a generic helper per operator pair, `cupelAddSub[T number](id, a, b T) T`, whose `T` Go infers from the operands. (3) For literal sites (`0`/`1`), `cupelPick(id, 0, 1)`. With these, the fixture's schemata build compiled and every mutant gave the file-copy outcome (`>=`→`>` and `*`→`/` survived, the rest killed).

**Where the type-preservation rule breaks.** (a) *Untyped constants become typed run-time values, silently.* `0.1 + 0.2` is exact constant arithmetic (0.3); `cupelAddSub(1, 0.1, 0.2)` adds two `float64` at run time (0.30000000000000004). The schemata build compiles and the unmutated program behaves differently, so the validation run is the only guard, and only if a test notices. The same holds for constant expressions that overflow a typed integer but not arbitrary-precision constants. (b) *Default types from generic inference.* With only untyped operands, `T` takes the default type: `var half float64 = cupelMulDiv(1, 1, 2)` is `int`, a compile error (`cannot use … (value of type int) as float64 value`), as are `f * cupelPick(2, 0, 1)` with `f float64` and `var b byte = 'a' + cupelPick(3, 0, 1)`. These fall back cleanly, but in ordinary Go code (floats, `time.Duration`, bytes) a large share of literal and arithmetic sites would. (c) *`:=`* is not itself a problem: `x := 1` and `x := cupelPick(1, 0, 1)` both infer `int`. Avoiding (a) and (b) needs type information for every operand — whether an identifier names a constant, what type a place expects — which means running `go/types` through a Go helper program, a new kind of dependency for the engine.

**What it would buy.** Go builds are fast and cached. On the fixture a `go test` after editing the file took 0.66–0.71 s against 0.60–0.68 s for the same run with only `CUPEL_MUTANT` changed: the rebuild a schemata build saves is under a tenth of a second. Rust's rebuild of a crate and its dependents is minutes on Cupel.

## Decision

Go stays on file copies in P4. `schemata::rewriter("go")` is `None`. Go schemata are reconsidered no earlier than P6, and only with type information (a `go/types` helper) so that untyped-constant sites are excluded rather than silently changed; the `bool`-typed form (1) alone could be done without it, but would save almost nothing.

## Consequences

- No Go code path in the engine to keep correct against Go's constant semantics.
- A Go mutant keeps costing one incremental `go test` build, which on the fixtures is a fraction of a second; per-test selection, not schemata, is what shortens Go mutation runs.
- The design's "Rust and Go first" for schemata is Rust only until a later phase decides otherwise.
