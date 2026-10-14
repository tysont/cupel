# CUPEL_DECISION_056: Base-side facts for range scopes

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P2.7

## Context

`tests.weakening` (design §5.2, plan P2.7) diffs `TestFrames` between a range's base and head. P2.1 (ADR-040, ADR-041) resolves `--changed` and `--base/--head` to a list of files and produces every fact from the working-tree versions of those files: nothing in the pipeline can read a file as it was at the base commit, and a check is handed one `Facts` with one value per kind. The range's changed paths are also reduced to the source files that exist in the working tree, so a deleted test file and a rewritten snapshot file (not source code) are invisible to checks. Plan §2.8 forbids a check from touching git or the file system, so the base side must arrive as facts.

Six checks are being written concurrently against the P2.1 contract, and most construct `CheckCtx`, `Scope`, and `Manifest` literals in their tests; adding a field to any of those breaks every one of them at merge.

## Options

**Base facts produced from a checkout of the base files.** At scope resolution, write the base commit's version of every changed source file (`git show <base>:./<path>`) into the run's scratch directory, and run the ordinary language producers over that directory as a second root. Producers stay unaware of git and of the base; the parse is the same code as the head side, so the two sides are comparable by construction. Cost is one `git show` per changed source file plus a parse of the changed files only.

**Producers that read from git directly** (`ProduceCtx` gains a "read file at ref" hook). Avoids the scratch copy but makes every producer's file access go through a new abstraction, which touches every producer in `cupel-lang` and every future tool producer.

**Base facts from the fact store** (the last run at the base commit). Only works when someone ran Cupel at exactly the base commit with the same scope, which an agent's `--changed` loop rarely has.

**How a check declares that it reads the base.** A new `Manifest` field is the most visible, but every concurrent check builds a `Manifest` literal; a provided method on `Check` with an empty default is invisible to checks that do not use it.

**Where the base values live.** A `CheckCtx` field breaks concurrent `CheckCtx` literals; `Facts` is only ever built through `Facts::for_manifest`, so new methods on it are additive.

## Decision

1. **Declaring.** `Check::base_needs(&self) -> &[FactKind]` (provided, default empty) names the kinds a check also reads from the base side. They must also be in `needs`, so the support matrix (ADR-015) and the declared-facts rule (ADR-043) apply to them unchanged.
2. **Reading.** `Facts::base::<F>()` returns the base side's `F` (`UndeclaredFact` when `F` is not declared, `AbsentFact` when it was not produced). `Facts::range()` returns the `Range { commit, changes }` being compared: the base commit's SHA and every path the range changed, after `--paths`, with `ChangeKind::{Added, Modified, Deleted}` — including deleted files and files no language owns (snapshots, data). The scheduler fills both through `Facts::insert_base_shared` and `Facts::set_range`.
3. **Producing.** `git` gains `changes_since(rev)` and `changes_between(base, head)` (`git diff --name-status --no-renames -z`, untracked files as `Added`) and `show(rev, path)`; `changed_since`/`changed_between` are their path lists. `ResolvedFiles` gains `changes` (the range's changes after `--paths`; empty without a range). `BaseTree::checkout(range, &git, &writer, owned)` writes the base version of every changed path that is not `Added` and that `owned` accepts (a registered language owns it) to `.cupel/work/<run>/base/<path>` through the run's scratch writer, and returns `BaseTree { range, root, files }`. `BaseTree::planned(range, &writer)` is the same without reading or writing anything, for `--dry-run`. `Job` gains `base: Option<BaseTree>`.
4. **Scheduling.** With a `BaseTree`, the scheduler keeps a second fact store rooted at `BaseTree::root` over `BaseTree::files`, and produces each `base_needs` kind there at most once per language, exactly as on the head side. Base-side producer findings (a base file that does not parse) are dropped: they describe code the change already replaced. A check with `base_needs` on a scope without a range is skipped for every language in scope with `SkipReason::NeedsRange` (`needs-range`) and `see: cupel check --changed --only <id>`; its (check, language) pairs never become tasks.
5. **Languages.** The CLI counts a language as in scope when it owns a head file *or* a base file, so a change that only deletes a language's test file still runs the language's checks.
6. **Head side.** Unchanged: the working-tree versions of the changed files (ADR-040). `--base A --head B` therefore compares `A` with the working tree's files, not with `B`, unless the working tree is at `B`; making the head side a checkout of `B` is the same mechanism and is left to the step that first needs it.

## Consequences

- Any check can compare two sides of a range by adding `base_needs` and reading `facts.base`; no check and no producer touches git.
- A range run writes the changed source files' base versions under `.cupel/work/<run>/base/`; nothing prunes `.cupel/work/` yet (as for every scratch directory).
- `SkipReason` has a sixth value; its schema and the envelope contract test change with it.
- Producers that look beyond the files they are given (project manifests, `tsconfig.json`) see an empty root on the base side. No base-side consumer reads such a fact today; a check that needs `ProjectInfo` or `ModuleGraph` from the base would need the checkout to include those files.
