# CUPEL_DECISION_194: How a root's coverage and mutation take its dependents' tests from a per-test map, and say when they cannot

**Date:** 2026-11-13 · **Status:** accepted; amended by [202](CUPEL_DECISION_202.md) (owner rulings, 2026-11-13) · **Phase:** v0.2.7, item 16 · Details [179](CUPEL_DECISION_179.md), [191](CUPEL_DECISION_191.md)

## Context
ADR-179 rules that a root's coverage and mutation include the tests of roots that depend on it when a per-test map names them, and run the root's own tests only without one, saying so. It leaves open how. Before this, the two halves were the wrong way round:
- **No map:** a mutant ran `TestSelection::All`, which runs the suite in every root of the language, so a dependent root's tests did run against it. Only its coverage gate was own tests: the coverage map keeps each root's own files.
- **Map:** the per-test producers kept only the lines a root's tests ran in that root's own files, so the map never named a dependent root's test for a root's code, and a mutant ran its own root's covering tests alone.

## Options
1. Which roots count. (a) Configured `[roots]` only. (b) Any language with several project roots, configured or detected. Detected roots are kept apart the same way (a coverage map and a per-test map per root), so the rule applies to both.
2. A root's own suite. (a) Name its tests by ID: long command lines, and some runners cannot. (b) A selection of a root's whole suite, less the slow tests ADR-147 leaves out.
3. Coverage. (a) Coverage producers keep other roots' files: but then coverage counts dependents' tests with no map, against ADR-179. (b) Lines the per-test map says a test ran count as hit in the coverage map, for `tests.coverage` and for the mutation engine's coverage gate.

## Decision
1. **(b)**. A file's own root is the deepest project root of its language holding it, when the language has two or more (`ProjectInfo::own_root`); with one root the rule has nothing to add.
2. **(b)**. `TestSelection::Root { dir, except }` is the suite of the root at `dir` but `except`. Without a per-test map, or where the map names no test for a site, a mutant of a file with an own root runs that selection: the root's own tests. Its cache key folds in the root, so an outcome stored from a whole-suite run is not reused. The unmutated baseline (ADR-185) still runs and times the whole suite.
3. **Per-test maps measure other roots' files.** `tool.pytest-cov-per-test` measures each root with `--cov` for the language's other roots too. A file of another root is recorded relative to the measured root through `..` and placed at its repository path (`checkout/../fees/fees.py` is `fees/fees.py`). A cached root is measured again for the tests that ran a file of another root whose lines changed.
4. **(b)** for coverage. `CoverageMap::with_tests` counts an instrumented line that a test of the map ran as hit. `tests.coverage` (which now wants `PerTestMap` and `ProjectInfo`) scores with it. The mutation engine gates coverage with it, so a site only a dependent root's test runs is covered and runs that test.
5. **Saying so.** With an own root and no per-test map:
   - the evidence of `mutant.survived`, `mutant.uncovered`, `coverage.uncovered-symbol`, `coverage.unloaded-file`, and `coverage.uncovered-branch` carries `own_root`, the root's path;
   - the action's summary ends "(own tests of `<root>` only; no per-test map)";
   - `mutant.survived` and the coverage findings carry the hint `own_root`;
   - `explain` reads that hint as `explanation.own_root`, and its text's covering tests read `own tests of <root> only; no per-test map`.

   ADR-191's `all_tests` keeps its meaning: every test the mutant's selection could run did run. `own_root` says that selection was the root's own suite. All the fields are optional, so the schemas stay `v1`.

## Consequences
- Without a per-test map, a mutant in a multi-root language runs fewer tests than before: its own root's suite rather than every root's. It survives more often, and says why.
- With a map, a root's score and coverage count its dependents' tests (Molten's 15 false survivors in `packages/shared`).
- Open: the Rust (`tool.llvm-cov-per-test`, the joint suite run) and Go per-test producers still measure their own root's files only. So do the JavaScript and TypeScript producers, which have no per-test map at all. A file of another root that a root's coverage tool lists only when loaded (`Listing::Loaded`) gains no lines from the map.
