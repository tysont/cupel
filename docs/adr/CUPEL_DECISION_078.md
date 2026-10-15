# CUPEL_DECISION_078: Repeated test runs are their own fact, `TestReruns`

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P3.5

## Context

`tests.flaky` (design §5, §8.5) reports tests whose outcome changes across reruns. Design §8.5 says "a flake engine produces repeated `TestResults`" and the catalog (§8.3.2) lists `tests.flaky` as a consumer of `TestResults`, but the catalog has no field for more than one run, and P3.1 (ADR-075, ADR-076) gave `TestResults` one meaning: the baseline run, whose redness refuses the whole run with exit 4. The engine must also vary what makes a rerun differ (test order, seed, parallelism) and record it, or the check could not tell a test whose outcome follows the order from one whose outcome changes with nothing. And producers could not see the run's seed (`--seed`), so reruns could not be reproduced.

## Options

**Extend `TestResults` with an optional `runs: Vec<Run>`** (per-run order, seed, parallelism, and outcomes), bumping its version. Readers of single runs ignore the field, so it is backward-compatible as a shape. It is not as a meaning: the scheduler makes one fact per (kind, language) and selects among `TestResults` producers by pin and detection (ADR-076), so the engine would compete with the runners for the baseline's slot; every `TestResults` passes the baseline gate, which withholds a red one and refuses the run, and a rerun set that found a flaky test is red by construction; and the engine needs the baseline as input, so producing the same kind is a dependency cycle (`needs = [TestResults]` → `TestResults`).

**A sibling fact kind, `TestReruns`, produced by `engine.flake` from the baseline.** The engine needs `TestResults` (so a red baseline still refuses the run before any rerun, ADR-076 §5), reruns the tests the baseline reported, and reports each rerun with what it varied. Single-run consumers (the baseline gate, the mutation engine) are untouched. Adding a kind costs a catalog row and this ADR (plan §2.5); it is an execution fact, so no language owes it a producer and there is no conformance inventory (as for `TestResults`, P3.1 Notes).

## Decision

The sibling kind. `FactKind::TestReruns` (`test-reruns`), payload `cupel_core::TestReruns`, version 1:

```text
TestReruns {
  runner: ProducerId,              // the runner whose baseline was rerun, such as tool.pytest
  seed: u64,                       // the run's seed (`--seed`, else random); every rerun's seed derives from it
  runs: [Rerun {                   // in the order they ran
    order: declared | reversed | shuffled,
    seed: u64,                     // the seed this rerun handed the runner (shuffle seed, hash seed)
    parallelism: Option<u32>,      // the worker or thread count asked for; null for the runner's default
    executed: Option<[String]>,    // test IDs in the order they ran, when the runner reports it
    invocations: [RunnerInvocation],   // as in TestResults (ADR-075)
    tests: [TestResult],           // as in TestResults, sorted by ID; IDs are the baseline's
  }],
}
```

- **The schedule** derives from the run seed alone, so `--seed N` reproduces it: rerun `k` gets seed `splitmix64(seed + k)`. Reruns 0 and 1 run the declared order serially (they differ only in seed); rerun 2 the reversed order where the runner takes an explicit order (pytest node IDs, unittest names), else a seeded shuffle (`go test -shuffle`, libtest `--shuffle-seed`, `jest --randomize --seed`, `vitest --sequence.shuffle --sequence.seed`); reruns from 3 on are seeded shuffles, alternately at the runner's default parallelism and serial. `[lang.<id>] flake_runs` sets the count (default 3, the design's `tests.flaky ×3` at `standard`).
- **The order key** of a rerun is `executed` when present, else its `order` (with the seed for `shuffled`). Two reruns with one key ran the tests in the same order.
- **Classification** (in `tests.flaky`, over reruns where a test passed, failed, or errored): a test whose outcome differs between two reruns with the same order key is `test.flaky` — nothing but chance (or the seed) changed. A test whose outcome differs only between reruns with different keys is `test.order-dependent`; its evidence names the tests that ran before it in every failing rerun and after it in some passing one (the likely polluters), when the reruns report their order.
- **The seed reaches producers.** `Job` and `ProduceCtx` gain `seed` (the CLI passes the run's seed; `ProduceCtx::new` uses 0), as design §6.1 rule 7 already requires of every run.
- **No baseline, no reruns.** Given no upstream `TestResults` (a producer called outside the scheduler), the engine produces an empty `TestReruns`; it never runs a suite the baseline did not first run green.

## Consequences

- `tests.flaky` gets exit 4 for free: its needs reach `TestResults` through the engine.
- The engine drives each runner itself through a per-runner rerun table (`cupel-adapt/src/flake/`), reusing the runners' report parsers, because the runner producers expose no way to pass a selection, arguments, or environment until P3.3's `Runner::run(selection, env, timeout)`; `RunOptions` (selection in order, seed, parallelism, and the argv and environment each runner turns them into) is what that surface must absorb, after which the engine calls the registered runner. `cargo nextest` runs each test in its own process, so order cannot matter there; the engine does not rerun it and says so.
- The libtest shuffle is unstable: the engine runs test binaries through `--config target.'cfg(all())'.runner = ["env", "RUSTC_BOOTSTRAP=1"]`, which reaches the test binaries only, never `rustc`, so nothing rebuilds; doc-tests are not rerun.
- A test that fails at random half the time is classified `test.flaky` only when two same-order reruns happen to disagree; otherwise, if reruns in other orders disagree, it is reported `test.order-dependent` with lower confidence and no polluter. More reruns (`flake_runs`) lower the chance.
