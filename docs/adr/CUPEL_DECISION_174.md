# CUPEL_DECISION_174: `static.api-surface` follows re-exports and counts trailing parameters a caller may leave out as widening

**Date:** 2026-11-10 · **Status:** accepted · **Phase:** v0.2.6, Molten field trial Area 8 · Amends [101](CUPEL_DECISION_101.md)

## Context

Area 8 replayed ten Molten PRs that changed `packages/shared` as `cupel check --depth quick --base <c>^ --head <c>`. `static.api-surface` reported seven `api.breaking` findings. None of them broke a caller.

- **Two were moves behind a re-export.** #622 moved `CLAUDE_MODELS` and `CAPABILITY_MODEL` from `constants.ts` to a new `models.ts`, and `constants.ts` re-exports both (`export { CAPABILITY_MODEL, CLAUDE_MODELS } from "./models.js"`). The check compared the items each file defines. A re-export defines nothing, so both read as removed.
- **Five added trailing parameters a caller may leave out.**
  - #628 gave `reviewNotice` a third parameter with a default (`consumes = []`).
  - #623 gave three `Collector` methods a defaulted `LlmCallBilling` parameter, and the `UsageMeter` interface method an optional one.

  ADR-101 counts a signature missing at head as changed, so each was breaking.

  For TypeScript, a call without the new parameter still type-checks. So does an implementation of the interface method that does not declare it.

A gate holding a branch to `no-breaking` would have failed three of the ten PRs for nothing.

## Decision

1. **Re-exports.** `static.api-surface` wants the head `ModuleGraph`. For a base path with no item at head, the check looks at its base file's head re-export edges (ADR-068's `re-export` kind). If one exports the path's name (by name, under an alias, or with `export *`), the check compares the base items with the head items of the re-exported file at the same path from the re-exported name.
   - Same signatures: no finding. Others: `changed`.
   - When a named re-export's file is not in the head surface (the range did not change it), the path is taken as unchanged.
   - Without a module graph, or with no re-export of the name, a path missing at head is `removed`, as before.
2. **Trailing parameters a caller may leave out.** A base signature with no exact match at head matches a head signature of the same kind that:
   - takes the same parameters, then more, each of which a caller may leave out;
   - has the rest of its signature (its return type, and for a Go method its receiver's method) unchanged.

   A parameter a caller may leave out is optional (`_?`), variadic (`...`, `*`), or defaulted (a `=` at its top level, outside brackets and quotes, not part of `=>`, `==`, `!=`, `<=`, or `>=`). Such a path is `extended`, which widens.

## Consequences

- On the replay, #622 reports only the new `models.ts` paths, as `api.widened`, and #623 and #628 report their methods as widened.
- **Re-export chains.** Only one re-export is followed. A path moved behind two (an `index.ts` re-exporting a module that re-exports another) is compared with what the first names, and it is `removed` if that holds nothing at the path.
- **Overload resolution.** A trailing defaulted parameter can still change which overload a call resolves to. Cupel does not model overload resolution, so it is widening here.
- **Tests:** `cupel-checks/tests/static_api_surface.rs`:
  - an item its module re-exports by name, alias, or `export *` is not removed;
  - a re-exported item is compared with its target;
  - trailing parameters a caller may leave out widen a signature, and a required, changed, or Go one does not.
