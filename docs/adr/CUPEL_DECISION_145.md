# CUPEL_DECISION_145: Cupel's own tests wait on events, not on the clock

**Date:** 2026-10-23 · **Status:** accepted · **Phase:** P9.F4

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
6. **No `make check-slow`.** Every timing-sensitive test was made deterministic within the hour, so no target exists.

### The tests, their causes, and the fix per test

| Test | Cause | Fix |
|---|---|---|
| `similarity::tests::ten_thousand_symbols_are_searched_in_linear_work` (was `…_and_under_a_second`) | 20 s debug budget; 22.5 s at load ~110 | work bound only (§1), plus every planted pair compared |
| `check_host::a_cancelled_run_sends_sigterm_then_sigkill_and_is_cut_short` | cancelled at 400 ms, before the plugin had set its trap (no marker file) | cancel once the plugin writes `ready` (§2) |
| `process.rs` `termination_sends_sigterm…`, `a_child_that_ignores_sigterm…` | the same trap race at 300 ms; 10 s ceilings | `ready` event; grace measured from the cancellation (§2, §3) |
| `process.rs` `cancellation_kills_the_child_and_its_process_group` | 10 s ceiling | sleeps and ceiling of `HANG` (§3) |
| `run_controls.rs` `remaining_is_the_time…` | `as_secs()` exact (59, 4) after an unbounded gap | bounds derived from the elapsed time measured in the test (§3) |
| `run_controls.rs` `wait_returns_early_…` (two) | 5 s ceilings | an hour's wait that must not complete, at least 20 ms elapsed (§3) |
| `runner.rs` `a_child_cancellation_ends_at_its_timeout…` | "not cancelled yet" read 20 ms after the child's start | read on a child of an hour (§3) |
| `budget_runs.rs` four budget tests | 300 ms/800 ms/5 s budgets beside real work | §4; the cut test's task lasts `HANG` |
| `cupel-plugin` and `cupel-cli` plugin tests (`plugins.rs` both named in the brief, `check_host`, `discovery`, `producer_host`) | first execution of a fresh script: 2–4 s at load ~100, inside the 10 s `describe`/`plan` limit | warm (§5) |
| `server.rs` streaming, concurrency, hang-up, stop, socket-removal, watcher; daemon start | 150 ms/1 200 ms/5 s timing, sleeps for readiness, a 10 s poll, a 60 s start | handler signs and `HANG` (§2, §3) |
| `cupel-cli` `serve.rs`, `mcp.rs`, `cupel-fixtures` parity daemon, `cupel-serve` `mcp.rs` | 15–30 s polls for a daemon that may start a watcher | `HANG`/`PATIENCE` (§3) |
| `dogfood_lock.rs` `a_second_dogfood_waits…` | a 300 ms sleep for "the first holds the lock", a 1 s hold for "the second waits" | log line and the second's own message (§2) |
| `exit_codes.rs` two tests, `serve.rs` unit test, `budget.rs` cut test | `--timeout 10s` on a whole `describe`; 20 s/30 s/25 s ceilings | `1h`; ceilings are the slow task's length (§3) |

Reviewed and left: the 20 ms sleeps that order modification times (`fresh.rs`, `coverage_producers.rs`, `overlay.rs`), which load only lengthens; `selection.rs`'s 1 ms timeout, which no run meets; `a_one_second_budget_skips_mutation_and_says_so`, written to accept either outcome per cheap check; the fuzz tests (a 300 s cap on 200 executions, a target that spins forever); `porcupine`'s 30 s search cap on a six-operation history; the Linux-only ETXTBSY test (`process.rs`), bounded by production's `BUSY_WAIT`; `cupel-adapt` `contracts.rs`, whose runs take no timeout and which passed every baseline and stress round here.

### Measurements

`tests.flaky` over Cupel's own suite with a config copy whose `[lang.rust]` pins `runner = "cargo-test"` (the brief's `"cargo"` names no producer, so the check was `unconfigured`) and whose `[scope] exclude` adds the non-Rust sources (`**/*.clj`, `*.py`, `*.go`, `*.js`, `*.mjs`, `*.rb`, `dist/**`, `plugins/**`; without it the Clojure file's unavailable runner skipped the check), `--depth standard`, run with the binary built at 937c723. Load: the one-minute average every 30 s, from ripgrep 14.1.1's `cargo test --all` in loops plus the machine's other work.

| Tree | Seed | Load (min / median / max) | Result |
|---|---|---|---|
| 937c723 | 1 | 40 / 109 / 220 | exit 4: baseline red on the similarity test (took 22.45 s) |
| 937c723 + the similarity fix | 1 | 54 / 107 / 217 | exit 4: baseline red on `a_cancelled_run_sends_sigterm_then_sigkill_and_is_cut_short` (no marker: SIGTERM before the trap) |
| 937c723 + the similarity fix | 2 | 17 / 103 / 240 | baseline green; 3 reruns: 0 `test.flaky`, 0 `test.order-dependent` |
| 937c723 + the similarity fix | 3 | 17 / 103 / 240 | baseline green; 3 reruns: 0, 0 |
| 0018202 (all fixes) | 1 | 17 / 90 / 240 | baseline green; 3 reruns: 0, 0 |
| 0018202 | 2 | 13 / 55 / 118 | baseline green; 3 reruns: 0, 0 |
| 0018202 | 3 | 13 / 58 / 118 | baseline green; 3 reruns: 0, 0 |

The engine stops at a red baseline (exit 4), so a test that flakes in the baseline hides every other: the before measurement saw the two above and, with them out of the way, nothing more in six reruns. The rest of the list came from review and from what a loaded macOS does to it: the first run of a just-written script took 2.1–4.0 s at a load average of ~100 against 0.01–0.14 s for its second, with `XprotectService`, `syspolicyd`, and `mds` among the busiest processes, so the load average understates how slow a process start can get. The after measurement ran at a lower median load for seeds 2 and 3 because the induced load was capped at 90 to leave the machine to other work. `make check` at 0018202 three times in a row: green, 2 679 passed each (load 21 / 80 / 166 over the three); at 2d6f1af (the dogfood's follow-ups) three times more: green, 2 679 each (load 35 / 65 / 108, 23 / 55 / 101, 18 / 61 / 99).

## Consequences

- A test that regresses into a hang waits up to ten minutes before it fails instead of a few seconds; a correct run is as fast as before.
- The plugin limit still applies to a warm plugin, whose second start took at most 0.14 s at a load average of ~100.
- New timing-sensitive tests follow the same patterns; review rejects a sleep standing in for readiness and an elapsed-time assertion shorter than a hang.
