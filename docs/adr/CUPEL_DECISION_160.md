# CUPEL_DECISION_160: A narrowed mutant runs its tests one at a time, fastest first, each under its own cap

**Date:** 2026-11-04 · **Status:** accepted (the owner's v0.2.5 ruling); amends [147](CUPEL_DECISION_147.md) §1 (what the cap bounds) and [080](CUPEL_DECISION_080.md) §10 (a timeout is not reused) · **Phase:** B.2

## Context

A mutant that per-test selection narrowed (ADR-083) ran its selected tests as one invocation of the runner, all at once, under one timeout: `timeout_factor` times the file's reference run, at most `max_mutant_timeout` (ADR-147, default 120 s). The nextest runner passes `--no-fail-fast`, and a run stopped at its timeout reports no results. So a mutant was killed only when every selected test finished inside the cap, however early one of them failed.

v0.2.4's changed deep dogfood showed the cost. Seven mutants of `cupel-core/src/schedule.rs` (`CheckRun::finish`, `skipped_entry`, `reported`) timed out. Nearly every test in the workspace runs those functions, so each mutant selected over a hundred tests, among them `cupel-cli`'s end-to-end tests of a minute or more. `cupel-core`'s scheduler tests kill five of the seven in milliseconds. Even with nextest's own fail-fast, the whole workspace had not reached them after 400 s, because nextest starts with the slow `cupel-cli` binaries. Rerun with a 40-minute cap on a cleared cache, five were killed and two survived (both since killed by a pinned test). A timeout there meant only "not decided in time", and it hid what the tests could tell.

The owner's v0.2.5 ruling:
- Order each mutant's selected covering tests ascending by last known duration from the fact store, unknown durations last.
- Apply the time cap per test, not per mutant.
- A test that fails kills the mutant.
- A test that exceeds its own cap while normally finishing well inside it (under a quarter of the cap) is a hang, and kills the mutant.
- A mutant whose selected tests cannot all complete within the overall cap for reasons of slowness, not hanging, is `mutant.timeout` with its outcome unknown:
  - left out of the score's denominator,
  - surfaced in `next_actions` naming the slowest tests,
  - never counted as killed.

## Options

**Ordering inside one invocation, or across invocations.**
- (a) Ask the runner to run the selection in the given order. nextest runs one process per test in its own order, with no flag to order them. Go and Jest order by package and file. Only `pytest` and `unittest` take an explicit order (`RunOrdering::Explicit`).
- (b) One invocation per test, in the engine's order.
  - Every runner with a per-test map (nextest, `go test`, `pytest`) runs one test by name, so this works for each.
  - The engine sees each test's outcome and time, and can stop at the first that decides the mutant.
  - The cost is the runner's start-up for every test: 0.7 to 1.2 s for a warm nextest run of one test in this workspace.
- Chosen: (b). A killed mutant stops at its first failing test, which is the fastest that fails, so killed mutants get much cheaper. A survivor pays one start-up per selected test, sequentially, where before its tests shared the slot's threads. The overall cap bounds that, and a survivor whose tests cannot all finish under it is reported as unknown, not survived.

**What "its own cap" is.**
- The cap a run already has, `timeout_factor` times the reference (floor 2 s), applies to each test with that test's own baseline duration as the reference.
- A run of one test also pays the runner's start-up. Each slot measures that start-up once per file: an unmutated run of the file's fastest selected test, after the reference or validation that showed the slot green. Each test's cap adds it.
- (a) Leave start-up out: a fast test's 2 s floor would be mostly start-up, and a loaded machine would read a slow start as a hang that kills. (b) Add the measured start-up. Chosen: (b). A false hang inflates the score, which is the error this decision must not make.

**What the overall cap is.**
- (a) A new setting.
- (b) `max_mutant_timeout`, as ADR-147 named it, "the longest a mutant's run may take", now over the mutant's whole sequence.
- Chosen: (b). It keeps the wall time a mutant may cost a run where ADR-147 put it, and CI's changed dogfood keeps its bound. The per-test caps are what decide a hang, and the overall cap is what decides slowness.

## Decision

1. **Which mutants.** A covered mutant that per-test selection narrowed (`TestSelection::Tests` from the per-test map) runs as a sequence. A mutant that runs the whole suite, or all of it but the slow tests (ADR-147 §3), runs as one invocation under one timeout, as before.
2. **The order.** Ascending by the test's duration in the baseline `TestResults` (the fact store's last known duration), unknown durations last, ties by ID.
3. **Start-up.** After a slot's reference run (file copies) or the file's validation (schemata), the slot runs the file's fastest selected test alone with no mutant and keeps its wall time as the start-up `S`. A run that is not green is the same error as a red reference.
4. **Each test's cap.** For a test with baseline duration `d`, its own cap is `max(2 s, timeout_factor × d)`, and its run's timeout is `S` plus that, or what is left of the overall cap when that is less. A test with no known duration gets what is left of the overall cap.
5. **The overall cap.** `max_mutant_timeout` (default 120 s) bounds the sequence's wall time, start-ups and builds included (ADR-147 §1 amended: it caps a mutant's whole run, and each test also has its own cap).
6. **Outcomes, test by test, stopping at the first that decides:**
   - A test fails: **killed**, with that test as the killing test.
   - A test's run reaches its own cap (not one cut by the overall cap), its duration is known, and that duration is under a quarter of its own cap: a **hang**. The mutant is **killed**, with that test as the killing test and named in `hung`.
   - A test's run reaches a timeout that is not such a hang: the overall cap cut it short, its duration is unknown, or it normally takes a quarter of its cap or more. That is **slowness**. The mutant is **`timeout`**, with its outcome unknown: it is not in the score's denominator (ADR-141, unchanged) and is never counted as killed. `slowest` names, slowest first, up to three of the selected tests that had not completed, with their baseline durations.
   - The first test of a file copy rebuilds what the mutant changed, so its timeout also allows the reference run's time. The reference rebuilt the same file, so reaching that timeout is judged like any other test's.
   - A run that ends incomplete without reaching its timeout is **unviable**, as before.
   - Every test passes: **survived**.
