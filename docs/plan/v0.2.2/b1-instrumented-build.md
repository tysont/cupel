# B.1 — The instrumented build's cost

Branch `b1-instrumented-build` (worktree `.claude/worktrees/b1-instrumented-build`). Read `common.md` first, then the plan's `#### B.1`, ADR-083 §1, ADR-084 §4 and §6, ADR-149, ADR-151, and P10 Notes "P10 — the final measurement". ADR number: **155**.

## The owner's ruling

Give coverage-instrumented builds their own target directory (`.cupel/target/cov` or equivalent) with incremental compilation enabled, so a one-function change rebuilds proportionally instead of near-fully. Keep it warm from the daemon when one is running. Remeasure the one-function `standard` run; target under 3 minutes warm. Record before/after in the ADR.

## What is known

- The warm run on `main` is 342 s, and 270 s of it is the suite run (`tool.llvm-cov-nextest`), which reran 18 of 2 818 tests. P10.3 and P10.2 both report that most of those 270 s is the instrumented rebuild, the profile export, and the report, not the tests. Nobody has measured the split.
- The instrumented build already has a target directory of its own, `target/cupel-llvm-cov` (`CARGO_LLVM_COV_TARGET_DIR`, ADR-084 §4), kept between runs by `--no-report`. What is not known is whether it compiles incrementally: `cargo llvm-cov` and Cupel's environment may set `CARGO_INCREMENTAL=0`, and CI sets `CARGO_PROFILE_DEV_DEBUG=0`. Find out first; the owner's "equivalent" covers keeping the existing directory if it is the right one. A move under `.cupel/` must keep `make reclaim`, the disk floor, and ADR-084's "a clean of `target/llvm-cov-target` does not discard it" true.

## What to build

1. **Measure the split before changing anything.** On the P10 change, time the warm suite run's phases: the instrumented build, the test runs, the per-test profile merge and export (ADR-083 §1), and the coverage report (`cargo llvm-cov report --lcov`). Record them in the store's `durations` as `tool.llvm-cov-nextest.<phase>`, as P10.2 did for the engine (ADR-150). The breakdown is the evidence for everything after it.
2. **Fix what dominates, one change at a time, measured.** Candidates, to be confirmed by the numbers: incremental compilation for the instrumented build; exporting profiles only for the tests that ran (the kept tests' coverage is reused, ADR-149); a report over the changed files only when nothing else needs the whole map; nextest building only the stale tests' binaries.
3. **The daemon.** When `cupel serve` runs (ADR-089), it keeps the instrumented build warm after a change it watches, so the next check finds it built. Put this behind the daemon's existing idle and budget rules, and make sure it never runs while a heavy command holds the lock.
4. **ADR-155** before the code that changes behaviour, with the before/after table.
5. **Measure** with `make inner-loop` before (main as you found it) and after. The target is under 180 s warm. Report honestly if it is not reached, with what remains.

## Leave alone

`crates/cupel-mutate/**` (ADR-150's engine), the gate rules, `[lang.rust] attribution` (ADR-151). B.2 changes where agent builds put their target directory; coordinate through the integrator if your change touches `CARGO_TARGET_DIR`.
