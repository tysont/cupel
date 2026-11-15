# CUPEL_DECISION_219: A test that fails with no mutant applied is left out of every mutant's run

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 4 · Details [185](CUPEL_DECISION_185.md); the analogue of [147](CUPEL_DECISION_147.md) §3–4

## Context
Before it runs a file's mutants as file copies, the mutation engine runs the whole suite once in the first slot with no mutant applied. This is the baseline that times every copy (ADR-185). When that run is red, the slot does not reproduce the tree, and the engine errors: `tests.mutation` ends `error` with "the unmutated tree does not pass its tests in the mutation overlay at …".

In the v0.2.9 validation on encode/httpx, `tests.mutation` errored that way: `tests/test_timeouts.py::test_write_timeout[trio]` failed in the overlay, although the baseline test run and `tests.flaky`'s reruns passed it. It is a timing test, and it failed under the parallel slots. The survey's quick "pass" (323 s) was mutation erroring early, and with the test deselected by hand, mutation ran for 29 minutes (NOTES Q-py-4).

The owner's ruling (Q-py-4): "a test failing with no mutation applied is excluded from mutant runs with mutant.flaky-test-excluded at info."

Open: which unmutated runs detect such a test, how it is left out, and what the stored baseline and mutant outcomes hold afterward.

## Options
1. Where to detect. (a) A separate unmutated run of the whole suite before any mutant: it costs a run even when the stored baseline would have served. (b) The baseline that times the file copies (ADR-185), already a whole-suite run with no mutant in a slot, which is where httpx failed. A red schemata validation (Rust) already falls back to file copies, whose baseline then detects it.
2. How to leave it out. (a) As ADR-147 leaves out slow tests: `TestSelection::Except`, which a runner turns into its deselect flag (`pytest --deselect`, nextest `-E 'not (…)'`, `go test -skip`). A runner without one cannot leave a test out, and the engine errors as before. (b) Run the other tests by name: only runners that name tests, and a command line as long as the suite.
3. Which tests are left out. (a) Each test the red baseline failed, when the run otherwise reached its end; a build failure or a crashed invocation stays an error. (b) Also retry the run until it is green: a suite that fails differently every run would be cut down to nothing.

Chosen: 1(b), 2(a), 3(a) with one retry.

## Decision
1. **Detection.** When the file copies' baseline run in the first slot is red, every invocation reached its end, and the runner can leave tests out, the engine runs the baseline again without the tests that failed. If that run is green, those tests are the production's *flaky* tests, and the second run's time is the baseline. If it is still red, or the first run did not reach its end, or the runner cannot deselect, the engine errors as before.
2. **Exclusion.** A flaky test is left out of every mutant's run for the rest of the production. A whole-suite run gets the slow tests and the flaky ones in its `Except`. A per-test-narrowed run drops them from its tests, and a mutant with no narrowed test left runs the whole suite instead. The start-up probe (ADR-160) and the schemata validation of later files select the same way.
3. **The fact and the finding.** `MutationOutcomes` gains `flaky: [{id, file}]`, sorted by ID (absent from JSON when empty; an optional field, so the fact stays version 3). `tests.mutation` emits one `mutant.flaky-test-excluded` per entry: `info`, action `review`, evidence `{test, file}`, title "Test left out of mutant runs: `<id>` failed in the mutation overlay with no mutant applied". The score is over the mutants as before, decided by the tests that ran.
4. **The stored baseline.** A baseline that needed flaky tests left out is not stored (ADR-185's store keeps only a green first run). The next production measures again and finds which tests fail then, so a test that becomes reliable rejoins the mutants' runs.

## Consequences
- httpx's mutation runs instead of erroring, without `test_write_timeout[trio]`, with one info finding naming it. A test that fails only in the overlay because it depends on its directory, environment, or parallel neighbours is named, and the other tests still kill mutants.
- A flaky test is detected only by a measured baseline. When a stored baseline time is read (ADR-185), no unmutated run happens, and a test that fails in every mutant's run kills each of them, as before this ruling. A stored baseline exists only when an earlier production's run was green.
- The suite hash in a mutant's cache key is unchanged by a flaky exclusion: it is known only after the plan reads the store. Outcomes stored from a run without the flaky test are reused under the same key.
- A runner that cannot deselect (`cargo test`, `unittest`, `jest`, `vitest`, Surefire, `clojure.test`; see ADR-147 §5) still errors on a red baseline.
- Implemented in `cupel-mutate/src/execute.rs` (`reference`, `flaky_in`, `whole`, `narrowed`), `baseline.rs` (`Baseline::flaky`), `engine.rs`, and `cupel-checks/src/tests_mutation.rs`. Fixture: `crates/cupel-fixtures/repos/mutant-flaky/python`, a test that fails wherever `CUPEL_MUTANT` is set, with `crates/cupel-fixtures/tests/mutant_flaky.rs`.
