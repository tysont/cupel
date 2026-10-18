# CUPEL_DECISION_120: `conc.dst`: the `SimulationResults` fact, madsim harnesses over a seed range, and replaying the failing seed

**Date:** 2026-10-17 · **Status:** accepted · **Phase:** P7.4

## Context

Design §5.4: `conc.dst` runs "deterministic simulation campaigns (madsim/turmoil/Shuttle harness; Antithesis via adapter) with seed ranges", kind `dst.failure` "(seed + trace)", cost exhaustive, wrap; §4 puts it at tier 4 and §5.8 runs "long fuzz/DST campaigns" at `exhaustive`. The fact catalog (§8.3.2) and P7's list of new facts (`Traces`, `Histories`, `ProofResults`, `ContractViolations`) name no fact for a simulation campaign: adding one is the standing trigger for an ADR (plan §2.5). Nothing says how a project declares a harness, how `--seed` and `--cases` become a seed range, what the trace of a failure is when a harness runs many seeds in one process, how a failing seed is reproduced, or which of the named frameworks Cupel can wrap.

## Options

**A fact of its own or an existing one.** `FuzzResults` (ADR-107) is per target with one crash input; a campaign's result is per harness with a seed, which is not an input, and a printed trace. `TestResults` has no seed. `Traces` is P7.2's (`spec.trace`), an implementation trace validated against a spec. A new kind, **`SimulationResults`**, keeps each check reading one fact.

**Declaring the harness.** (a) A configured command run once per seed with `CUPEL_SEED`: language-neutral, but it reruns the harness's process per seed (a build check, a binary start), it is a convention of Cupel's no framework follows, and nothing names the harnesses inside it. (b) Detect a framework's own convention. madsim's `#[madsim::test]` reads `MADSIM_TEST_SEED` (the first seed) and `MADSIM_TEST_NUM` (how many), runs the seeds one after another in one process under `--cfg madsim`, stops at the first failing seed, and prints `note: run with MADSIM_TEST_SEED=<n> environment variable to reproduce this error`; its random generator is xoshiro, so a seed replays the same schedule on every platform. turmoil takes a seed through `Builder::rng_seed` and has no environment convention; Shuttle's `check_random` draws its seeds from the operating system and replays a printed schedule string, not a seed, and it tests thread interleavings of one process (closer to `conc.model-check`). (b) for madsim, which has a seed-range convention end to end.

**The range.** ADR-107's precedent is the case budget (`Focus::case_budget`: `--cases`, else 100, 1 000, 10 000, 100 000 by depth), one case being one execution; here one case is one seed, so the range is `[seed, seed + cases)` from the run's seed, and `--seed N --cases 1` runs exactly seed `N`.

**The trace.** madsim's process prints every seed's output into one captured stream, with no boundary between seeds, so the campaign's output of a failing harness holds the passing seeds' output before it. Replaying the failing seed alone (`MADSIM_TEST_NUM=1`) gives exactly that seed's output and also proves the seed reproduces: a failure that does not repeat alone is a harness that is not deterministic, which is worth saying.

## Decision

