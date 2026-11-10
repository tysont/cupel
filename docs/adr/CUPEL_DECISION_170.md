# CUPEL_DECISION_170: Vitest coverage needs a coverage provider, and `doctor` names a missing package its producer reports

**Date:** 2026-11-10 · **Status:** accepted · **Phase:** v0.2.6, Molten field trial Area 6 · Amends [079](CUPEL_DECISION_079.md) (coverage availability) and [081](CUPEL_DECISION_081.md) (`doctor --fix`)

## Context

`vitest run --coverage` measures coverage through a provider package, `@vitest/coverage-v8` or `@vitest/coverage-istanbul`, which Vitest resolves as Node does: from the project directory upward, and from where Vitest itself is installed. Without one it stops before running a test. The toolchain table's install command for `tool.vitest-coverage` already adds `@vitest/coverage-v8`. But the producer judged itself ready when the `vitest` program was installed, and `doctor` decided by running the producer's probe, `vitest --version`.

On the owner's Molten repository, Vitest is installed in every package, but no package installs a provider. `init` writes `npx vitest run --coverage …` as the TypeScript coverage command, and `doctor` reported nothing. The first coverage run would have failed inside Vitest instead.

## Decision

1. **Providers.** `CoverageSpec` gains `providers`: Node packages of which the tool needs one. It is `@vitest/coverage-v8` and `@vitest/coverage-istanbul` for `tool.vitest-coverage`, and empty for every other spec.

   With the tool installed, a producer with providers is `MissingTool` (named for the first provider, with the table's install command) unless a provider's `package.json` is found in one of two places:
   - in `node_modules/<provider>` of the installed tool's real directory or any directory above it;
   - in that of every directory coverage runs in (the configured roots of its languages, else the repository root) or any directory above it.

   This is where Node would find it.
2. **`doctor` takes a producer's word for what is missing.** When the chosen producer is `MissingTool` for a name other than its probe's tool, `doctor` keeps the probe's requirement, which reports the tool's version. It adds a requirement for the missing name with that install command and no probe of its own, so it is missing.

   After `--fix` installs anything, `doctor` resolves its requirements again before probing. A producer the install made ready then needs nothing more.

## Consequences

- On Molten, `doctor` exits 3 naming `@vitest/coverage-v8` with `pnpm add --save-dev vitest @vitest/coverage-v8`.
- A machine with a global Vitest and its provider installed beside it (Cupel's dev environment) stays ready.
- Open: a coverage producer of another tool can still stand in as a fallback when it is ready. Examples are Jest's or c8's on a machine where `jest` or `c8` is on `PATH`, since a language's configured coverage command is shared. Detection of coverage producers is also judged at the repository root rather than the run's roots (as ADR-167 did for runners): on Molten the JavaScript roots, which run `node --test`, are attributed to Vitest's coverage producer.
