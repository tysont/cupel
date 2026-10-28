# CUPEL_DECISION_150: The mutation engine's phase breakdown

**Date:** 2026-10-28 · **Status:** proposed · **Phase:** P10.2

## Context

On a one-function change in Cupel's own repository at `standard`, the mutation engine takes 285 s for 2 mutants and 412 s for 3 (plan, P4 Notes "P4 — Definition of Done, remeasured"); on a quiet machine, 311 s for 2: "141 s each in parallel slots, the remaining ~170 s the slots' sync with the tree, their schemata builds, and validation" (P3 Notes "P3.6 — measurement on a quiet machine"). Those remainders are inferred from wall clocks, not measured: the store's `durations` table (ADR-086 §2) holds one row per production, `engine.mutation`, and `MutationOutcomes` holds one `duration_ms` per mutant, the test run alone. Nothing records what the slots' sync costs, what the schemata validation costs (its compile and its test run together, ADR-082 §5), how many tests a mutant's selection narrowed to, or what planning and the store's history cost. P10.2's ruling (plan, P10) is to instrument one mutant end to end, confirm schemata and per-test selection on Cupel's own workspace, and fix what dominates; the breakdown is the evidence the rest of the step stands on, so it is decided first.

## Options

**Where the breakdown lives.** (a) Only in the store's `durations` table, as rows the scheduler records: the scheduler times productions as a whole and knows no producer's phases, so each producer would need a channel into the scheduler (`ProduceCtx`) for sub-durations, a mechanism for one producer. (b) In the fact: `MutationOutcomes` carries the phases the production went through, as it already carries each mutant's run time; the CLI, which already indexes the fact's mutants into the store (`record_mutants`, ADR-080 §10), records the phase sums beside the production's duration. The fact is what `--format json` consumers and the goldens see; the table is what `scripts/inner_loop.sh` and the budget's history read. (c) Per mutant only, as fields on `Mutant`: the sync and the validation are not a mutant's; they are a slot's or a file's, shared by every mutant of the file, so per-mutant fields would either repeat them or apportion them.

**How fine.** (a) Per phase totals: five numbers, no way to see that a validation ran in every slot or that one slot's sync was the slow one. (b) One row per phase occurrence, with the file and slot it served and the tests it ran: a timeline of the production, from which totals, maxima, and parallelism are read.

**What the check shows.** (a) Nothing: the fact is not in the envelope, so `cupel check --format json` and `cupel explain tests.mutation` would show no phase. (b) Metrics: `tests.mutation` reports each phase's total as `<phase>_ms`, so the envelope, `explain`, and the gate keys carry them; the goldens redact every `*_ms` metric as they redact `duration_ms`.

## Decision

1. **The fact.** `MutationOutcomes` gains `phases: Vec<PhaseDuration>` (optional, absent from JSON when empty, so the fact's version stays 2 as `excluded` left it, ADR-147 §4): one row per phase the production went through, in the order they ended:

   ```text
   PhaseDuration {
     phase: plan | sync | rewrite | build | run,
     file: Option<String>,      // the file whose mutants the phase served; null for plan and sync
     slot: Option<u32>,         // the slot it ran in; null for plan and rewrite
     tests: Option<u32>,        // the tests the phase ran, when it ran tests; null for the whole suite
     duration_ms: u64,
   }
   ```

   `plan` is the plan of the mutants in focus, their coverage gating and per-test selection, and the store's history (ADR-080 §5–7, §10–11), once per production. `sync` is one slot's reconciliation with the tree (ADR-080 §8), once per slot used. `rewrite` is one file's schemata rewrite (ADR-082 §2), once per attempt. `build` is the run every mutant of a file shares: the schemata validation (ADR-082 §5: the build and the file's selected tests with no mutant) or a file copy's reference run (ADR-080 §9: the rebuild and the same tests), once per slot the file used, with `tests` the file's selection (`null` when it is the whole suite). `run` is one mutant's test run, its `duration_ms` equal to the mutant's, so the phases alone are the production's timeline. A production that runs no mutant has no phases. A wrapped engine (PIT, ADR-110) records none.
2. **The store.** When a run is recorded, each language's `MutationOutcomes` adds one `durations` row per phase present, kind `producer`, task `<engine>.<phase>` (`engine.mutation.build`), class `run`, with the phase's total over its rows; beside the production's own `engine.mutation` row the scheduler records. The budget's cost model (ADR-086) reads shapes by name and never asks for these.
3. **The check.** `tests.mutation` reports `plan_ms`, `sync_ms`, `rewrite_ms`, `build_ms`, and `run_ms`, each the total of that phase over the outcomes in scope; 0 when the phase did not occur. The goldens redact every metric named `*_ms` as `<duration-ms>`, so a fixture's run time never fixes a golden.
4. **The engine** records the phases as it goes (`Phases`, a recorder the production's workers share), and nothing else about how it runs changes under this section. What the breakdown showed on Cupel's own workspace, and what changed because of it, follows in the sections below.

## Consequences

- The timeline of a mutation production is in its fact and its totals in the store, so `scripts/inner_loop.sh`'s durations query lists `engine.mutation.sync`, `.build`, and `.run` beside the suite run and the engine, and a measurement names the phase that dominates instead of inferring it.
- A deep run with thousands of mutants carries thousands of `run` rows that repeat each mutant's `duration_ms`; accepted for a uniform table (a row is five small fields).
- Five metrics join `tests.mutation`'s `describe` output and the README's generated check table; nothing gates on them.
