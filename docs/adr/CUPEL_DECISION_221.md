# CUPEL_DECISION_221: A failure under a condition pins that condition: Go's `if <cond> { t.Fatal }` and Rust's `if <cond> { panic! }` are strong

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 2 · Amends [032](CUPEL_DECISION_032.md)

## Context
The v0.2.9 validation found 25 `test.weak-assertion` findings on hashicorp/raft (Go), 13 of them truthiness only, such as `configuration_test.go:65`: `if hasVote(cfg, "id0") { t.Fatalf(…) }`. ADR-032 classifies a Go failure call by the condition guarding it, and every condition that is not one of its shapes (`err == nil`, `err != nil`, `x == nil`, `got != want`, `got == unwanted`, the standard library's equality calls, `errors.Is`) is weak truthiness. NOTES Q-go-2: "Go has no implicit truthiness: the condition is a bool, so the test pins the predicate's exact value." On clap (Rust), Q-rs-3 made the same point for Rust, which has no truthiness either. A Rust test that fails by `if !cond { panic!(…) }` is no assertion at all today: `tests.scm` has no `panic!` pattern. The owner's ruling (2026-11-14): "(if pred() { t.Fatal } / if !cond { panic }): strong assertion."

## Options
(a) Strong for every failure call, guarded or not. A bare `t.FailNow()` in a `select`'s `default` or a bare `panic!` says only that a line was reached; the ruling names the guarded form. (b) Strong for a failure call that is a statement of an `if`'s consequence whose condition no more specific idiom names; the specific idioms (`not-nil`, `no-error`, `error`, `not-equal`) keep their classes, being placed earlier. (c) Also make `assert!(pred())` strong: the ruling names the `if` form only, and `assert!` keeps ADR-070's classes.

Chosen: (b).

## Decision
1. **Go.** A failure call (`Error`, `Errorf`, `Fatal`, `Fatalf`, `Fail`, `FailNow` on `t`, `b`, or `tb`) that is a statement of the consequence block of an `if` is the idiom `guard`, `truthiness`/`strong`, unless an earlier idiom pattern of `tests.scm` names its condition (`err == nil` throws weak, `err != nil` and `x == nil` not-null weak, `got == unwanted` equality weak, and the strong equality and `errors.Is` shapes). A failure call anywhere else (a `select` or `switch` arm the case idioms do not name, an `else` block, a bare call) stays `fails`, `truthiness`/`weak`. testify's `True` and `False` are unchanged.
2. **Rust.** A `panic!` invocation that is a statement or the final expression of the consequence block of an `if` whose condition is a boolean expression, in a test, is the idiom `guard`, `truthiness`/`strong`. An `if let` condition is a pattern match, the `.expect(…)` a test spells out, and stays no assertion as `expect` is; a bare `panic!` and `unreachable!` stay no assertion.
3. As every assertion, the guarded failure makes its `if` a guard rather than logic (ADR-054).

## Consequences
- 13 of raft's 25 weak-assertion findings were truthiness only; those whose guards are predicates, negated flags, or orderings now count as strong and are no longer reported unless the test also holds only weak assertions elsewhere.
- A Rust test asserting by `if !cond { panic!(…) }` reads as asserting strongly instead of asserting nothing.
- `assert!(pred())` in Rust stays weak truthiness (Q-rs-3's own example): the ruling names the `if` form; a ruling on `assert!` would be a separate change.
- Goldens whose Go fixtures fail under a predicate change (`TestMembersGetADiscount` in `repos/go` is `truthiness`/`strong`).
