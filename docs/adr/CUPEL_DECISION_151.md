# CUPEL_DECISION_151: Tests attributed by declaration to the files they own

**Date:** 2026-10-30 · **Status:** accepted · **Phase:** P10.3

## Context

ADR-083 §3 and ADR-084 §7 make a cached test stale when it executed a changed function, and ADR-149 reruns exactly those tests on a range scope. A test that spawns the repository's own binary executes, in the binary, every line the binary runs for it: ADR-083 §1 exports a test process that started children with every executable the build made, so the test's coverage names most of the crates of the workspace. On Cupel, the integration tests of `cupel-cli` and the trials of `cupel-fixtures` run `cupel check` over fixture repositories; each covers the check modules, the scheduler, and the adapters, so a one-function change in almost any crate makes most of them stale, and they are also the slowest tests of the suite. The owner's ruling (plan, P10.3): tag these tests so that coverage attribution never reruns them for a change that does not touch what they cover, report how many the tag kept, and leave `--depth deep` rerunning everything.

## Options

**Where the tag lives.** (a) An annotation in the test's source (an attribute, a comment, a pytest marker): it travels with the test, but every language needs its own reader, a test file must be parsed before the staleness rule can run, and a tag on hundreds of tests is hundreds of edits. (b) A declaration in `cupel.toml`, per language, naming the tests by pattern and the files they are attributed to: one place, read with the rest of `[lang.<id>]`, the same in every language.

**How tests are named.** (a) The runner's own selection syntax (a nextest filterset, a pytest `-k` expression): exact for its runner, but Cupel would need a parser per runner to decide membership without running it. (b) Globs over the test IDs the envelope shows (ADR-075): `cupel-cli::*`, `tests/e2e/*`. One matcher for every language; an ID is a string.

**What a tagged test is attributed to.** (a) The crates the binary is built from, derived from the build (`cargo metadata`): automatic, but language-specific and wider than the owner asked. (b) The files the declaration names, as path globs relative to the repository root: the test's own files, its fixtures, and the entry-point crate it drives, as the author knows them. Its measured coverage still exists and still counts everywhere coverage counts; only the staleness rule reads the declaration.

**What the count means.** (a) Every tagged test kept. (b) Only the kept tagged tests that executed a changed function, so would have rerun without the tag: the tag's effect, which is what the owner wants to see.

## Decision

1. **The declaration** (options b). `[lang.<id>.attribution.<name>]` with `tests`, globs over the test IDs as the envelope shows them (relative to the repository root; `*` matches any run of characters, `/`, `::`, and spaces included), and `files`, path globs relative to the repository root with `[scope] exclude`'s rules (a pattern with a `/` is anchored at the root, one without matches a name at any depth, `**` crosses directories, a directory names everything below it). Both are non-empty lists; no other key is allowed; the name matches `[a-z][a-z0-9_-]*`. The table is checked as the file loads, like relations (ADR-097 §1). A test matching several attributions is attributed to the union of their files.
2. **The staleness rule.** For a test an attribution names, ADR-083 §3 and ADR-084 §7 keep only their configuration and ownership rules: it is stale when a whole-suite file changed (every test is), when it owns a changed, added, or removed file (ADR-083), or when a changed, added, or removed file under the root matches its attribution's `files`; a change it executed elsewhere does not make it stale. A kept tagged test keeps its recorded result and its per-test lines, moved through the diff (ADR-084 §7), as any kept test does. The rule applies wherever the per-test rule decides staleness: the suite joint's incremental run (ADR-149 §1), its estimate and dry run (§6), and the per-test producers' own refresh (ADR-083 §3), all below `--depth deep`; at `deep` no attribution applies and the joint runs the whole suite (ADR-149 §2).
3. **A change only tagged tests ran.** ADR-149 §2 runs the whole suite when a source file changed and no cached test is stale, since nothing recorded then says what the change affects. When the tests that executed the change are all kept by their attribution, the change is placed: they are rerun instead (stale, not counted as kept), so the run measures the changed file's lines and still finds a new test in it, and the suite is not run whole.
4. **The counts.** `SuiteReuse` (`TestResults.reuse`, `run.suite`) gains `tagged`: of `reused`, the tests their attribution kept though they executed a changed function. The text renderer's `suite` line adds `(N by attribution)` when it is not 0. `tests --impacted` lists such a test among the impacted with `kept` naming the attribution and why (`attribution binary: no change under its files`), and leaves it out of the commands.
5. **What does not change.** `tests.coverage` and every other consumer of `CoverageMap` and `PerTestMap` read the tests' measured lines as before. The mutation engine selects a mutant's tests by executed lines (ADR-080 §7, ADR-083 §6), not by attribution: a tagged test that covers a mutant's line is one of its covering tests and runs against it, and its ID is part of the mutant's reuse key.

## Consequences

- On a range below `deep`, a change outside every attribution's files reruns no tagged test; on Cupel, the binary-spawning tests of `cupel-cli` and `cupel-fixtures` rerun only for a change to those crates, a whole-suite file, or the tests' own files (measured in P10 Notes "P10.3 — measurement").
- A kept tagged test's result is from a tree on which the code it ran through the binary may since have changed: a behaviour change outside its files that would make it fail is found at `--depth deep`, on a tree scope, or by the dogfood's changed gate (which runs at `deep`), not by the inner loop. That is the owner's trade; the declaration says which tests take it.
- A kept tagged test's per-test lines in a changed function are its old lines moved through the diff: the mutation engine still selects it for that function's mutants, from coverage measured before the change.
- A declaration that names too few files keeps a test that should rerun; one that names too many reruns it as before. Nothing checks a declaration against the tests' coverage.
- Rule 3 reruns the tagged tests that ran a change no untagged test ran, so such a change costs those tests, not the suite.
- `TestResults` stays version 2: `tagged` reads as 0 from a fact written before it.
