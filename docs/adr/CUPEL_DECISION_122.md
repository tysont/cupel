# CUPEL_DECISION_122: `gen.symbolic` and `perf.scaling` are deferred: no wrappable tool installs on both CI platforms, and no bench convention gives input sizes

**Date:** 2026-10-18 · **Status:** accepted · **Phase:** P7

## Context

The P7 Definition of Done asks that every check in the catalog (design §5) be implemented or have an ADR explaining its deferral. After P7.1 through P7.7, two §5 rows have neither: `gen.symbolic` (§5.3: "concolic/search-based input generation targeted at surviving mutants and uncovered branches (KLEE/SymCC/Pynguin/EvoSuite where available)", kind `symbolic.input-found`, cost `exhaustive`, wrap) and `perf.scaling` (§5.6: "fit input size vs time; flag complexity-class changes", kind `perf.scaling-worse`, cost `exhaustive`, build). P6's Definition of Done already set `gen.symbolic` aside ("§5.3 except `gen.symbolic`"); nothing recorded why, and `perf.scaling` was never assigned to a step.

The bar every P6 and P7 wrapped tool met: it installs on this machine and on `ubuntu-latest` and `macos-latest` within minutes, pinned, so the matrix goldens run it for real in CI (plan P7 brief; ADR-105 through ADR-120 each record their installs).

## Options

**`gen.symbolic`.** (a) KLEE: runs LLVM bitcode of C/C++; Rust, Go, Python, JavaScript, TypeScript, Java, and Clojure projects would need a bitcode build of their runtime and a harness per project; KLEE itself is a source build or a Docker image, not an install of minutes on macOS. (b) SymCC: a clang plugin for C/C++, same reach. (c) Pynguin (Python): generates unit tests for a module by search; it runs for minutes per module, pins Python 3.10 and specific dependency versions, needs the module importable in a clean environment, and produces tests to review rather than inputs that kill a named mutant, so the `symbolic.input-found` evidence ("an input that kills mutant X") would need the mutation engine run on its output as well. (d) EvoSuite (Java): JVM 8 through 11 only, against the Temurin 21 CI uses (ADR-108). (e) Build the search on Cupel's own property engines (ADR-095) and mutation outcomes (ADR-080): mutant-targeted generation is a research project, not a wrap. No option meets the bar for even one of the seven languages.

**`perf.scaling`.** (a) Fit time against input size from the benchmarks `perf.bench` already runs (ADR-106): Go `testing.B`, criterion, pytest-benchmark, and hyperfine commands report time per benchmark with no notion of an input size; a complexity fit needs several sizes of one workload per run. (b) A Cupel convention for parameterized benchmarks (a size in the benchmark name, `bench_sort_1000`/`bench_sort_10000`, or a `sizes` setting that reruns a command with `CUPEL_SIZE`): a convention projects must adopt before the check says anything, and a fit of two or three points separates classes only for large constant factors. (c) Defer until the P9 dogfood or a user shows a workload whose complexity class regressed and a check would have caught it.

## Decision

1. `gen.symbolic` and `perf.scaling` stay in the catalog (§5) as planned checks and are not implemented in P7. Neither is registered, so `describe matrix` does not list them and the README's generated tables do not name them; the §5 rows are the only place they appear.
2. Either arrives as its own step with its own ADR when a tool meets the install bar (a symbolic or search-based generator for one of the seven languages that installs pinned on both CI runners within minutes and names the input it found) or, for `perf.scaling`, when a parameterized-benchmark convention is chosen with a user who needs it. P9 (dogfood) is the earliest point at which the need is measured.

## Consequences

- The P7 Definition of Done holds: every §5 check is implemented (28 registered) or deferred here (2).
- `gen.fuzz` (ADR-107), `gen.property` (ADR-095), and `tests.mutation` (ADR-080) cover the ground `gen.symbolic` would target, without the mutant-directed search; `perf.bench` (ADR-106) catches a slowdown at the sizes a project's benchmarks use, not a change of complexity class.
- Design §5's two rows are unchanged; §5.8's `exhaustive` depth lists both as members, which is where they will run once implemented.
