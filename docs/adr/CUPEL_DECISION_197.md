# CUPEL_DECISION_197: `init` writes Node's own coverage command for a `node --test` package, and c8's only where c8 is a dev dependency

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.8, the owner's rulings after v0.2.7 · Amends [196](CUPEL_DECISION_196.md)

## Context
ADR-196 added `tool.node-test-coverage` but left `init` writing c8's command for every `node --test` root, so a root without c8 (Molten's `containers/sandbox`) got a coverage command that cannot run.

## Options
(a) Keep c8 as the default. (b) Node's built-in coverage always. (c) Node's built-in coverage unless c8 is already in `devDependencies` (the owner's ruling).

## Decision
For a `node --test` root, `init` writes `node --test --experimental-test-coverage --test-reporter=lcov --test-reporter-destination={out}`. It writes c8's command only when the root's `package.json` lists `c8` in `devDependencies`.

## Consequences
A `node --test` root gets a coverage command that runs with only `node` installed. v0.2.8 implements it; the plan's batch v0.2.8 Notes record the implementation and its tests.
