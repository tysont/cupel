# CUPEL_DECISION_145: Cupel's own tests wait on events, not on the clock

**Date:** 2026-10-23 · **Status:** proposed · **Phase:** P9.F4

## Context

Three merges in P9 waited an hour on a rerun because a test of Cupel's own suite failed under load and passed alone (plan, P9 Notes: P9.8, P9.F1 dogfoods). Every such test held a wall-clock constant that measured the machine as much as the code: a performance budget (`similarity`'s ten-thousand-symbol search, 20 s in debug, took 47–50 s at a load average of 60–170), a sleep that stood in for "the other side is ready" (a child's signal trap, a daemon's run, a second dogfood), an elapsed-time ceiling of a few seconds on something that should only not hang, or a production limit (the plugin host's 10 s `describe`) spent on something the test did not mean to time. A constant that passes today fails on the next slower machine, and a loosened one is the same constant.

## Options

**Per test, in order of preference.** (a) An injected clock: the code under test takes the time it reads, and the test hands it one it controls. Cupel has no clock abstraction; the scheduler's budget, `Cancellation`, and `Process` read `Instant::now()` themselves, and their tests are about real deadlines cutting real work, so a fake clock would test the fake. (b) Event-based waits: the test waits for the observable it means (a file the child writes once its trap is set, a counter of runs begun, a line on a pipe, a client having received a line), under a ceiling long enough that only a hang reaches it, never a sleep whose length is the assertion. (c) A bound derived from a baseline measured in the same process (run the small case, allow a multiple). (d) Work bounds instead of time bounds for performance tests: count operations, not milliseconds. Retries, `#[ignore]`, and loosened constants are not options.

**What cannot be made deterministic.** (e) A `make check-slow` target outside the merge gate for tests that need hours of work to fix; (f) none, if every test is fixed.

## Decision

1. **Performance tests bound work.** `similarity`'s test keeps every planted pair found and its exact comparisons at most twice the symbol count, and loses its wall-clock budget.
2. **Readiness is an event.** A test that must act once the other side is ready waits for that side's own sign: a child's trap test cancels once the child has written `ready` after setting its trap (`process.rs`, `check_host.rs`); the daemon's scripted handler counts runs begun, and the hang-up and stop tests wait on that count; the streaming test's handler goes on past its first line only once the client holds it; the concurrency test's runs answer only once all four are in flight together; the dogfood lock test releases the first run once the second has said it waits.
3. **Ceilings are hang guards.** Where a test asserts a run ended early, the ceiling is the length of what it cut (a slow task, a sleeping child, a suite), and that length is ten minutes (`HANG`) or an hour: longer than any stall of a loaded machine. Polls for a daemon or watcher to answer wait up to `HANG`. Lower bounds stay where they are deterministic (a run cut by a budget lasts at least the budget; a child that ignores SIGTERM is killed no sooner than the grace period after the cancellation).
4. **Budget scheduling tests leave the clock all the room their meaning allows.** A budget test asserts what fits and what does not by estimates; the budget is set as far from the instant work's actual time as the estimates permit (30 s below the costly check's 30.2 s default; an hour, with an estimate 400 ms short of it, for the overrun case), so only a hang changes the outcome.
5. **A fresh executable is run once before a time limit applies to it.** macOS checks a file's first execution: a just-written `sh` plugin took 2–4 s to start at a load average of ~100 (0.01 s on its second run), which spends the plugin host's 10 s `describe` limit (ADR-088 §4) on the operating system, not the plugin. The plugin tests' support writes each plugin and runs it once with an argument no plugin answers (`warm`); the limit itself is production behaviour and stays.
6. **`make check-slow`.** Recorded with the measurements below.

## Consequences

- A test that regresses into a hang waits up to ten minutes before it fails instead of a few seconds; a correct run is as fast as before.
- The plugin limit still applies to a warm plugin, which starts in well under a second at a load average of 150.
- New timing-sensitive tests follow the same patterns; review rejects a sleep standing in for readiness and an elapsed-time assertion shorter than a hang.
