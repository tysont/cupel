# CUPEL_DECISION_199: Ratchets on largest-value metrics allow +2 and never fail below the check's watch threshold

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.8, the owner's rulings after v0.2.7 · Amends [175](CUPEL_DECISION_175.md), [182](CUPEL_DECISION_182.md) and [195](CUPEL_DECISION_195.md)

## Context
Molten #661 added one `if` to a two-branch function; `static.complexity.cyclomatic_max <= base` failed at actual 3, bound 2, because over a small changed scope the base maximum is that same small function. ADR-182 gave counts δ = 0, which treats a largest value like a count.

## Options
(a) Keep δ = 0. (b) A larger δ alone. (c) δ = +2 for largest-value metrics, and no failure while the new value is below the check's watch threshold (the owner's ruling).

## Decision
`<= base` on a largest-value metric of a function (`static.complexity.cyclomatic_max`, `cognitive_max`, `nesting_max`) has the default tolerance δ = +2. Such a rule never fails while the actual value is below the check's watch threshold for that metric, whatever the base. An offset written in the rule still replaces δ. Molten's M8-3 `[gate.changed]` keeps `cyclomatic_max <= base` under these defaults.

## Consequences
A small change adding a branch or two to a simple function passes; a change that takes a function past watch, or more than two above its base, fails. v0.2.8 implements it; the plan's batch v0.2.8 Notes record the implementation and its tests.
