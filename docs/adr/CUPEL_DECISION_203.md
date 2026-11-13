# CUPEL_DECISION_203: `test.logic` and `test.duplicate-setup` have fixture repositories at the calibrated thresholds

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.8, the owner's rulings after v0.2.7 · Details [184](CUPEL_DECISION_184.md)

## Context
After ADR-184, no language's fixture triggered `test.logic` or `test.duplicate-setup`; both had unit tests only.

## Options
(a) Unit tests only. (b) A fixture repository for each at the new thresholds (the owner's ruling).

## Decision
Fixture repositories exercise `test.logic` (a branch in a test, beside a loop holding an assertion that is not logic) and `test.duplicate-setup` (three tests sharing five setup statements, beside two that do not fire), end to end.

## Consequences
Both finding kinds are covered through the binary. v0.2.8 implements it; the plan's batch v0.2.8 Notes record the implementation and its tests.
