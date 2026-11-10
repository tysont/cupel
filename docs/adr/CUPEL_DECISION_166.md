# CUPEL_DECISION_166: A deferred producer is no candidate

**Date:** 2026-11-10 · **Status:** accepted · **Phase:** v0.2.6, Molten field trial Area 5 · Completes [162](CUPEL_DECISION_162.md) §2

## Context

ADR-162 gave the toolchain table `defer` rules. A JavaScript or TypeScript runner whose root's `scripts.test` names another command (Molten's `pnpm -r test`) is not detected there, and `init` writes no command for it.

`ProducerRegistry::candidates` does not drop an undetected producer. It orders detected producers first and keeps the rest as fallbacks, and the store takes the first ready one (ADR-076). So once Vitest is installed (Molten's `pnpm install`), a `standard` run of that repository with no pin still chose `tool.vitest` as a fallback and ran `vitest run` at the root: the sweep ADR-162 was written to stop, by another path.

Three things showed it:
- `doctor` on the Molten clone reported Vitest as required for `gen.property` and `tests.flaky` "via tool.vitest";
- `candidates` showed why;
- a fake Vitest that records its runs proved it.

## Decision

1. **`Producer::deferred(ctx)`**, false by default: the repository's own entry point runs something else where this producer would run. `candidates` leaves deferred producers out entirely, before ordering by detection. A pin in `[lang.<id>]` that names the producer still chooses it, because the pinned path returns first.
2. **Which producers defer.**
   - A runner defers by its row's `defer` rules (`toolchain::defers`).
   - A per-test map producer defers by its runner's, since it runs that runner's tests one by one.
   - Coverage producers need no rule: without a configured `coverage` command they are `Unconfigured` (ADR-079).
   - Bench producers run only at `deep` and are not yet given one.
3. **The test.** `crates/cupel-cli/tests/init.rs` builds Molten's shape: a root `pnpm -r test`, Vitest in a member, and a `node_modules/.bin/vitest` that records each run. A `standard` check must not run it. With the candidate filter removed, it runs.

## Consequences

- On a repository whose root `test` script names another command, no JavaScript or TypeScript runner runs unless it is pinned (`[lang.typescript] runner = "vitest"`). Coverage, mutation, flakiness, and property checks are skipped, because no runner produces test results.
- `doctor` no longer reports a deferred runner's tool as required.
- A bench producer whose runner is deferred could still run at `deep`. It is listed as the next place to apply the rule.
