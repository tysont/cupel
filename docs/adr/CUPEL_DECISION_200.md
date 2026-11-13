# CUPEL_DECISION_200: Chai's, Mocha's and Jasmine's membership, pattern and partial-object assertions are not weak

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.8, the owner's rulings after v0.2.7 · Amends [184](CUPEL_DECISION_184.md)

## Context
ADR-184 made Jest's and Vitest's `toContain`, `toMatch` and `toMatchObject` strong; Chai's equivalents (`include`, `contain`, `match`, `deep.include`) and Jasmine's stayed weak.

## Options
(a) Keep them weak. (b) Treat the equivalents the same as Jest's (the owner's ruling).

## Decision
Chai's `include`/`contain` (and `includes`/`contains`), `match`, and `deep.include`, `assert.include`/`assert.match`, and Jasmine's `toContain`/`toMatch`/`jasmine.objectContaining` are strong assertions.

## Consequences
Mocha-with-Chai and Jasmine suites score as Jest suites do for the same checks. v0.2.8 implements it; the plan's batch v0.2.8 Notes record the implementation and its tests.
