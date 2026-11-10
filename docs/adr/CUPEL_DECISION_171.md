# CUPEL_DECISION_171: With configured `[roots]`, a language they name no root of runs nowhere

**Date:** 2026-11-10 · **Status:** accepted · **Phase:** v0.2.6, Molten field trial Area 6 · Corrects [167](CUPEL_DECISION_167.md) §1 and §3

## Context

ADR-167 made configured `[roots]` each language's project roots and said a `[roots]` that names no root of a language leaves that language without roots. Two fallbacks contradicted it:
- `runner::roots`, which every producer that runs a command uses to choose its directories, returned the repository root whenever `ProjectInfo` named none;
- `runner::run_dirs`, where detection and deferral are judged, did the same whenever no configured root was of the producer's languages.

Area 6 narrowed the Molten clone's `[roots]` to `packages/shared` (TypeScript) and ran `cupel check --only tests.coverage`. The TypeScript coverage ran in `packages/shared` as intended. But the JavaScript coverage producer, with no roots, took the repository root and ran the shared `npx vitest run --coverage` there. That is the sweep of every package and of the deployed-environment API tier that ADR-162, 166, and 167 exist to prevent.

It stopped before collecting a test only because Vitest could not resolve a coverage provider from the root. `MOLTEN_BASE_URL` pointed at a closed local port, as the trial's environment sets it.

## Decision

1. **No roots means nowhere.** When `[roots]` names any root:
   - `roots` returns exactly the language's configured roots, possibly none;
   - `run_dirs` returns exactly the configured roots of the producer's languages, possibly none.

   A producer with no directories runs nothing, is detected nowhere, and is deferred nowhere. Without `[roots]`, both still fall back to the repository root, as before ADR-167.
2. **The test.** `crates/cupel-cli/tests/init.rs` gives a workspace a JavaScript file at its root and `[roots]` naming one TypeScript package, with a coverage command that leaves a marker where it runs. Only the package may get the marker. With the fallbacks, the root got one too.

## Consequences

- Narrowing `[roots]` is now a safe way to choose where Cupel runs a project's commands: no language falls back to the repository root.
- A language whose files are present but which `[roots]` leaves out gets no runner, coverage, mutation, or analyzer results. Its static checks, which read files and run nothing, are unaffected.
