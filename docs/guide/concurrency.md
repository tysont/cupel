<!-- ABOUTME: Race detectors, linearizability checking, and deterministic simulation. -->
<!-- ABOUTME: Part of Cupel's guide, linked from the README. -->
# Concurrency

## Race detectors

`conc.race` runs a language's tests in each project root under its race detector, and reports what the detector saw:

| Language | Detector | Run as |
|---|---|---|
| Go | Go's race detector | `go test -race -count=1 -json` over the module's packages |
| Rust | Miri (`tool.miri`, the default) | `cargo +nightly miri test --no-fail-fast`, one test at a time, with full backtraces |
| Rust | ThreadSanitizer (`tool.tsan`) | `cargo +nightly test -Zbuild-std --target <host> --tests` with `-Zsanitizer=thread` |

Miri interprets every test, so it finds undefined behavior (an out-of-bounds read, a dangling pointer, an invalid value) as well as data races, but it refuses what it cannot model (foreign functions, most file and process calls); ThreadSanitizer runs the tests natively and finds data races only. Both need a nightly toolchain (`rustup toolchain install nightly --component miri --component rust-src`); `[lang.rust] race = "tsan"` picks ThreadSanitizer. Builds go to `.cupel/cache/race/`, so nothing is written in the tree. Python, JavaScript, and TypeScript have no race detector, and the matrix says `unsupported`.

A report's two accesses are listed in the order of their place in the code, whichever happened first, and only frames in the repository are kept, so the same race reads the same on every run and machine. On the Go fixture, where `Publish` reads a variable a goroutine writes without synchronizing:

```text
$ cupel check --only conc.race --format text
fail (1)
  conc.race
    data race: `write` in `example.com/tally.Publish.func1` and `read` in `example.com/tally.Publish`
      fnd_c9f4ec · race.detected · tier 4 · confidence 1
      at tally.go:10
      evidence: message "data race" · stacks [2] · test "TestPublish" · tool "tool.go-race" (full: cupel explain fnd_c9f4ec)
      action fix_bug (priority 1): Synchronize the accesses (a lock, an atomic, a channel) or stop sharing the memory, then rerun the race detector.
      repro: cupel check --only conc.race --paths tally.go
```

The evidence holds the `tool`, the `test` that was running, the detector's `message` (addresses and allocation IDs masked), and the `stacks`, each an `access` (`read`, `write`, `non-atomic write`; none for undefined behavior) and its `frames`. Under `--changed` a report is kept when one of its frames is in a changed file. A project root whose run could not test what it was given (a build failure, `-race` without cgo, an operation Miri does not support) is a note, beside whatever was found before it. The gate keys are `conc.race.races` and `undefined_behavior`.

## Linearizability

`conc.linearizability` checks histories of concurrent operations that your own tests or harness record: Cupel never runs the system itself. Name the command in `[lang.<id>] history_command`; Cupel runs it from the repository root with `CUPEL_HISTORY_DIR` set to an empty directory under `.cupel/cache/linearizability/` and `CUPEL_SEED` set to the run's seed, and checks every `*.jsonl` file the command writes there. Until the command is set the check is `unconfigured` for the language.

A history is JSON lines in the order things happened. The first line declares the model, `register` (one value: `read`, `write`, and `cas` with the value `[expected, new]`) or `kv` (the same per `key`, checked key by key); each other line is an event of a process, an `invoke` and then its completion, `ok` (it took effect), `fail` (it did not), or `info` (unknown: a timeout, a crash). An invocation the history never completes is unknown too, and may have taken effect anywhere after it began.

```text
{"model": "register"}
{"process": 0, "type": "invoke", "f": "write", "value": 1}
{"process": 0, "type": "ok", "f": "write", "value": 1}
{"process": 0, "type": "invoke", "f": "write", "value": 2}
{"process": 0, "type": "ok", "f": "write", "value": 2}
{"process": 1, "type": "invoke", "f": "read", "value": null}
{"process": 1, "type": "ok", "f": "read", "value": 1}
```

Porcupine (v1.3.1, run with `go run` through a small runner Cupel writes into `.cupel/cache/linearizability/porcupine/`) decides whether some order of the operations, consistent with real time, is a legal run of the model. The read above began after the write of 2 completed, so no order explains it:

