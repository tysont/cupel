# CUPEL_DECISION_217: Budgets, timeouts, and run durations count awake time, by one named clock

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 2 · Details [086](CUPEL_DECISION_086.md) §6 and [020](CUPEL_DECISION_020.md) §5

## Context
The v0.2.9 validation's cold `survey --budget 30m` on hashicorp/raft took 4 142 s of wall clock (15:52:15 to 17:01:17 in the harness log). Its envelope's `duration_ms` was 1 800 358 and its checks' durations add up to about 1 660 s. That left about 2 340 s outside both the budget and the reported duration (NOTES V4). The validation later withdrew V4 as a Cupel defect: the Mac had been asleep for that gap (lid closed, on battery), and the harness has held `caffeinate -ims` since.

The owner's ruling (Q-all-2): "budgets use monotonic time excluding sleep."

What the code does, read before changing anything:

- Every clock that bounds or times a run is `std::time::Instant`. That covers the run's start and `duration_ms` (`cupel-cli/src/run.rs`, `Run::clock`), the budget's deadline (`Run::deadline_after`, from which `cupel_core::budget::Budget` is built), the time left that the knapsack and `over_budget` read (`Budget::left_ms`, `schedule/budgeted.rs`), and `Cancellation`'s deadlines and polling (`cancel.rs`). `Cancellation` carries `--timeout`, the budget's cut of running work, and a runner's per-call timeout. Process timeouts (`process.rs`) and the mutation engine's per-mutant timeouts measure with `Instant` too. Nothing in the workspace computes a duration from `SystemTime` or from wall-clock timestamps. `SystemTime` is read only for `run.started` (RFC 3339 text), file modification times, and the Windows transport's socket name.
- The pinned toolchain is Rust 1.92.0. Its `Instant::now` (`library/std/src/sys/pal/unix/time.rs`) reads `clock_gettime(CLOCK_UPTIME_RAW)` on Apple targets ("does not increment while the system is asleep", the same value as `mach_absolute_time`) and `clock_gettime(CLOCK_MONOTONIC)` elsewhere on Unix, which on Linux does not count suspend (`CLOCK_BOOTTIME` does). std's documentation does not promise either: "it is also not specified whether system suspends count as elapsed time or not. The behavior varies across platforms and Rust versions."
- The raft evidence matches that. The budget cut work after 1 800 s of awake time, and `duration_ms` reported the same 1 800 s, while the wall clock ran on through the sleep. Neither the budget nor the duration counted the 2 340 s.

The pin test below shows the difference on the development Mac it was written on: there `CLOCK_MONOTONIC` read 1 063 775 s while `Instant` read 879 920 s, so the Mac had slept about 51 hours since it booted, and `Instant` had not counted any of it.

So nothing in V4 counted sleep. The ruling is what Cupel already does, but only because of a std implementation detail that std says may change.

## Options
(a) Leave it to std, and document the behavior. A toolchain bump could silently start counting sleep (as `CLOCK_MONOTONIC` on macOS, or `CLOCK_BOOTTIME` on Linux, would), and nothing would notice. (b) Replace `Instant` with Cupel's own clock type over `clock_gettime(CLOCK_UPTIME_RAW | CLOCK_MONOTONIC)`. That pins the source without depending on std, but every deadline, `Cancellation`, and `Budget` would change type, for no change in behavior today. (c) Name the clock in one place, `cupel_core::clock::now`, which returns std's `Instant`, and pin with a test that its value is the sleep-excluding clock's on each Unix platform. A toolchain that changes the source fails the test instead of changing the budget.

Chosen: (c).

## Decision
1. **The clock.** `cupel_core::clock::now()` is the clock every budget, timeout, and run duration is read from: the run's start and `duration_ms`, the budget's deadline and time left, the scheduler's task timings, and every `Cancellation` deadline. It counts the time the machine is awake and stops while it sleeps or is suspended: `CLOCK_UPTIME_RAW` on macOS, `CLOCK_MONOTONIC` on Linux.
2. **The pin.** A test in `cupel-core/src/clock.rs` (`the_clock_stops_while_the_machine_sleeps`, Unix only) reads `clock::now()` between two reads of that platform's sleep-excluding clock and checks that its seconds lie between them. On a machine that has slept since boot, `CLOCK_MONOTONIC` on macOS and `CLOCK_BOOTTIME` on Linux are ahead of the sleep-excluding clock by the time slept, so a std that switched to either would fail the test there. On a machine that never slept, the test still pins the clock's value domain. It reads the seconds from `Instant`'s `Debug` output (`tv_sec`), which std does not promise either; a change there also fails the test, which is the point: a toolchain bump must look at the clock again.
3. **What is not pinned.** On Windows, std's `Instant` reads `QueryPerformanceCounter`; this ADR does not establish whether that counts sleep, and the test does not run there. The tools Cupel runs keep their own clocks: `go test -timeout`, a test framework's per-test timeouts, and so on.
4. **Documentation.** The README's `--budget` text says the budget counts awake time.

## Consequences
- A run whose machine sleeps ends later on the wall clock than its budget says. That is the ruling: awake time is the work time, and `duration_ms` reports the same time the budget counted.
- No behavior changes on macOS or Linux at Rust 1.92. A toolchain bump that changes `Instant`'s source fails `make check` on the platform it changed.
- `clock::now` is read by the run's clock (`cupel-cli/src/run.rs`), `Cancellation` (`cancel.rs`), the scheduler and its budget (`schedule.rs`, `schedule/budgeted.rs`), and process timeouts (`process.rs`). The remaining `Instant::now` calls read the same clock today: the mutation engine's per-mutant timers, the daemon's idle and watch timers, and the store's lock retries. They are not routed through `clock` because no budget reads them, and the pin covers them as well, since they use the same std `Instant`.
