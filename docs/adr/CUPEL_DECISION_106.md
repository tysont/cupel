# CUPEL_DECISION_106: `perf.bench`: the `BenchSamples` payload, bench producers that measure the base commit themselves, and a Mann-Whitney U test in the check

**Date:** 2026-10-17 · **Status:** accepted · **Phase:** P6.3

## Context

Design §5.6: `perf.bench` compares "benchmarks vs baseline with statistical test (criterion, benchstat, hyperfine); allocation/syscall counts preferred to wall time", kinds `perf.regression` and `perf.improvement`, cost `deep`, wrap. The catalog (§8.3.2) has a `BenchSamples` row, "per benchmark: samples, unit", made by a "bench tool"; the toolchain table (§8.7) names criterion (Rust), `go test -bench` + benchstat (Go), pytest-benchmark and hyperfine (Python), hyperfine and tinybench (JavaScript, TypeScript). Design §6.7 gates on `"perf.bench.regression" = "< 5%"`. Plan P6.3 asks for the statistical comparison against a baseline to be done in the check. `FactKind::BenchSamples` existed with no payload.

Open: what "baseline" is when a check cannot read the store or git (§8.3.1); what the payload holds; what each tool is asked to run and how its samples are read; which statistical test; what the gate metric `regression` means and in which unit; and what happens on a whole tree, where there is nothing to compare with.

## Options

**Where the base side's samples come from.**

