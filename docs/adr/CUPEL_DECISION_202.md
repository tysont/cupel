# CUPEL_DECISION_202: Rust and Go per-test maps reach across a workspace's packages

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.8, the owner's rulings after v0.2.7 · Amends [179](CUPEL_DECISION_179.md) and [194](CUPEL_DECISION_194.md)

## Context
ADR-194 made Python's per-test map measure each root's tests on the other roots' files; the Rust and Go maps still measured only their own package, so an integration test in a sibling crate or package never counted for the code it covers.

## Options
(a) Python only. (b) Rust through nextest and llvm-cov per test, workspace-wide; Go through `-coverpkg` (the owner's ruling).

## Decision
Rust's per-test map runs each test under `cargo llvm-cov` with nextest across the whole workspace, so a test records every workspace crate's lines it ran. Go's runs each package's tests with `-coverpkg` naming every package of the module. Each language has a fixture whose integration test lives in a sibling crate or package.

## Consequences
A sibling crate's or package's test kills a mutant and covers a line, with a per-test map, as Python's already does. v0.2.8 implements it; the plan's batch v0.2.8 Notes record the implementation and its tests.
