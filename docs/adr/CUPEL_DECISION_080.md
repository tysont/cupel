# CUPEL_DECISION_080: `MutationOutcomes` and the file-copy mutation engine

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P3.4

## Context

P3.4 makes `MutationOutcomes` real: `cupel-mutate` is the engine that produces it (design §8.7: `Producer<MutationOutcomes>`, `needs = [MutationSites, CoverageMap, TestResults]`, `wants = [PerTestMap, ModuleGraph]`, a `Runner` from the registry) and `tests.mutation` is the check that scores it (§5.2, §8.5). The design names the performance stack (§9.3: coverage gating, store-based differential reruns, per-test selection, worker overlays, timeouts, ordering) and the cache key (§8.8), but leaves open: the payload's fields; how a producer gets a runner, its wants, the run's depth, and the changed lines (the producer contract has none of them); where overlays live and how they stay correct under incremental builds; what a timeout is measured against when runners report no per-test durations and compiled languages rebuild per mutant; and what the differential key's "symbol hash" is, given that the structural hash (ADR-025) ignores literals and local names.

## Options

**Overlays.** (a) A fresh copy of the tree per mutant: simple, but a compiled language rebuilds everything per mutant. (b) Symlink the tree and copy the mutated file (mutator's approach): cheap, but tools that resolve symlinks (Node, Python's `__file__`) reach the original tree, so importers must be copied too, and any tool writing in place writes the user's files. (c) Persistent worker trees ("slots") outside the repository, each a mirror of the tree that is reconciled at the start of a production, the mutated file written privately and restored afterwards: one build per slot, then incremental builds. Inside `.cupel/` a slot would be discovered by runners that do not skip hidden directories (vitest's default `include` does not), so the user's own test runs would run the slots' copies.

**Timeouts.** (a) `timeout_factor × Σ baseline durations` of the selected tests, as §9.3 says: `cargo test` and `unittest` report no durations, and for a compiled language the run includes the rebuild the mutant causes, which no baseline duration contains, so mutants would time out for building. (b) Measure a reference: before a slot's first mutant of a file, run the unmutated file's tests in the slot after rewriting the file with a fresh modification time, which costs exactly the rebuild a mutant of that file causes, and time it.

**The differential key's symbol component.** (a) The structural hash: literals and locals are markers in it, so changing `x < 10` to `x < 20` would reuse the old outcome. (b) A hash of the symbol's source text: any edit to the function reruns its mutants; reformatting does too, which costs time but never correctness.

## Decision

1. **Payload** `cupel_core::MutationOutcomes`, version 1 (`mutation.rs`):

   ```text
   MutationOutcomes {
     engine: ProducerId,                 // engine.mutation, or a wrapped tool's producer
     runner: Option<ProducerId>,         // the runner that ran the mutants; null when none ran
     depth: CostClass,                   // the depth the engine planned for
     per_test: bool,                     // tests were selected by a PerTestMap
     mutants: [Mutant {                  // sorted by file, line, column, replacement
       symbol: SymbolKey, symbol_hash: Option<SymbolHash>,   // structural (ADR-025), for findings
       site_hash: SiteHash, operator: MutationOperator,
       from: String, to: String, span: Span,
       outcome: killed | survived | timeout | uncovered | unviable,
       tests: [String],                  // the tests run against it (none when uncovered)
       killing_tests: [String],          // those that failed or errored
       duration_ms: Option<u64>,         // its run's wall time; null when not run
       reused: bool,                     // the outcome came from the store
       equivalent: Option<String>,       // why it may be equivalent (ADR-004)
     }],
   }
   ```

   One mutant per (site, replacement). `unviable`: the run did not complete and no test failed (it did not build or load); it is neither killed nor survived. Version rule: bump on a renamed, removed, or retyped field, an outcome added or redefined, or a change to what `tests` holds. `FactKind::MutationOutcomes` was already in the catalog; this ADR gives it its payload, its schema (`FACT_SCHEMAS`), and its producer.
