# CUPEL_DECISION_172: A mutation run cut short keeps the mutants it finished

**Date:** 2026-11-10 · **Status:** accepted · **Phase:** v0.2.6, Molten field trial Area 7 · Completes [086](CUPEL_DECISION_086.md) §9

## Context

ADR-086 §9 left an open item: the mutation engine makes its fact only when it finishes, so a mutation run cut by `--budget` reports `partial` with no score. Its Consequences add that the run's completed mutants are not stored either. Closing it needs an outcome for "not run" and a new payload version.

Area 7 ran `tests.mutation` under `--budget 30m` on a 10 726-mutant TypeScript package. The engine ran hundreds of mutants. When the budget's cancellation fired, `Running::run` returned `Err(Error::Cancelled)` from the checkpoint at the top of its per-file loop, and every outcome already written into the plan was dropped:
- nothing went into the `mutants` table, so the next run started from zero;
- the check reported `partial` with no score and no findings.

Thirty minutes of mutants produced nothing an agent could act on or a later run could reuse.

## Decision

1. **The outcome.** `MutantOutcome::NotRun` (`not-run`): a covered mutant the run's cancellation (`--budget` or `--timeout`) left without an outcome. It has no tests, no duration, and no strategy, and the score leaves it out as it leaves out `timeout`. `MutationOutcomes` goes to version 3. Adding an enum value is additive (ADR-148 kept version 2 for `out-of-memory`), but a version-3 fact can now be partial, which a reader of version 2 would not expect.
2. **The engine returns what it finished.**
   - Every covered mutant the store's history does not reuse starts as `not-run` and takes its real outcome when its run ends.
   - At the cancellation the engine stops taking files and returns the fact.
   - Inside the file being run, a slot that hits the cancellation stops, and the others finish or stop at their next run. `Execution::run_file` returns each mutant's outcome or `None`, keeping what every slot finished; an error other than the cancellation still ends the production.
   - A mutant whose run the cancellation reached (for a mutant whose tests run one at a time, ADR-160, any of those runs) is `not-run` whatever the runner reported, since a killed run's results are not the mutant's.
   - A file cut short has its original written back into every slot it used (`Execution::restore`), as a file whose mutants ended in an error does.
   - The fact has no separate "cut short" field: the `not-run` mutants say it, and say how many.
3. **The store.** `Store::record_mutants` stores every mutant but a `not-run` one. The next run reuses the stored outcomes by cache key (`History::reuse`, ADR-080 §10) and runs the rest, so a run cut by the budget is continued by the next one rather than repeated. Last run's survivors still run first (ADR-080 §11). A site's last stored outcome is not overwritten by a `not-run`.
4. **The check.** `tests.mutation` scores the mutants that ran (killed, survived, uncovered; reused ones included) and reports its usual findings about them. A `not-run` mutant is no finding of its own: a cut can leave thousands. A metric `not_run` counts them. When any is in scope, the result is `partial`, whatever its survivors, with a note first among its notes:

   > N of M mutants were not run: the run was cut short by --budget or --timeout. The score and findings are over the mutants that ran; the next run reuses their outcomes and runs the rest.

   `partial` keeps ADR-086 §7: the scheduler marks the run interrupted from a `partial` check, so it exits 5. The difference is that the result now carries a score, metrics, and findings. `executed` counts only mutants that ran.
5. **Timing.** A production that returns while the run's cancellation is set is not timed (`FactStore::timed`). ADR-086 §2 times only productions that finished, and a cut production's time says nothing of what the whole costs. The phase rows of its fact (ADR-150 §2) are still recorded: they are time the phases took, and the budget's cost model never reads them.

## Consequences

- A budgeted mutation run reports a score and survivors over the mutants it reached, and the next run continues where it stopped. A run of the Molten package can finish across several `--budget 30m` runs instead of never.
- The score of a cut run is over the mutants the engine's order reached: last run's survivors first, then mutants of symbols never run (ADR-080 §11). That sample leans toward survivors, so the score tends to understate the whole. The note and the `partial` status say it is incomplete, and `tests.mutation.score` gates on it like any score.
- A mutant run cut by the cancellation is lost even when it would have finished a moment later. At most one mutant per slot is lost per cut.
- Goldens and the `mutation-outcomes` schema snapshot change by the `not_run` metric, the `not-run` value, and the version.
- A producer plugin reads and writes a fact at one version (ADR-110), so a plugin that provides `mutation-outcomes` must declare version 3. Cupel's PIT plugin does; its payload is otherwise unchanged, as PIT's report has no mutant a cut left.
