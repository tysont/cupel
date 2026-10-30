# B.3 — The temporary-directory leak

Branch `b3-temp-leak` (worktree `.claude/worktrees/b3-temp-leak`), after B.2. Read `common.md`, the plan's `#### B.3`, P10 Notes "P10 — leaked test directories", and the test support modules (`crates/*/tests/support/`, `crates/cupel-fixtures/src/`). No ADR unless the product changes; an ADR number is reserved: **157**.

## The owner's ruling

Fix the harness so tests use scoped temporary directories that are removed on drop, including tests that spawn the binary. Add a test that counts temp directories before and after `make check` and fails on growth.

## What is known

One `make check` leaves about 140 `$TMPDIR/.tmp*` directories, and runs over days left 616 (32 GB). Mutation slots under `$TMPDIR/cupel-mutate/<repo hash>` outlive the runs that made them (ADR-141's gone-repository rule removes them only at the next claim).

## What to build

1. **Find every leak.** Diff `$TMPDIR` before and after `make test` with nothing else running, then map each leaked directory to the test that made it, by its contents or by running test binaries one at a time. Likely kinds: a `TempDir` turned into a path (`into_path`, `keep`), `std::mem::forget`, a directory made by the spawned binary or a tool it runs (pytest, cargo, the JVM), a child killed before its cleanup ran, and a test that panics before its guard drops (a `TempDir` drops on panic; a hand-made path does not).
2. **Fix each at its source.** Use scoped guards in the harness. A directory the binary or a tool makes goes under a test-owned `TMPDIR` (set `TMPDIR` for every spawned process to a directory the test's guard owns), so it goes when the guard drops. Remove the slots of a run's repository when the repository is a test's temporary copy.
3. **The test.** Make it a `make check` step (a script under `scripts/` that counts `$TMPDIR` entries, runs the rest, and counts again), or a test harness that spawns the suite. It fails on growth, allows for directories other processes on the machine make meanwhile (count only those whose names and contents are Cupel's tests'), and passes on a clean run.

## Leave alone

The product's behaviour, unless a leak is the product's own (the binary leaving a directory behind). In that case reserve ADR-157 and say so.
