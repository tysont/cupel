# CUPEL_DECISION_173: With configured `[roots]`, mutation plans only the files under them

**Date:** 2026-11-10 · **Status:** accepted · **Phase:** v0.2.6, Molten field trial Area 7 · Extends [167](CUPEL_DECISION_167.md) §4 and [171](CUPEL_DECISION_171.md)

## Context

ADR-167 made editing `[roots]` the way to narrow a run to some packages, and ADR-171 made a runner run only in configured roots. The mutation engine still planned mutants over the run's whole scope, the repository's tree.

Area 7 ran `tests.mutation --depth deep --budget 25m` on the Molten clone with `[roots]` narrowed to `packages/shared`. The engine planned 18 165 mutants, 17 258 of them uncovered. Of those, 17 157 were in `apps/agent`, `apps/web`, `apps/cli`, `poc`, `scripts`, `tools`, and the deployed-environment test tiers: files no test the run could start reaches. The score was 130 killed over 17 435, 0.0075. Shared's own score, over its 278 mutants that ran or were uncovered, is 0.47.

`tests.coverage` reports only the files in its coverage map, so it showed no such files. Mutation counted every one as a miss.

## Decision

With configured `[roots]`, the mutation engine plans mutants only in the part of the scope under a configured root (`Scope::under`). Every directory a runner runs in is a configured root, so a file outside all of them is one no test of the run can reach: it is out of the run, not uncovered.

Without configured roots, the scope is unchanged.

## Consequences

- On the Molten clone, a run with `[roots]` naming `packages/shared` mutates only `packages/shared`, and its score is shared's.
- A file under a root of another language is still planned: the engine does not know its runner's languages, and a TypeScript root's Vitest runs its JavaScript files too.
- Static checks still read the whole tree. Roots narrow what is tested, not what is read.
- Test: `cupel-core/tests/scope.rs`, a scope under directories keeps only the files and symbols they hold.