1. **`simulation-results` v1** (`cupel_core::SimulationResults`, a catalog kind appended after `histories`, design §8.3.2 row, schema, providable by producer plugins):

   ```text
   SimulationResults {
     tool: ProducerId,                  // tool.madsim
     seed: u64, seeds: u64,             // the range: seed .. seed + seeds - 1
     runs: [RootRun],                   // each project root's campaign: ran | failed (a build error, no test run)
     harnesses: [SimulationHarness {
       root, name,                      // the test as libtest names it
       location: Option<Location>,      // its #[madsim::test] function
       status: passed | failed,
       failure: Option<SimulationFailure {
         seed: Option<u64>,             // the first failing seed, when the simulator names it
         message, location: Option<Location>,   // the panic's first line and place in the repository
         trace: [String],               // what the replayed seed printed, at most the last 50 lines
         reproduced: bool,              // whether replaying the seed alone failed again
         reproduce: String,             // the command that replays the seed alone, from the project root
       }>,
     }],
   }
   ```

   Its conformance inventory `cupel-fixtures/conformance/simulation-results.toml` lists `passes-every-seed`, `fails-at-a-seed`, and `fails-in-a-task` (a panic in a spawned task is the seed's failure too), located by `file` and `name` (`conformance::EXECUTION`, as ADR-119 §1); the madsim fixture's mapping pins the failing seeds and places.

2. **`tool.madsim`** (`cupel-adapt/src/madsim.rs`): Rust, needs `ProjectInfo`, cost `exhaustive`, ready when `cargo` is on `PATH`. In every Rust project root `ProjectInfo` names that holds a file of the scope (every root for an empty scope, as ADR-107 §2): `cargo metadata --no-deps --offline` lists the packages depending on `madsim`; their `#[madsim::test]` functions (by a scan of the package's `.rs` files) are the harnesses; each library and test target runs as `cargo test -p <package> --lib|--test <t> --no-fail-fast` with `RUSTFLAGS=--cfg madsim`, `CARGO_TARGET_DIR=.cupel/cache/dst/madsim/target`, `MADSIM_TEST_SEED=<seed>`, `MADSIM_TEST_NUM=<seeds>` (the seed is lowered when `seed + seeds` would pass `u64::MAX`). A test libtest reports whose function name is one harness's is that harness (`ok` passed, `FAILED` failed; tests that are not harnesses, and harness names defined twice, are left out). A failed harness's seed is the one madsim's note names; that seed is replayed alone (`<test> -- --exact`, `MADSIM_TEST_NUM=1`); the trace is the replay's `---- <test> stdout ----` section (the campaign's when the replay passed) without `note:` lines, with panicking threads' numbers and absolute `spawned at` places outside the root removed and the root's paths made relative; the message and place are the panic's (`panicked at <file>:<line>:<col>:` and the line after). `reproduce`: `RUSTFLAGS='--cfg madsim' MADSIM_TEST_SEED=<n> MADSIM_TEST_NUM=1 cargo test -p <package> <selector> <test> -- --exact`. A run with a build error or no test run is `failed` with the error (race's `cargo::failure`). Nothing is written in the tree.

3. **`conc.dst`** (`cupel-checks/src/conc_dst.rs`): tier 4, cost `exhaustive`, needs `SimulationResults`, no settings (the range is `--seed` and `--cases`), stability `experimental`. **`dst.failure`** (`fail`, `fix_bug`; confidence 1 when the seed reproduced alone, 0.5 when it did not) per failed harness; subject `<root>|<name>`; located at the failure's place, else the harness's; title `` simulation `<name>` fails at seed <n>: <message> `` (without ` at seed <n>` when none is named); evidence `{harness, tool, seed, first_seed, seeds, message, trace, reproduced, reproduce}`; the action names the reproduce command; repro `cupel check --only conc.dst --paths <harness file, else root> --seed <n> --cases 1` (a failure without a seed: `--seed <first> --cases <seeds>`). Metrics, summed across languages: `harnesses`, `failures`; score `{value: failures, unit: "failing", n: harnesses}`; status `fail` with any failure. Notes: each failed run (`<tool> failed in <dir>: <detail>`), each root that ran with no harness (`<tool> found no simulation harness in <dir>`).

## Consequences

- `conc.dst` is supported for Rust (madsim), `unsupported(simulation-results)` for Go, Python, JavaScript, TypeScript, Java, and Clojure: none of their ecosystems has a deterministic simulation framework with a seed convention Cupel wraps (Go and the JVM languages have Antithesis's SDKs, which run only on Antithesis's platform; Python and JavaScript have none in common use).
- A failing seed is found deterministically: madsim's xoshiro generator repeats a seed's schedule on every platform for one madsim version, so the Rust golden (from seed 3, 1 000 seeds at the matrix's `standard`) holds seeds 4 and 5; the fixture's `Cargo.lock` pins madsim 0.2.34, and the first build fetches it from crates.io.
- A campaign builds the root under `--cfg madsim` into a target directory of its own, a full build the first time; `--changed` runs every harness of a root with a changed Rust file, since a harness exercises code across files.
- Every seed of the range runs in one process per target, so `exhaustive`'s 100 000 seeds per harness can take minutes; a harness stops at its first failing seed, and later failures are found by a run from the next seed.
- Not done, with the reason: **turmoil** has no seed convention (a harness passes `rng_seed` itself), so wrapping it needs a convention of Cupel's, such as a harness reading `CUPEL_SEED`, and a way to name its harnesses; **Shuttle** draws seeds from the operating system and replays schedule strings, not seeds, and checks thread interleavings within a process, which P7.5's `conc.model-check` covers; **Antithesis** is a hosted, commercial platform (an account, container images pushed to it, its API for results), not a tool installable on this machine or a CI runner; its adapter would be a producer plugin (design §8.3.8) emitting `SimulationResults`. madsim's determinism check (`MADSIM_TEST_CHECK_DETERMINISM`) is not run; the replay is the cheaper evidence of repetition.
