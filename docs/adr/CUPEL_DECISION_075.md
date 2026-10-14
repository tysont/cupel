# CUPEL_DECISION_075: The `TestResults` payload and its version rule

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P3.1

## Context

`TestResults` has been a fact kind since P0.2, with no payload type. Design §8.3.2 gives its content as "per test: outcome, duration", produced by a runner tool and consumed by the baseline refusal (exit 4), `tests.flaky` (P3.5), and the mutation engine (P3.4). P3.1 adds the first producers (`cargo test`, `cargo nextest`, `go test -json`, `pytest`, `unittest`, `vitest`, `jest`), so the shape must be fixed before their parsers are written. The design is silent on identity (what makes two runs' tests "the same test"), on suite-level failures (a package that does not build, a test file that does not load), on which output is kept, and on what invalidates the fact.

## Options

**Outcome and duration only, as the catalog row says.** Enough for a pass/fail baseline, but the exit-4 envelope could not say which test failed or why, the mutation engine could not key per-test timeouts (P3.4 step 5) or covering tests to an identity, and a run that never reached its tests (a compile error) would look green.

**The runner's report, kept raw.** Every runner's format differs; every consumer would parse JUnit, libtest text, test2json, and Jest JSON again. That is the tool knowledge the fact layer exists to contain (design §8.3.1).

**A normalized per-test record plus the invocations that produced it.** Each producer maps its format to one shape: a stable ID, a file, a name, one of four outcomes, a duration when the runner measures one, and the tail of the output of a failure. The invocations record what ran and whether each run reached its end, so "the suite is red" covers both failing tests and a runner that never got that far.

## Decision

The third option. `cupel_core::TestResults`, version 1:

```text
TestResults {
  runner: ProducerId,                 // tool.pytest, tool.cargo-test, ...
  invocations: [{
    dir: String,                      // the project root it ran in, relative, "." for the repository root
    command: [String],                // program and arguments, as run
    exit_code: Option<i32>,           // null when killed by a signal
    completed: bool,                  // the runner reached the end of the suite and reported it
    output: Option<String>,           // the tail of the runner's output when it did not complete
  }],
  tests: [{                           // sorted by id
    id: String,                       // unique within the fact; stable across runs of one runner
    file: Option<String>,             // relative to the repository root, when the runner names one
    name: String,                     // the runner's display name of the test
    outcome: passed | failed | skipped | errored,
    duration_ms: Option<u64>,         // null when the runner does not measure it
    output: Option<String>,           // failed and errored only: the tail of the failure message and captured output
  }],
  suite_hash: String,                 // blake3 hex of the sorted test IDs and the content of every file they name
}
```

- **Outcomes.** `failed`: an assertion or panic in the test. `errored`: the test or its suite could not run (a JUnit `<error>`, a Jest suite that failed to load, a Go package that did not build); the producer names the suite as the test when the runner names no test. `skipped` covers ignored, pending, todo, and skipped tests. Expected failures count as `passed`, unexpected successes as `failed`.
- **The suite is green** (`TestResults::is_green`) when every invocation completed and no test is `failed` or `errored`. `failures()` lists the tests that make it red.
- **IDs** are the runner's own identity of a test, made unique across roots: a root other than `.` prefixes `<dir>: `. Per runner: `cargo test` `<binary>/<file>::<path>` (the binary from the `target/debug/deps` stem, the file as libtest prints it); `cargo nextest` `<binary-id> <path>` (nextest's own identity); `go test` `<package>/<Test>`; `pytest` the node ID `<file>::<class>::<name>`; `unittest` the dotted `<module>.<Class>.<method>`; `vitest`/`jest` `<file> > <ancestor titles> > <title>`.
- **Output tails** are at most 40 lines, the last lines of what the runner reported for the failure; passing tests keep none, so the fact stays small.
- **The suite hash** identifies the suite that ran: change a test file, add or remove a test, and it changes. It is what the mutation engine's reuse key (design §8.8, `suite_hash`) starts from when no per-test map exists.
- **No producer sees a check, and no check sees a runner.** Checks read outcomes and IDs; the runner ID is evidence, never a branch.

**Version rule.** `VERSION` is bumped whenever a consumer that stored or compared an older payload could be wrong about a newer one: a field renamed, removed, or retyped; an outcome added, removed, or given a new meaning; or **any runner's ID formation changing**, because IDs key stored test results, per-test maps, and mutation reuse across runs. Adding a runner, adding an optional field that older readers may ignore, or changing how much output a tail keeps does not bump it.

## Consequences

- The exit-4 envelope names each failing test with its file and output (ADR-076), and the store's `test_results` index has one row per test.
- `cargo test` reports no per-test durations on stable Rust (`--report-time` is unstable), and `unittest`'s `--durations` exists only from Python 3.12, so both producers leave `duration_ms` null; the mutation engine's per-test timeout (P3.4) falls back to the run's duration for them.
- `cargo test` prints a test's file relative to its package, so in a workspace a member's tests carry the member-relative path; their IDs are still unique (the binary name differs).
- `cargo nextest` does not list ignored tests in its JUnit report, so its results have no `skipped` entries; `cargo test` reports them.
- A per-test map (P3.3) and flake reruns (P3.5) key on `id`; changing an ID scheme is a version bump that discards their history.
