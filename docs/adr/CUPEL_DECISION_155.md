# CUPEL_DECISION_155: The instrumented suite run's phase breakdown and its cost

**Date:** 2026-10-30 · **Status:** proposed · **Phase:** B.1

## Context

On Cupel's own repository the warm one-function `standard` run (`make inner-loop`, P10 Notes "P10 — the final measurement") takes 342 s on `main` at 8bfd45d, and 270 s of it is the Rust suite joint (`tool.llvm-cov-nextest`, ADR-084), which reran 18 of 2 818 tests (ADR-149). P10.2 and P10.3 both inferred that most of those 270 s are the coverage-instrumented rebuild, the per-test profile export (ADR-083 §1), and the whole report (`cargo llvm-cov report --lcov`, ADR-084 §4), not the tests, but nothing measured it: the store's `durations` (ADR-086 §2) hold one row for the joint's production. The owner's ruling for v0.2.2 (plan, B.1) is to give the instrumented build its own incrementally compiled target directory, keep it warm from the daemon, and bring the warm run under 180 s. Whether the existing `target/cupel-llvm-cov` (ADR-084 §4) already compiles incrementally, and which phase dominates, decide what to change, so the breakdown is decided first, as ADR-150 did for the mutation engine.

What is known before measuring: `cargo llvm-cov` 0.9.1 never sets `CARGO_INCREMENTAL`, and Cupel's workspace sets no `incremental` in its profiles, so a local instrumented build of the workspace's own crates is incremental (cargo's default for the `dev`/`test` profile); CI sets `CARGO_PROFILE_DEV_DEBUG=0` and nothing about incremental. `target/cupel-llvm-cov` is already a target directory of its own, kept between runs by `--no-report`, outside the clean of `target/llvm-cov-target`, and removed by `make reclaim`.

## Options

**Where the breakdown lives.** (a) Rows the scheduler records: it times productions as a whole and knows no producer's phases (rejected in ADR-150 for the same reason). (b) In the fact, as ADR-150 did for `MutationOutcomes`: the joint's `TestResults.reuse` (`SuiteReuse`, ADR-149 §5), which says how the joint obtained its results, gains the phases the run went through, and the CLI records each phase's total in `durations` beside the production's own row. `SuiteReuse` is also the envelope's `run.suite` (ADR-149 §5), so `--format json` shows where a suite run's time went; the goldens redact every `duration_ms`.

**How to time the build apart from the tests.** (a) Two invocations, the compile alone first, then the run as before. `cargo llvm-cov nextest --no-run` is not nextest's flag but `cargo llvm-cov`'s own (deprecated: "report without running") and refuses `--no-report`; nextest's way to build without running is a filterset that matches no test, `-E 'none()'`, which builds every test binary and skips them all without listing them (probed on Cupel: `Starting 0 tests across 0 binaries (272 binaries skipped)`, 0.002 s after the build). The second invocation costs one more cargo start and fingerprint check. (b) One invocation, as now, split by cargo's own report of its build: cargo always ends a build with `Finished … target(s) in <time>` on standard error (`0.35s`, `1m 56s`), even when it compiled nothing; the build is that time and the tests are the rest of the invocation (nextest's listing, its runs, and cargo's start). No process is added and nothing about the run changes; a build that fails prints no such line, and then the invocation was its build.

## Decision## Decision

1. **The phases.** `SuiteReuse` gains `joint` (the joint's producer ID, absent in a fact written before it) and `phases: Vec<SuitePhaseDuration>`, absent from JSON when empty (`TestResults` stays version 2):

   ```text
   SuitePhaseDuration {
     phase: build | test | export | report,
     dir: String,            // the project root the phase ran in, relative to the repository root
     duration_ms: u64,
   }
   ```

   `build` is the instrumented compile of the suite; `test` is the run of the tests that are not kept (ADR-149 §1), with cargo's fingerprint check and nextest's listing of the test binaries; `export` is every test process's profiles merged and exported alone (ADR-083 §1); `report` is the whole run's `cargo llvm-cov report --lcov` (ADR-084 §4). A joint records the phases it runs, one row per phase per root, in the order they ended; a root served from the cache records none, and a joint that times nothing (Python's, Go's) records none.
2. **The Rust joint's build is read from cargo's report** (option (b)): `build` is the time on cargo's `Finished … target(s) in <time>` line in the run's standard error, at most the invocation's; `test` is the rest of the invocation. An invocation with no such line (a build that failed) records `build` alone, the whole invocation. The run's command, its environment, and what it reports are unchanged. `tool.llvm-cov-per-test`'s own run (ADR-083) shares the code.
3. **The store.** When a run is recorded, each language's `TestResults` whose `reuse` names a joint and holds phases adds one `durations` row per phase present, kind `producer`, task `<joint>.<phase>` (`tool.llvm-cov-nextest.build`), class `run`, with the phase's total over its roots, beside the production's own row. `scripts/inner_loop.sh`'s durations query lists them; the budget reads no such shape.