7. **The fact.** `Mutant` gains `hung` (the test whose hang killed it) and `slowest` (`[{id, baseline_ms}]`). Both are optional and absent from JSON when empty, so the fact's version stays the same. `tests` lists the tests that ran, so a killed mutant lists those up to and including its killing test.
8. **The finding.** `mutant.timeout` says its selected tests could not all run within `max_mutant_timeout` and names the slowest with their baseline seconds. Its action tells the reader to speed those up or raise the cap, and it carries the slowest in its evidence, so `next_actions` names them.
9. **Reuse** (ADR-080 §10 amended). A stored `timeout` is not reused: its outcome is unknown, and a rerun under another cap, another order, or a quieter machine may decide it. Every other stored outcome is reused as before.

## Consequences

- A mutant of a hot-path function with any fast killing test is killed in seconds, whatever slow tests it also selects.
  - The fixture `tests.mutation/python-ordering`: one function covered by a fast test and a slow one, both of which kill. Every mutant is killed by the fast test alone, in a fraction of the slow test's time.
  - Rerunning v0.2.4's seven `schedule.rs` timeouts under the default cap resolves them.
- A survivor costs one runner start-up per selected test, run one after another. A hot-path survivor with many slow tests reports `timeout` naming them rather than `survived`. That is honest: it was not decided within the cap.
- A mutant that makes a fast test loop is killed when that test passes its own cap, not when the mutant's whole cap is spent.
- A runner's start-up that grows mid-file (a cold cache, a loaded machine) can push a fast test past its cap. Measuring start-up per slot and per file, and allowing at least the 2 s floor beyond it, makes that rare. When it happens, the hang rule needs the test to normally take under a quarter of its cap, which a 10× factor always meets, so the default settings would count it as a hang. A `timeout_factor` under 4 turns every reached cap into slowness.
