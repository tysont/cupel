# CUPEL_DECISION_183: A property test that pins its own case count is respected and reported

**Date:** 2026-11-11 · **Status:** accepted · **Phase:** v0.2.7, the owner's rulings on the Molten field trial · Amends [095](CUPEL_DECISION_095.md)

## Context
All six of Molten's fast-check properties pass `{ numRuns }` to `fc.assert`, which wins over Cupel's `fc.configureGlobal` (ADR-095 §4). `--cases 1000` ran them at their own 120 to 300 cases and said nothing.

## Options
(a) Silent precedence, as before. (b) Override the test's count: changes what the author chose. (c) Respect it and say so (the owner's ruling).

## Decision
A property test that pins its own case count runs at that count. `gen.property` emits `property.cases-pinned` at info, naming the test and its count. `--cases` applies only to unpinned tests, and a falsified finding's `evidence.cases` is the count the test ran at.

## Consequences
Detection is per language, from the call's literal arguments; a count from a variable is reported as pinned with the count unknown. v0.2.7 implements it; the plan's batch v0.2.7 Notes record the implementation and its tests.
