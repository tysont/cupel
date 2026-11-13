# CUPEL_DECISION_191: A survivor without a per-test map says "all tests" instead of listing them

**Date:** 2026-11-12 · **Status:** accepted · **Phase:** v0.2.7, item 4 · Details [179](CUPEL_DECISION_179.md), [133](CUPEL_DECISION_133.md)

## Context
`mutant.survived` named the tests that ran against the mutant in `evidence.covering_tests` and `action.hints.covering_tests`. With a per-test map those are the tests that cover the site. Without one every mutant runs the whole suite (less ADR-147's slow tests), so the list is the package's every test: 1,007 names per survivor on Molten's `packages/shared`. That is accurate, but it buries the finding and says nothing about the site. ADR-179 has such findings say that there was no per-test map.

The run schema is `v1`, additive within the major version (design §11). A consumer may read `covering_tests` as test IDs to run, so the field cannot become a sentence or change type.

## Options
(a) Keep the list. (b) Put the sentence "all tests (no per-test map)" in `covering_tests` as its one element: a consumer would run it as a test ID. (c) Change `covering_tests` to a string or an array: breaks typed consumers. (d) Keep `covering_tests` an array of test IDs, empty without a per-test map, and add a flag that says all tests ran.

## Decision
(d). Without a per-test map (`MutationOutcomes.per_test` false):
- the evidence of `mutant.survived` has `covering_tests: []`, `all_tests: true`, and `tests_run` counting the tests that ran; with a map, `all_tests` is false and `covering_tests` lists the selected tests as before;
- `action.hints` has `all_tests: true` and no `covering_tests` (both optional hint keys);
- `explain`'s `explanation.all_tests` is true, with `covering_tests` empty, and its text reads `covering tests  all tests (no per-test map)`.

What changes in meaning: `covering_tests` is the tests a per-test map selected, empty when every test ran. The new fields are additive, so the schema stays `v1`.

## Consequences
A survivor without a per-test map is a short finding; its tests are read from `tests_run` and the runner's suite. A consumer that ran `covering_tests` to reproduce a survivor runs nothing without a map and should run the suite when `all_tests` is true. The other half of ADR-179's wording, "own tests only; no per-test map" on coverage and mutation findings, is item 16's.