2. **What producers get** (`ProduceCtx`, `Producer`). `Producer::wants` (default none): the scheduler makes each want it can, after the needs, and hands it upstream; a want that fails is left out. `ProduceCtx::runners`: the registered runners (`ProducerRegistry::runners`), so engines call the runner a baseline names. `ProduceCtx::focus`: the run's depth, the symbols `--symbols` selected (per language), and, with a range, the lines it changed per file (`git diff -U0`, as `tests --impacted` reads them). `Job` carries the depth and the changed lines.
3. **`Runner::run(ctx, selection, &RunOptions) -> TestRun`.** `RunOptions { env, timeout, order, seed, parallelism }` (default: the baseline's run) replaces `(env, timeout)`; each runner turns order, seed, and parallelism into its own arguments and environment (`RunnerSpec::vary`), and `TestRun` adds the order its tests ran in where the runner reports it. `Runner::ordering()` says whether it runs a given order (`pytest`, `unittest`), shuffles by seed, or neither (`cargo nextest`). The flake engine (ADR-078) now calls the registered runner through it; its private per-runner table is gone.
4. **The engine** `engine.mutation` (`crates/cupel-mutate`): `needs = [MutationSites, CoverageMap, TestResults]`; `wants = [PerTestMap, ModuleGraph, NormTree, ProjectInfo]` — `NormTree` for each symbol's structural hash, `ProjectInfo` for the runner's project roots. With no baseline upstream it runs nothing and produces no mutants. It depends on `cupel-core` and `cupel-store` (to read past outcomes); nothing depends on it but `cupel-cli` and `cupel-fixtures`, which register it.
5. **Plan** (depth mapping, applied by `plan`): only sites of the `default` operator set unless `[lang.<id>] mutation = { operators = "extended" }`; only symbols in scope (and the `--symbols` selection); at `standard` with a range, only symbols whose span holds a changed line; at `deep` and `exhaustive`, every symbol in the scope's files (the scope is the touched files with `--changed`, the tree without).
6. **Coverage gating.** A mutant whose site has no line the `CoverageMap` records as hit is `uncovered` and never run. A line the map does not instrument counts as uncovered.
7. **Per-test selection.** With a `PerTestMap` whose runner is registered and ready, the tests run against a mutant are those covering its site's lines, run by that runner (for Rust, nextest IDs). Otherwise the baseline's runner runs its whole suite.
8. **Overlays.** One slot per worker under the system temporary directory, `cupel-mutate/<blake3 of the repository path>/<lang>/<n>/`, kept between runs. At the start of a production each slot is reconciled with the tree: every regular file the walk keeps (`.gitignore`s honoured, `.git` and `.cupel` left out) is placed — a clone (`std::fs::copy`, which is `clonefile` on APFS) on macOS, a hard link elsewhere, a copy when linking fails — unless the slot already holds the same bytes; files gone from the tree are removed; ignored directories (`target`, `node_modules`, virtual environments) are symlinks to the originals, so dependencies and build caches are shared. A mutant replaces its file, and every file `ModuleGraph` marks as a relative importer of it, with private copies (the link is removed first, so the original is never written) whose modification time is now; afterwards the original bytes are written back the same way, so incremental builds always see the change. Mutants of one file run concurrently, one per slot; files run one after another. `[lang.<id>] mutation = { jobs = N }` sets the slot count (default: half the cores, at most 4).
9. **Timeouts.** Before a slot's first mutant of a file, the engine runs the tests the file's mutants will run, unmutated, after rewriting the file fresh, and times it: the reference. Each mutant's run gets `max(2 s, timeout_factor × reference)` (`[lang.<id>] mutation = { timeout_factor = F }`, default 10); a run that hits it is `timeout`. A reference that is not green means the overlay does not reproduce the baseline, and the production fails rather than report breakage as kills.
10. **Differential reruns.** A mutant's outcome is reused when the store holds one under the same key: `(source_hash, site_hash, replacement, suite_hash, toolchain_hash)`. `source_hash` is BLAKE3 of the symbol's source text; `suite_hash` of the sorted IDs of the tests the mutant would run and the content of their files (the baseline's suite hash without a per-test map); `toolchain_hash` of the engine's version, the runner, and the runner's tool version. Rows live in the `mutants` table (store migration 3), indexed from the fact when the run is recorded, never written by the engine.
11. **Ordering.** Mutants to run are ordered: those that survived in the last stored run of the same site and replacement, then those in symbols with no stored outcome at all (changed or new code), then the rest; files take the position of their first mutant.

## Consequences

- `tests.mutation` scores whatever produced the fact; a PIT or Stryker wrapper is another producer of this payload.
- A slot's first production builds the project once in the slot (for Rust, into the shared `target/` under the slot's path); later productions build incrementally. Slots in the temporary directory can be removed at any time; they are rebuilt.
- The reference run costs one extra test run per slot per file, and makes mutants of a broken overlay an error instead of a false score.
- The source-text key reruns mutants of a reformatted function; changes outside the function (a callee, a shared constant) reuse outcomes, as design §8.8 accepts.
- Schemata (design §9.3 item 4, P4.1) replace the per-mutant rebuild for compiled languages; until then a Rust mutant costs an incremental build of its crate and its dependents.
