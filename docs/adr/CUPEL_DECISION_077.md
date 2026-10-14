# CUPEL_DECISION_077: The `PerTestMap` payload, the `Runner` selection surface, and `tests --impacted`

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P3.3

## Context

`PerTestMap` has been a fact kind since P0.2 with no payload. Design §8.3.2 gives its content as `test_id → lines`, produced by a coverage tool and consumed by `tests --impacted` and the mutation engine; §9.4 inverts it to `line → tests`. §8.7 names the `Runner` trait (`list()`, `run(selection, env, timeout)`), which P3.1 deferred to its first consumer: this step selects tests and must hand the selection to a runner as a command. §6.5 says `tests --impacted` returns "an envelope with `tests: [{id, file, reason}]` and a ready-to-run `command` per runner" and "falls back to file-level impact when no per-test map exists and says so". The design is silent on which IDs the map uses, what "lines" means (executed, instrumented, or both), how a verb that runs no check gets facts from the scheduler, what a changed scope means at line level, and where the verb's output sits in the envelope.

## Options

**IDs.** The map could name tests by its own scheme (a coverage tool's context label) or by the IDs a runner reports in `TestResults` (ADR-075). Only the second lets a consumer hand a selection to the runner and join the map with test outcomes.

**Lines.** Keep hit counts per line (a per-test `CoverageMap`), or only the lines each test executed. Impact and mutant selection ask one question, "does this test execute this line", and hit counts per test multiply the fact's size for no consumer.

**Facts for a verb.** `tests --impacted` could be a check (it is not: it scores nothing and emits no findings), call producers itself (duplicating selection, upstream needs, and pins), or ask the scheduler for facts the way a run would produce them.

**Changed lines.** `--changed` and `--base` resolve to files; every symbol of a changed file would select every test of that file, so "change one function, get one test" needs the lines a range changed.

**Envelope.** A top-level `tests` array (as the design writes it) plus a separate fallback flag and command list would add three fields; one object holds them together.

## Decision

1. **`cupel_core::PerTestMap`, version 1:**

   ```text
   PerTestMap {
     runner: ProducerId,             // the TestResults producer whose IDs `tests` uses: tool.cargo-nextest, tool.go-test, tool.pytest
     tests: [{                       // sorted by id
       id: String,                   // that runner's test ID (ADR-075), root-prefixed the same way
       file: Option<String>,         // the test's own file, when the coverage tool names it
       covered: [{ file: String,     // relative to the repository root, sorted
                   lines: [u32] }],  // 1-based, ascending, unique: lines the test alone executed
     }],
   }
   ```

   A test that executed nothing is listed with no `covered` entries, so the map says it was measured. `PerTestMap::index()` is the inverted index `file → line → tests`; `covering(file, lines)` answers which tests execute any of `lines`, with the first line each one executes.

   **Version rule:** bump `VERSION` on a field renamed, removed, or retyped; on a change of what `lines` means; or when the runner whose IDs the map uses changes its ID formation (ADR-075 bumps `TestResults` for the same reason, and both must move together). A new producer, or an optional field older readers may ignore, does not bump it.

2. **Producers** (`cupel-adapt/src/per_test/`, cost `deep`, `needs = [ProjectInfo]`, one run per project root):
   - **Rust, `tool.llvm-cov-per-test`:** lists tests with `cargo nextest list --message-format json`; cleans once (`cargo llvm-cov clean --workspace`); per test, removes profiles (`cargo llvm-cov clean --profraw-only`) and runs `cargo llvm-cov nextest --no-clean --lcov --output-path <report> --workspace -E 'binary_id(=<b>) & test(=<t>)'`, so the instrumented build happens once. IDs are nextest's.
   - **Go, `tool.go-cover-per-test`:** lists with `go test -list ^Test -json <packages>`; per test, `go test -coverprofile=<report> -coverpkg=<packages> -run '^<Test>$' <package>`; profile paths are import paths, made file paths through the root's `go.mod` module line. `_test.go` lines are never in a Go profile. IDs are `go test`'s.
   - **Python, `tool.pytest-cov-per-test`:** one run, `pytest -p no:cacheprovider --cov=. --cov-context=test --cov-config=<rc> --cov-report=json:<report>` with a coverage configuration in `.cupel/` that sets the data file there and `show_contexts`; each line's contexts (`<node id>|run`, `|setup`, `|teardown`) name the tests. IDs are pytest node IDs.
   - **JavaScript and TypeScript:** none ([ADR-006](CUPEL_DECISION_006.md)).

   Reports and data files live under `.cupel/cache/per-test/<producer>/<root>/`; nothing is written into the tree outside the tools' own `target/`. Availability is every program the producer runs on `PATH` (`cargo-llvm-cov` and `cargo-nextest`; `go`; `pytest`), and `Producer::tool` names the coverage tool for `doctor`. A run that leaves no report for a test records the test with nothing covered; a root whose listing or run fails is `Error::Process` with the tail of the tool's output, which `tests --impacted` turns into a fallback.

3. **The `Runner` trait** (`cupel_core::Runner: Producer<TestResults>`):

   ```text
   list(ctx) -> Result<Vec<String>>                                   // test IDs, without running them
   run(ctx, selection, env, timeout) -> Result<TestResults>            // selection: All | Tests(ids) | Files(paths)
   commands(ctx, selection) -> Option<Vec<RunnerCommand { dir, command }>>   // None: the runner cannot narrow to it
   ```

   Runners register with `ProducerRegistry::register_runner`, which registers the producer and keeps the runner for `ProducerRegistry::runner(id)`. Every runner runs `All`. Narrowing: `cargo nextest` tests (a filterset), `go test` tests (`-run` per package) and files (their packages), `pytest` tests and files (node IDs and paths), `unittest` tests (dotted IDs), `vitest` and `jest` files. `list` exists for `cargo nextest`, `go test`, and `pytest`; the others return `Error::InvalidValue` naming the runners that list. `timeout` is a child of the run's cancellation: the run's own `--timeout` still cancels (`Error::Cancelled`), while the per-call timeout ends only that run, which reports an incomplete invocation (`exit_code: null`, output "timed out after …").

4. **Facts for a verb.** `Scheduler::production(job)` returns a `Production`, the scheduler's fact store opened for a verb: `ensure(kind, lang, sink)` produces exactly as a check run would (pins, detection, upstream needs, the `TestResults` gate), and the producers' findings come back with the verb's envelope. `SupportMatrix::fact(producers, kind, lang, ctx)` says whether one fact can be produced, with the same reasons as a check's cell.

5. **Line-level scope.** With a range (`--changed`, `--base`), `Git::changed_lines` reads `git diff -U0` and the verb widens each changed hunk to the symbols (`Symbols`) it touches; added files count whole. With `--symbols`, the selected symbols' spans; otherwise the whole of each file in scope. A per-test language selects the tests whose map covers any of those lines, plus the tests whose own definition (`TestFrames`) the lines touch; a fallback language selects the test files (`TestConventions`) that are in scope or import a file in scope through `ModuleGraph`.

6. **Envelope.** `Envelope.tests` (null for every other verb, ADR-013):

   ```text
   tests: {
     per_test_map: bool,             // true only when every language in scope used a per-test map
     impacted: [{ id, file, reason, lang }],
     commands: [{ lang, runner, dir, command, narrowed }],   // command: shell words from the repository root
     languages: [{ lang, per_test_map, producer, fallback }], // fallback: why the language used ModuleGraph impact
   }
   ```

   The design's `tests: [{id, file, reason}]` is `tests.impacted`. A fallback language's IDs are its test files. `narrowed: false` marks a command that runs the whole suite because the runner cannot narrow to the selection.

## Consequences

- The mutation engine (P3.4) selects the tests covering a mutated line through `index().covering(...)` and runs them through `ProducerRegistry::runner(map.runner).run(Tests(ids), ...)`, with per-mutant timeouts from `run`'s `timeout`.
- The Rust producer runs one nextest process per test after one instrumented build; it is `deep` and suits impact and mutation on changed code, not a large suite on every run. `PerTestMap` is not cached in the `facts` table yet, so every `tests --impacted` remeasures.
- Every envelope gains `"tests": null`; goldens written before this step change by that line.
- A test in a Go `_test.go` file is impacted by a change to its own body only through `TestFrames`, since Go profiles never include test files.
