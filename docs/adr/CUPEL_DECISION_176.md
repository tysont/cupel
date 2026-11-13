# CUPEL_DECISION_176: A TypeScript package's API is what its `exports` map exposes

**Date:** 2026-11-11 · **Status:** accepted; detailed by [192](CUPEL_DECISION_192.md) · **Phase:** v0.2.7, the owner's rulings on the Molten field trial · Amends [101](CUPEL_DECISION_101.md) and [174](CUPEL_DECISION_174.md)

## Context
Area 11's one blocking false finding was `api.breaking` on a function moved inside `apps/agent`, a `"private": true` package whose `package.json` `exports` map names five entry files, none the moved one's. `static.api-surface` treated every exported item of every file as public, so a move no importer outside the package could see counted as a removal.

## Options
(a) Every file's exports are API, as before: simple, and wrong for any package with entries. (b) Exported items of files under the package's entry points and what they re-export (the owner's ruling). (c) Honor `private` as well and report nothing for private packages: drops real findings inside a monorepo whose packages import one another through their entries.

## Decision
For JavaScript and TypeScript, a package's API is what its `package.json` `exports` map exposes: the items its entry files define or re-export, followed through `export … from` and `export *` as ADR-174 follows them. With no `exports` map, the entries are `main` and `types` (or `typings`), with their re-exports. Nothing else is API surface: an exported item of any other file is internal, and moving, renaming, or removing it is not `api.breaking` or `api.widened`.

## Consequences
`ApiSurface` needs a per-language notion of a package's entries, read from the manifest at each side of the range. A package with neither an `exports` map nor `main`/`types` has no API surface. v0.2.7 implements it; the plan's batch v0.2.7 Notes record the implementation and its tests.
