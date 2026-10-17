# CUPEL_DECISION_105: `conc.race`: the `RaceReports` payload, one producer per race detector, and race and undefined-behavior findings

**Date:** 2026-10-17 · **Status:** accepted · **Phase:** P6.3

## Context

Design §5.4: `conc.race` "TSan, Go `-race`, Miri, Loom exhaustive interleavings for marked sections", kinds `race.detected` and `ub.detected`, cost standard/deep, wrap, tier 4; §5.8 runs it at `standard`. The catalog (§8.3.2) has a `RaceReports` row — "races/UB with stacks", produced by a "race tool" — and the toolchain table (§8.7, ADR-036) a `race` column (`go test -race` for Go; `miri, loom` for Rust; none for Python, JavaScript, TypeScript). `FactKind::RaceReports` existed with no payload. Plan P6.3 asks for producers wrapping Go `-race`, TSan, and Miri, each tool one producer.

Open: what the fact holds so that a check written once reads three tools whose reports look nothing alike (Go's runtime, ThreadSanitizer's compiler-rt format, Miri's rustc diagnostics); how a report becomes stable from run to run when it is full of addresses, goroutine numbers, and allocation IDs, and two racing accesses arrive in whichever order the scheduler chose; which tool runs when a language has two; what the tools need installed and where they write; and what a run that could not complete means.

## Options

**One producer per language running every tool (as `tool.analyzers`, ADR-102) or one producer per tool.** Running Miri and ThreadSanitizer both reruns a Rust suite twice under instrumentation, each run minutes long; the analyzers' argument (one missing linter must not hide the type checker) is weaker here, since either detector finds the races a test exercises. One producer per tool is what the plan asks ("each new tool is one producer"), keeps each tool's cost visible, and lets the registry's order and a pin choose: the toolchain table's order (ADR-076) with a `[lang.<id>] race` pin, as `runner` pins `TestResults`.

**Loom.** Loom runs a model checker over code written against its own `loom::sync` types, under `--cfg loom`, for tests the project marks; it needs the project to depend on it and is closer to `conc.model-check` (§5.4) than to a detector that watches an ordinary test run. It is left out of the `race` column (replaced by `tsan`) until a producer for it exists.

**What a report holds.** (a) The tool's text: nothing to normalize, but no check could compare or locate it. (b) A kind (data race or undefined behavior), the test that was running, a one-line message, and the stacks the tool printed, each a list of frames `function, file, line`. (b), with three normalizations that make a report the same on every run: addresses, goroutine and thread numbers, and allocation IDs are masked; frames outside the repository (the language runtime, the standard library, the registry), whose paths differ between machines, are dropped; and a data race's two access stacks are ordered by their first frame, since which access the detector saw second depends on scheduling.

**Which reports a scope keeps.** A race involves code the change touched when any frame of it is in a file in scope; a report with no frame in the repository cannot be placed and is kept.

**A run that cannot complete** (a build failure, `-race` without cgo, a detector that crashes the test binary): an error of the producer would hide the other roots' reports; recording each root's run with its status lets the check note it, as `AnalyzerReport` does for an analyzer.

## Decision

1. **`race-reports` v1** (`cupel_core::RaceReports`), providable by producer plugins:

   ```text
   RaceReports {
     tool: ProducerId,                              // tool.go-race, tool.miri, tool.tsan
     runs: [RootRun { dir, status: ran | failed, detail }],   // each project root, ProjectInfo order
     reports: [RaceReport {
       kind: data-race | undefined-behavior,
       test: Option<String>,                        // the test that was running, as the runner names it
       message: String,                             // masked
       stacks: [RaceStack {
         access: Option<String>,                    // "read", "write", "non-atomic write", …; null for undefined behavior
         frames: [RaceFrame { function: Option<String>, file, line }],   // in the repository only, innermost first
       }],
     }],
   }
   ```

   `RootRun` and `RunStatus` are shared with `BenchSamples` (ADR-106). Reports are kept in the order the tool printed them, without exact duplicates, when any frame is in a file of the producer's scope or they have no frame in the repository. `FactKind::pin_key` gains `race` for `RaceReports`.

2. **Producers** (`cupel-adapt/src/race/`), each needing `ProjectInfo` and running in every project root of its language that `ProjectInfo` names (none detected: nothing runs, as for the test runners), output read line by line, the test that was running taken from the runner's own progress (`test publishes ... `, test2json's `Test`):

   | Producer | Language | Invocation | Cost | Ready when |
   |---|---|---|---|---|
   | `tool.go-race` | `go` | `go test -race -count=1 -json <packages>`; reports between `WARNING: DATA RACE` and `==================` in each test's output events | standard | `go` on `PATH` |
   | `tool.miri` | `rust` | `cargo +nightly miri test --no-fail-fast -- --test-threads=1` with `MIRIFLAGS=-Zmiri-backtrace=full` (before the user's own; the full backtrace names the functions) and `CARGO_TARGET_DIR` under `.cupel/cache/race/tool.miri`, output and errors merged | deep | `cargo +nightly miri --version` succeeds; else missing, install `rustup toolchain install nightly --component miri` |
   | `tool.tsan` | `rust` | `cargo +nightly test -Zbuild-std --target <host> --tests --no-fail-fast -- --test-threads=1` with `RUSTFLAGS` and `RUSTDOCFLAGS` `-Zsanitizer=thread` and `CARGO_TARGET_DIR` under `.cupel/cache/race/tool.tsan` (doctests are left out: rustdoc does not build them for an explicit target), output and errors merged; reports between `WARNING: ThreadSanitizer:` and `SUMMARY: ThreadSanitizer:` | standard | the nightly sysroot has `rust-src`; else missing, install `rustup toolchain install nightly --component rust-src` |

   - Go: each access header (`Read at 0x… by goroutine 8:`, `Previous write at …`) starts a stack whose access is its first words lowercased without `previous`; its frames are the `function()` / `file:line +0x…` pairs under it; the `Goroutine N (…) created at:` stacks are not accesses and are not kept. Message `data race`.
   - Miri: `error: Undefined Behavior: <message>`; a message starting `Data race detected` is a data race whose accesses are the two `(1)`/`(2)` operations it names (`non-atomic write`, `atomic read`, …), placed at the `-->` span that follows the message (`(2)`) and the one after `and (1) occurred earlier here`; other undefined behavior has one stack: the `-->` span, then the `stack backtrace:` frames. Allocation IDs (`alloc1234` → `alloc`) are masked. Miri stops a test binary at its first undefined behavior, so later tests of that binary are not run (`--no-fail-fast` still runs the other binaries).
   - ThreadSanitizer: the warning's headline is the message (`data race` is a data race; any other warning — `heap-use-after-free`, `thread leak`, `lock-order-inversion` — is undefined behavior); each `Read of size N at 0x… by thread T2:` or `Previous write …` starts an access, its `#N function file:line:col (module+0x…)` lines are frames, Rust's `::h<16 hex>` suffix is removed; `Location is …` and `Thread T2 … created by …` sections are not accesses.
   - A root's run is `failed`, with what failed as its detail, when it could not test what it was given: no test event or test binary run at all (`-race requires cgo`), a package or crate that did not build, a test binary killed by a signal with no report to show for it (ThreadSanitizer's runtime crashes every instrumented binary on this machine's macOS), or an operation Miri does not support (a foreign function, most file and process calls), which ends that test binary. Otherwise it `ran`, whatever the tests' own outcomes. Reports read before a failure are kept.
   - Nothing is written in the tree: builds go to the cache, Go's to its build cache. Miri's sysroot is built once in Miri's own cache directory.
   - Registration order puts `tool.miri` before `tool.tsan` (the registry orders producers the table does not list by ID); Miri is chosen for Rust when both are ready, because it also finds undefined behavior. `[lang.rust] race = "tsan"` picks ThreadSanitizer.

3. **`conc.race`** (`cupel-checks/src/conc_race.rs`): tier 4, cost `standard`, needs `RaceReports`, no settings, stability `experimental`. Each report is a finding:
   - **`race.detected`** (data race) and **`ub.detected`** (undefined behavior), both severity `fail`, action `fix_bug`, confidence 1; located at the first frame of the first stack (no location without one); evidence `{tool, test, message, stacks}` as the fact has them; ID `FindingId::for_subject(check, kind, "<test>|<message>|<function of each stack's first frame>|<n>")` (`n` counts identical subjects in the run), stable across line moves; repro `cupel check --only conc.race --paths <file>` (`cupel check --only conc.race` without a location). Titles name the accesses and functions (``data race: `write` in `units::count` and `read` in `units::count` ``) or the message.
   - Score `{value: reports, unit: "reports", n: runs that ran}`; metrics `races` and `undefined_behavior`, summed across languages. A failed run is a note (`<tool> failed in <dir>: <detail>`). Status `fail` with any finding.

4. **Support.** `go` and `rust` are supported (Rust needs a nightly toolchain); `python`, `javascript`, and `typescript` have no race detector producer and are `unsupported(race-reports)`: neither runtime has a data-race detector for its own code (CPython's free-threaded build has none in its test tooling; Node has no shared-memory threads outside `SharedArrayBuffer`).

## Consequences

- `conc.race` runs a language's whole suite under the detector: on Go that is close to the baseline's cost; Miri interprets every test and refuses what it cannot model (process spawning, most file-system and network calls), so a suite that does those gets a `failed` run naming the operation, beside whatever Miri found before it. `make dogfood` skips `conc.race` for that reason (Cupel's own tests spawn processes everywhere).
- Under `--changed`, a race that a changed test exposes in unchanged code is reported when one of its frames is in a changed file; one entirely in unchanged code is not.
- Messages and frames follow tool versions; CI pins the nightly toolchain (and so Miri) the goldens were made with.
- Loom, Shuttle, and other model-checking harnesses are not wrapped; they fit `conc.model-check` (P7) or a later producer of this fact.
- ThreadSanitizer's frame lines differ by platform: on Linux the module offset is followed by `(BuildId: …)`, so the reader strips every trailing parenthesized group before the place, not one. The first two Ubuntu CI runs (39060938864, 39066145726) read every frame of the race as placeless and `race.detected` had no location. Where the runtime has no symbolizer of its own, the frames are addresses unless `llvm-symbolizer` is on `PATH`; CI installs `llvm` on Linux.
- ThreadSanitizer is exercised by its parser's tests on real compiler-rt output and by a real-tool test that runs where an instrumented binary can run (on this machine's macOS it crashes the binary at start, and the test says `SKIPPED`).