- (a) `base_needs = [BenchSamples]` (ADR-056): the scheduler produces the base side's facts on a `BaseTree`, which holds only the range's changed files, so no project can be built or benchmarked there.
- (b) The stored facts of the `--against baseline|run:ID` run, handed to the check by the scheduler. The check cannot read the store (§8.3.4 grants read access in the design, but no check has it and ADR-056 rejected the store for the base side); the scheduler would have to load another run's facts, keyed by another commit, toolchain hash, and machine; such a run rarely exists at exactly the range's base (an agent's loop does not run Cupel on `main` first); and `--against` already means the reference for finding deltas (ADR-046), a different question from "is the code slower than at the base".
- (c) The producer measures the base itself, as `tool.footprint` reads the base's lockfiles (ADR-103 §5): `Focus.base` names the range's base commit; the producer exports it with `git archive` (as the differential engine exports a `ref` oracle, ADR-093 §7) and runs the same tool in the same project roots there. Expensive (every benchmark runs twice), but it is the honest comparison: same machine, same tool, minutes apart.
- (d) (c) with the base side cached by commit. The base would be measured at another time than head, under other load: a systematic bias a significance test cannot see.

**Statistics.** benchstat compares two sets of samples with a two-sided Mann-Whitney U test at α = 0.05 and reports the change of medians; it assumes no distribution, which suits timing data (skewed, with outliers). A t-test assumes normal samples. Wrapping benchstat itself would put the comparison in a producer and its text output in a fact, contradicting the plan; its method is what the check implements.

**The gate metric.** `"perf.bench.regression" = "< 5%"` (design §6.7) compares a metric with 0.05 (the gate reads `%` as hundredths, ADR-060). A count would make `5%` meaningless; a fraction of the base's median makes it read as written.

## Decision

1. **`bench-samples` v1** (`cupel_core::BenchSamples`), providable by producer plugins:

   ```text
   BenchSamples {
     tool: ProducerId,                 // tool.go-bench, tool.criterion, tool.pytest-benchmark, tool.hyperfine
     samples: u32,                     // samples asked of every benchmark on each side
     runs: [RootRun {dir, status: ran|failed, detail}],      // each project root at head
     benchmarks: [Benchmark {name, unit, file?, line?, samples: [f64]}],
     base: Option<BenchBase {commit, runs: [RootRun], benchmarks: [Benchmark]}>,  // null outside a range
   }
   ```

   Every sample is a cost per iteration (lower is better) in `ns` of wall time, `bytes`, or `allocs`; a benchmark is identified on both sides by `(name, unit)`. `RootRun` (`cupel_core::root_run`) is shared with `RaceReports` (ADR-105).

2. **The base side is measured by the producer** (option c, no cache). Under a range (`Focus.base`), each bench producer first exports the base commit into a temporary directory (`cupel_adapt::tree::export`, shared with the differential engine) and measures every project root there, then measures the working tree, so both sides run back to back. An export that fails marks the base side's one run `failed` with the error; head is still measured. Outside a range `base` is null.

3. **Samples per side** by depth: 5 at `quick` and `standard` (the fewest for which a complete separation is significant: 2/C(10,5) ≈ 0.008), 10 at `deep`, 20 at `exhaustive`. criterion takes at least its own minimum of 10.

4. **Producers** (`cupel-adapt/src/bench/`, one module per tool on a shared `BenchTool` spec; cost `deep`; per tool, in every project root of its language that `ProjectInfo` names (none: nothing is measured), except hyperfine, which runs its commands once from the repository root; nothing written in the tree: reports, tool storage, and build directories go under `.cupel/cache/bench/<tool>/`):

| Producer | Languages | Invocation (per project root unless noted) | Samples |
|---|---|---|---|
| `tool.go-bench` | go | `go test -run=^$ -bench=. -benchmem -count=<n> -benchtime=100ms -json <packages>` | each result line: `ns/op` → `ns`, `B/op` → `bytes`, `allocs/op` → `allocs`; name `<package>.<Benchmark…>` without the `-N` GOMAXPROCS suffix; placed at the one `_test.go` file under the root with `func <Benchmark>(` |
| `tool.criterion` | rust | `cargo metadata --no-deps --offline`, then for each bench target of a package depending on criterion `cargo bench --bench <t> -- --noplot --sample-size <max(10,n)> --warm-up-time 0.1 --measurement-time 0.5` with `CRITERION_HOME` and `CARGO_TARGET_DIR` under the cache | each `<home>/**/new/sample.json` entry `times[i] / iters[i]` ns, named by `benchmark.json`'s `full_id` |
| `tool.pytest-benchmark` | python | `python3 -m pytest -p no:cacheprovider --benchmark-only --benchmark-json=<out> --benchmark-min-rounds=<n> --benchmark-max-time=0.000001 --benchmark-warmup=off --benchmark-storage=file://<cache>`, `PYTHONDONTWRITEBYTECODE=1` | the first `n` of `stats.data` (seconds → ns), named by the node ID from the repository root, placed at its `def` line |
| `tool.hyperfine` | javascript, typescript, python | `hyperfine --runs <n> --warmup 1 --style none --export-json <out> <commands>` once, from the repository root (the export's root on the base side) | each command's `times` (seconds → ns), named by the command |

   Availability: `go` / `cargo` / `hyperfine` on `PATH`, else missing with the toolchain row's install command or the `[toolchains]` hint; pytest-benchmark is ready when `python3` imports `pytest_benchmark`, else missing (`<add> pytest-benchmark`); hyperfine is `unconfigured` (`lang.<id>.bench_commands`) until `[lang.<id>] bench_commands` lists commands. Detection (ADR-076): criterion and pytest-benchmark where the project mentions them, hyperfine where `cupel.toml` contains `bench_commands`; in Python pytest-benchmark is ahead of hyperfine. A run that does not exit 0 is `failed` with its output's tail, keeping whatever it measured. benchstat is not wrapped (above); tinybench has no machine-readable sample output Cupel reads and is not wrapped.

5. **The test** (in the check, `perf_bench/mann_whitney.rs`): two-sided Mann-Whitney U on the two sides' finite samples, ties given midranks. Exact — counting every assignment of the pooled ranks to the base sample whose rank sum is at least as far from its mean as the observed one — while there are at most 100 000 assignments (C(n₁+n₂, n₁); 9 + 9 samples is 48 620); beyond, the normal approximation with the tie-corrected variance and a continuity correction of ½. All values equal gives p = 1. The change is `(median_head − median_base) / median_base`; a base median of 0 counts any growth as 1 (100%) and none as 0.

6. **`perf.bench`** (`cupel-checks/src/perf_bench.rs`): tier 0 (a non-functional measurement, as `perf.footprint`, not evidence of correctness), cost `deep`, needs `BenchSamples`, stability `experimental`. Settings (`[checks."perf.bench"]`, integers): `threshold_percent` (5) — the smallest change a finding reports — and `significance_percent` (5) — α.
   - **`perf.regression`** (`warn`, action `review`) for a benchmark measured on both sides whose p < α and change ≥ threshold; **`perf.improvement`** (`info`, `review`) for change ≤ −threshold. Evidence `{benchmark, unit, base_median, head_median, change, p_value, base_samples, head_samples, significance, threshold}`; ID subject `<name>|<unit>`; located at the benchmark's file and line when known; confidence 1 − p; title `` `name` regressed 83.3% in allocs (p = 0.0079) ``; repro `cupel check --only perf.bench --base <commit>`.
   - **Metrics** (gate keys `perf.bench.<metric>`): `benchmarks` (measured at head), `compared` (on both sides), `regression` (the largest significant slowdown as a fraction of the base median, whatever the threshold; 0 when none or without a base), `regressed`, `improved`. Across languages `regression` is the largest language's, the counts add up. Score `{value: regression, unit: "regression", n: compared}`, combined by maximum. Status `fail` when anything regressed.
   - **Notes**: no base (`no base to compare with: run under --changed or --base <commit>`); each failed run, per side (`<tool> failed in <dir>[ at the base commit <c>]: <detail>`); each benchmark measured on one side only.

## Consequences

- `perf.bench` costs two benchmark runs per language under a range and one on a whole tree, where it reports what it measured and that nothing was compared; its matrix goldens (tree runs) therefore hold no timing. Under `--changed` a language is benchmarked only when one of its source files changed (as every check), and every benchmark of its roots runs, wherever the change was.
- Allocation counts (Go's `-benchmem`) are deterministic, so an allocation regression is found on every run; wall time is noisy, and a loaded machine can make a small change significant or hide it. The threshold keeps sub-5% wobbles out of findings; the `regression` metric still records them for a gate that wants a tighter bound.
- The base side is an export of the commit: ignored files (a virtual environment, `node_modules`, build directories) are absent there, so a benchmark that needs them fails on the base side only, which a note says. Cargo rebuilds the base side in a build directory of its own.
- hyperfine needs the project to name its commands; JavaScript and TypeScript have no other bench producer, so `perf.bench` is `unconfigured` for them until `bench_commands` is set. TypeScript commands run under the project's own launcher (`node bench.ts` uses Node's type stripping).
- A Go benchmark name ending in `-<digits>` run with GOMAXPROCS 1 (when Go adds no suffix) loses that ending; criterion and hyperfine benchmarks are not placed at a line.
- Daemon parity (ADR-085 §11) cannot hold for an output of wall-time measurements: a second run measures again. The history tests that compare benchmarks run cold only (`cupel_fixtures::run_cupel_measuring`); `perf.bench` goes through the daemon in its matrix goldens, which hold no timing.
- The bench producers do not compare; another tool (a plugin wrapping a JMH or BenchmarkDotNet run) feeds the same check with samples.
