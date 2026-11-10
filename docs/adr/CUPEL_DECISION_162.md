# CUPEL_DECISION_162: A run makes a check's wants only within its depth, and a root's own test script can defer a runner

**Date:** 2026-11-09 · **Status:** accepted; §2 completed by [166](CUPEL_DECISION_166.md) (a deferred producer is no candidate) · **Phase:** v0.2.6, Molten field trial Area 1b · Extends [080](CUPEL_DECISION_080.md) §2 and [154](CUPEL_DECISION_154.md) to a check's wants; amends [021](CUPEL_DECISION_021.md) §4 (the commands `init` writes) and [036](CUPEL_DECISION_036.md) (detection)

## Context

During the Molten field trial (the owner's `../molten`, a pnpm TypeScript monorepo), `cupel survey --depth quick` ran Molten's whole Vitest suite. That included `tests/api/**`, a tier that targets a deployed environment (`https://int.molten.live` on branch `main`), and three of its files made real requests. Two Cupel behaviors combined:

1. **A check's wants were made whatever they cost.** `static.crap` is a `quick` check that wants `CoverageMap`. The scheduler made a producer's wants only within the run's depth (ADR-080 §2), and a check's needs only within its bound (ADR-154). But it made a check's wants unconditionally (`serve`, "which a run makes whatever they cost"). So a `quick` run ran the configured coverage command, which runs the test suite. The README said so ("`static.crap` wants a coverage report even at `quick`"); no decision had weighed it.
2. **`init` wrote a root command the project does not run.** Molten's root `package.json` has `"test": "pnpm -r test"`, which runs each package's own Vitest configuration (one of them a Workers pool). Its root also declares Vitest, for the API tier's own config. The toolchain table selected `tool.vitest` from that dependency and wrote `pnpm vitest run` and its coverage form at the root. Vitest with no root config takes its default include, so it swept every package and `tests/api/**`, under no package's configuration. A run without `cupel.toml` behaves the same way, because runners and coverage tools fall back to the table's detection (`toolchain::detects`).

A `quick` run should execute nothing of the project, and Cupel should run a project's tests only the way the project runs them.

## Options

**Check wants.**
- (a) Keep them unconditional, and document that `quick` can run tests.
- (b) Gate them as ADR-154 gates needs. A want is made when it is already made, or when its chosen producer estimates the cost within the run's depth (or within the check's own cost under `--only`). Otherwise it is left out, and the check runs without it.

Chosen: (b). It is the rule ADR-080 already applies to producers, so one rule covers every optional fact. `static.crap` already runs without a coverage map (it then reports no score). And a fresh cached report still costs `quick` to read, so it is still used.

**A root whose own test script runs something else.**
- (a) Write commands per package. That is right in the end, but it is the per-root design change ADR-021 deferred; it is the trial's Area 5.
- (b) Have the table defer a runner where the root's own entry point names something else. `init` then writes no test or coverage command for the language, says why, and run-time detection agrees.
- (c) Exclude known deployed-environment directories by name. That is a guess about one project's layout.

Chosen: (b) now, (a) in Area 5.

## Decision

1. **A check's wants.**
   - The scheduler makes a want of a check (its manifest's `wants`) for a language only when one of these holds:
     - it is already made;
     - the producer the store would choose estimates its cost within the larger of the run's depth and the check's own cost (`FactStore::affordable_wants`).
   - The same set feeds the plan's and the budget's reach (`manifest_kinds`), so estimates count only what the run will make.
   - A dry run names a left-out want with its cost, as ADR-083 names a producer's: `coverage-map(standard)`.
2. **`defer` rules.**
   - A tool row of the toolchain table may list `defer` rules. When a tool's `detect` selects it but one of its `defer` rules holds, the tool is not eligible there. This applies to `init`'s selection and to run-time detection (`toolchain::detects`) alike.
   - The JSON rule gains `lacks`: the value at `path` is a string that does not name the package.
   - Each JavaScript and TypeScript runner (`tool.vitest`, `tool.jest`, `tool.mocha`, `tool.node-test`) defers when the root's `package.json` has a `scripts.test` that does not name it.
   - A root with no `test` script, or one that names the runner, keeps it.
   - `describe language` shows each tool's `defer` rules.
3. **`init`'s report.** A language whose first root had a runner deferred gets no `test` or `coverage` command. `init` adds an `info` finding `init.runner-deferred` with:
   - subject `<lang>:<path>`;
   - action `configure`;
   - evidence `{lang, path, runner, deferred_by}`, the rules in words.
   The fix says to add `test` and `coverage` under `[lang.<id>]` for the packages Cupel should run.

## Consequences

- A `quick` run executes no project code to make an optional fact. `static.crap` at `quick` scores CRAP only from a coverage report that is already fresh. The README's daemon section is corrected.
- Molten's `init` writes no TypeScript test or coverage command, and reports `init.runner-deferred` naming `tool.vitest`. Its `standard` runs skip coverage and mutation as unconfigured until the owner names per-package commands, or until Area 5 writes them.
- A project whose root `test` script calls a wrapper (`npm run test:unit`) defers every runner too. That is the safe reading: Cupel cannot see what the wrapper runs, and `init` says so.
- Another ecosystem's equivalent (a Makefile target, a Python `tox.ini`) has no `defer` rule yet. Each is added to its row when a real repository needs it.
