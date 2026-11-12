# CUPEL_DECISION_184: Test-quality calibration from the Molten trial

**Date:** 2026-11-11 · **Status:** accepted · **Phase:** v0.2.7, the owner's rulings on the Molten field trial · Amends [054](CUPEL_DECISION_054.md), [055](CUPEL_DECISION_055.md), [069](CUPEL_DECISION_069.md), and [050](CUPEL_DECISION_050.md)

## Context
Area 3 and Area 4 left calibration questions: whether `toContain`, `toMatch`, and `toMatchObject` are weak (1,221 findings, 31% of Molten's tests); whether a loop with an assertion is test logic; the duplicate-setup threshold; and whether clones inside test files (about 540 of 655) are reported.

## Options
Each question had the options its Area recorded; the owner ruled on each.

## Decision
1. `toContain`, `toMatch`, and `toMatchObject` are not weak assertions.
2. A loop whose body contains an assertion is a table-driven test, not test logic. Branching (`if`, `switch`, a conditional expression deciding what is asserted) is logic.
3. `test.duplicate-setup` fires when at least 3 tests share at least 5 identical normalized setup statements.
4. Clones whose members all lie in test files are reported at info and are excluded from the clone gate (`static.clones.pairs`).

## Consequences
Molten's `test.weak-assertion`, `test.logic`, `test.duplicate-setup`, and clone counts drop. v0.2.7 implements it; the plan's batch v0.2.7 Notes record the implementation and its tests.
