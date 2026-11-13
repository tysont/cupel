# CUPEL_DECISION_193: Where `[roots]` narrow a run's scope, and a `--paths` value outside them

**Date:** 2026-11-12 · **Status:** accepted · **Phase:** v0.2.7, item 14 · Details [177](CUPEL_DECISION_177.md), [190](CUPEL_DECISION_190.md)

## Context
ADR-177 rules that with `[roots]` configured every check reads only files under some root, and that a file outside all roots is not offered by `--changed`. It leaves open where the narrowing happens, what a `--paths` value naming only files outside the roots does, and whether the import graph `tests --impacted` reads (the whole walk, ADR-190) narrows too.

## Options
**Where.** (a) Each check drops what lies outside the roots: every check, and every metric, must remember to. (b) The scope resolution narrows the walk before the scope flags select from it: one place, and checks see only files in scope, as they already do.

**`--paths` outside the roots.** (a) An empty scope that passes: an agent reads a green run on code nothing checked. (b) A usage error naming the roots, as a `--paths` value that matches no source file already is.

**The import graph.** (a) Narrow it too: an importer outside the roots no longer counts, so a test under a root that imports through a file outside it is missed. (b) Keep the whole walk for import graphs, as ADR-177's consequence lets module resolution read past the roots.

## Decision
1. The CLI's scope resolution (`scope::resolve`, shared by `check`, `survey`, `tests`, and a recomputed base) resolves the scope flags against the walk's files under some configured root, and drops a range's changes outside them. Without `[roots]` nothing is dropped.
2. A `--paths` value that names files of the walk but none under a root is a usage error: `` `--paths P` names no source file under a configured root (R, …) ``. A value that names some file under a root narrows to those files.
3. The whole walk stays the import graph's (`Scoped::tree`, `tests --impacted`, ADR-190); its tests and changed lines are those in scope.

## Consequences
- A glob across a root's boundary silently keeps only its part under the roots.
- `scope.files`, every metric, and every gate bound count only files under the roots; a check whose facts are produced by root (coverage, mutation) was already narrowed (ADR-167).
- **Tests:** `cupel-fixtures/tests/roots_scope.rs` (findings, metrics, `--changed`, and `--paths` with one root, and the whole repository without `[roots]`); `cupel-cli` `scope.rs` unit tests.
