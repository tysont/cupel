# CUPEL_DECISION_179: A package's coverage and mutation use dependent packages' tests when a per-test map names them

**Date:** 2026-11-11 · **Status:** accepted; detailed by [194](CUPEL_DECISION_194.md); amended by [202](CUPEL_DECISION_202.md) (owner rulings, 2026-11-13) · **Phase:** v0.2.7, the owner's rulings on the Molten field trial · Amends [167](CUPEL_DECISION_167.md) and [173](CUPEL_DECISION_173.md)

## Context
With per-package roots, a root's coverage and mutation ran only that root's tests, so code another root's tests exercise read as uncovered. It was the largest source of false survivors on Molten: 15 of shared's 47, all killed by `apps/agent` tests.

## Options
(a) Own tests only, as before. (b) Include the tests of every dependent root, always: much slower, and unmeasurable without per-test attribution. (c) Include dependent roots' tests when a per-test map names them; otherwise own tests only, said in the finding (the owner's ruling).

## Decision
A root's coverage and mutation include tests from roots that depend on it when a per-test map names those tests as covering the code. Without a per-test map, they run the root's own tests only, and each affected finding says so ("own tests only; no per-test map").

## Consequences
Producers of per-test maps become worth more on monorepos. v0.2.7 implements it; the plan's batch v0.2.7 Notes record the implementation and its tests. v0.2.7's `covering_tests` wording ("all tests (no per-test map)") is the first half of saying so.
