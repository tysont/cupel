# CUPEL_DECISION_186: `tests.flaky` reruns only impacted tests

**Date:** 2026-11-11 · **Status:** accepted · **Phase:** v0.2.7, the owner's rulings on the Molten field trial · Amends [078](CUPEL_DECISION_078.md)

## Context
In Area 11, a pure deletion's gate ran `tests.flaky` over all of `apps/agent` (about 300 s).

## Options
(a) Rerun the whole suite, as before. (b) Rerun the tests `tests --impacted` selects for the run's scope (the owner's ruling).

## Decision
`tests.flaky` reruns only the tests impacted by the run's scope, as `tests --impacted` selects them. With no impacted tests, the check is skipped with that reason. A whole-tree run (no changed scope) still reruns every test.

## Consequences
A gate on a change that touches no tested code no longer pays for reruns. v0.2.7 implements it; the plan's batch v0.2.7 Notes record the implementation and its tests.