```text
$ cupel check --only conc.linearizability --format text
fail (1)
  conc.linearizability
    history `histories/stale-read.jsonl` (register) is non-linearizable: `read 1` by process 1 (line 6) cannot follow state `2`
      fnd_a1c5c8 · consistency.violation · tier 5 · confidence 1
      evidence: anomaly "non-linearizable" · command "cp -R histories "$CUPEL_HISTORY_DIR"/" · history "histories/stale-read.jsonl" · linearized [2] · model "register" · seed 3 · states [1] · stuck [1] · tool "tool.porcupine" (full: cupel explain fnd_a1c5c8)
      action fix_bug (priority 1): Find why `read 1` by process 1 (line 6) cannot follow state `2` in `histories/stale-read.jsonl`: the operations a linearizable system completes take effect in an order real time allows. `CUPEL_SEED=3 cp -R histories "$CUPEL_HISTORY_DIR"/` records the history again.
      repro: cupel check --only conc.linearizability --seed 3
```

The evidence holds the `linearized` operations (the last three of the longest prefix the checker could order), the `states` the model is in after them, and the `stuck` operations that could come next in real time but that the model does not allow; each operation names its `process`, `f`, `key`, `value`, `outcome`, and the `line` that invoked it. A history that is malformed, or declares a model Porcupine does not check, is a note; so is one the checker ran out of time on (30 s per key, 300 s at `exhaustive`). The command's failure is a note, and the histories it wrote are still checked. Porcupine is downloaded from the Go module proxy on the first run; Go must be on `PATH`. The gate keys are `conc.linearizability.violations`, `histories`, `operations`, `unknown`, and `invalid`.

## Deterministic simulation

`conc.dst` runs deterministic simulation harnesses over a range of seeds. A simulator replaces a program's clock, network, scheduler, and randomness with one driven by a seed, so a seed replays the same interleaving, delays, and faults every time. Cupel wraps [madsim](https://github.com/madsim-rs/madsim): in each Rust project root holding a file in scope, every package that depends on `madsim` has its library and test targets run as `cargo test` under `RUSTFLAGS=--cfg madsim`, building into `.cupel/cache/dst/madsim/target`; its `#[madsim::test]` functions are the harnesses. The seeds are the run's seed and the ones after it: `--cases`, else 1 000 at `--depth standard` (with `--only`), 10 000 at `deep`, and 100 000 at `exhaustive`, where the check runs by default. madsim stops a harness at its first failing seed and names it; Cupel then replays that seed alone, which gives the trace of that seed only and shows whether the failure repeats. Other languages have no simulator Cupel wraps, and the matrix says `unsupported`.

On the fixture, where two deposits read a balance, wait on an audit, and write it back, so overlapping audits lose one:

```text
$ cupel check --only conc.dst --seed 3 --format text
fail (2)
  conc.dst
    simulation `a_withdrawal_waits_for_its_deposit` fails at seed 5: overdrawn: 10 from 0
      fnd_d7a5ab · dst.failure · tier 4 · confidence 1
      at src/lib.rs:29:9-29
      ...
    simulation `concurrent_deposits_are_all_kept` fails at seed 4: assertion `left == right` failed: a deposit was lost
      fnd_350eac · dst.failure · tier 4 · confidence 1
      at tests/sim.rs:23:5-23
      evidence: first_seed 3 · harness "concurrent_deposits_are_all_kept" · message "assertion `left == right` failed: a depo…" · reproduce "RUSTFLAGS='--cfg madsim' MADSIM_TEST_SEE…" · reproduced true · seed 4 · seeds 1000 · tool "tool.madsim" · trace [10] (full: cupel explain fnd_350eac)
      action fix_bug (priority 1): Fix the failure `concurrent_deposits_are_all_kept` shows at seed 4 (`RUSTFLAGS='--cfg madsim' MADSIM_TEST_SEED=4 MADSIM_TEST_NUM=1 cargo test -p ledger --test sim concurrent_deposits_are_all_kept -- --exact` replays it from `.`), then keep the seed as a regression case of the harness.
      repro: cupel check --only conc.dst --paths tests/sim.rs --seed 4 --cases 1
```

The repro runs that one seed again (`--seed 4 --cases 1`). The evidence holds the `harness`, the failing `seed`, the range (`first_seed`, `seeds`), the panic's `message`, the `trace` (what the replayed seed printed, at most 50 lines, with thread numbers and paths outside the repository removed), whether it `reproduced` alone (a failure that does not repeat is a harness that is not deterministic, and is reported with confidence 0.5), and the `reproduce` command. A root whose build fails is a note, and so is a root with no harness. The gate keys are `conc.dst.failures` and `harnesses`.
