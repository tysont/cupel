# CUPEL_DECISION_198: A rule against a missing baseline is skipped with a `baseline.missing` finding, and the run may pass

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.8, the owner's rulings after v0.2.7 · Amends [195](CUPEL_DECISION_195.md)

## Context
ADR-195 let a run whose only failing rules lacked a reference count as passing, so a first baseline could be set. That was the implementer's reading; the owner has now ruled on it.

## Options
(a) ADR-195's reading: such rules fail, but the run counts as passing for `baseline set`. (b) Such rules are skipped, each with an info finding, and the run's gate may pass (the owner's ruling).

## Decision
A gate rule that compares with `base` when no baseline exists evaluates as skipped, not failed, and emits a `baseline.missing` finding at info naming the rule. The run's gate passes if every other rule does, so the first passing run on the default branch can become the baseline with no special case.

## Consequences
`baseline set` and `import` need only a passing run; ADR-195's exception for no-reference failures is no longer needed. v0.2.8 implements it; the plan's batch v0.2.8 Notes record the implementation and its tests.
