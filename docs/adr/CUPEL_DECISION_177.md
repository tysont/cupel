# CUPEL_DECISION_177: `[roots]` scope every check, static ones included

**Date:** 2026-11-11 · **Status:** accepted · **Phase:** v0.2.7, the owner's rulings on the Molten field trial · Amends [167](CUPEL_DECISION_167.md)

## Context
ADR-167 made configured `[roots]` decide which tests run and what is mutated, while the static checks still read every file of the repository's languages. In Area 11, `gate.sh shared` with only `packages/shared` configured gated `cyclomatic_max` on an `apps/agent` file under a header saying `roots packages/shared`.

## Options
(a) Keep the static scope whole and change the header: honest, but the gate still fails on code the configuration left out. (b) Every check's scope is the union of the configured roots (the owner's ruling).

## Decision
When `[roots]` is configured, every check, static checks included, reads only files under some root. A file outside all roots is out of scope entirely: not parsed for findings, not counted in any metric or bound, and not offered by `--changed`. Without `[roots]`, the scope is the whole repository, as before.

## Consequences
Module resolution may still read a file outside the roots to resolve an import's target, without reporting on it. `init`'s proposed roots now decide what a first run sees, so its proposals matter more. v0.2.7 implements it; the plan's batch v0.2.7 Notes record the implementation and its tests.
