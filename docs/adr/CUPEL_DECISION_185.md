# CUPEL_DECISION_185: Mutation's unmutated baseline runs once per package per run and is reused while the suite is unchanged

**Date:** 2026-11-11 · **Status:** accepted · **Phase:** v0.2.7, the owner's rulings on the Molten field trial · Amends [080](CUPEL_DECISION_080.md) §9

## Context
With file-copy mutation, each file's mutants began with a reference run of its selected tests unmutated: 872 s of a 25-minute budget on Molten's `packages/shared`, about 20 s per file.

## Options
(a) Per file, as before. (b) Share one reference run across files with the same selected tests. (c) Once per package per run, stored by suite hash and reused across runs while the hash is unchanged (the owner's ruling).

## Decision
The unmutated baseline (the outcome and timing of each selected test) runs once per package per run. It is stored keyed by a suite hash over the package's test files, the sources they load, the runner command, and Cupel's version, and a later run reuses it while the hash is unchanged. Per-mutant timeouts derive from the stored timings.

## Consequences
A change to any test or loaded source changes the hash and reruns the baseline. v0.2.7 implements it; the plan's batch v0.2.7 Notes record the implementation and its tests.
