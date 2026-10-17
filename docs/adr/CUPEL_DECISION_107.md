# CUPEL_DECISION_107: Fuzzing: the `FuzzResults` payload, one producer per fuzzer, an execution budget, and `gen.fuzz`

**Date:** 2026-10-17 · **Status:** accepted · **Phase:** P6.3

## Context

Design §5.3: `gen.fuzz` is "coverage-guided fuzzing (cargo-fuzz, libFuzzer, AFL++, Go native, Jazzer) with time budget; reproduces crashes", kinds `fuzz.crash`, `fuzz.timeout`, `fuzz.target-missing`, cost `deep`/`exhaustive`, wrap, tier 4. The catalog (§8.3.2) names `FuzzResults` — "crashes with reproducers, coverage reached", made by a "fuzz tool" — and the toolchain table's `fuzz` column lists `cargo-fuzz` (Rust), `go test -fuzz` (Go), `atheris` (Python), and `jazzer.js` (JavaScript and TypeScript). Plan P6.3 asks for `FuzzResults` producers for those four. The groundwork commit gave the kind its payload type (below) and a schema; the design is silent on what a fuzz target is in each ecosystem, what budget a run gets and how `--seed` and `--cases` reach a fuzzer, where corpora and crash inputs go when a producer may write only under `.cupel/` (and Go writes failing inputs into the package unconditionally), what "target missing" means to a check that cannot see a language, what a timeout is, and how a fixture's golden can hold a fuzzer's output when fuzzing is random and differs between fuzzer and compiler versions.

## Options

**Budget.** (a) Wall-clock time, as the design's "time budget" says: simple to bound, but the same command finds different crashes on a loaded machine and an idle one, so neither a golden nor a `repro` repeats. (b) Executions: libFuzzer's `-runs=N`, Go's `-fuzztime=Nx`; with a seed, libFuzzer's run is then the same run on every machine for one binary. (b), with a wall-clock cap so a slow target still ends, and the executions taken from the case budget the generative checks already share (`Focus::case_budget`: `--cases`, else 100/1 000/10 000/100 000 by depth, ADR-095 §3).

**Where inputs go.** libFuzzer (and so cargo-fuzz, atheris, Jazzer.js) writes new corpus entries into its first corpus directory and crash inputs at `-artifact_prefix`; `cargo fuzz run` adds its own `fuzz/artifacts/<target>/` and `fuzz/corpus/<target>/`. Go writes a failing input into the package's `testdata/fuzz/<Name>/` with no flag to move it. (a) Let the tools write where they write: the user's tree changes on every run (and Go's saved input turns `go test` red until it is fixed). (b) Run libFuzzer binaries directly with a fresh corpus and the artifact prefix under `.cupel/cache/fuzz/`, the project's committed corpus as a second, read-only directory; fuzz Go in a copy of the module under the cache and keep the failing input from there. (b): nothing is written in the tree, and the finding says how to make the input a regression case the ecosystem's way.

**Persistent corpora.** Keeping the cache corpus between runs makes later runs smarter but means `--seed` no longer repeats a run. The corpus is emptied before every run; a project that wants a growing corpus commits one, which the producers read.

**`fuzz.target-missing`.** (a) Public functions that take bytes or text without a target: needs parameter types the check cannot read without a language (ADR-096 stopped at `parameters ≥ 1` for the same reason). (b) A target named in configuration that does not exist: a configuration error, not a finding. (c) A project root the fuzzer searched that has no fuzz target: decidable from the fact, language-neutral, and the honest statement of what was not fuzzed. (c), at `info`.

**Timeouts.** A per-input limit separates a hang from a slow input only roughly; a slow path under load can cross it. `fuzz.timeout` is therefore a `warn` (confidence 0.7), while `fuzz.crash` is a `fail`.

**Fixtures.** A golden compares the fact exactly; a crash found after random exploration, its input, and coverage counts change with the fuzzer's version, the compiler's code generation, and (for Go, which takes no seed) the clock. libFuzzer runs the empty input first, so a target that fails on it fails the same way everywhere; Go's minimizer turns any failing string shorter than two bytes into `"0"`, found within a hundred executions in each of sixteen trials. Executions and coverage stay in the fact and out of evidence and metrics.

## Decision

1. **`fuzz-results` v1** (`cupel_core::FuzzResults`, providable by producer plugins):

```text
FuzzResults {
  tool: ProducerId,                 // tool.cargo-fuzz, tool.go-fuzz, tool.atheris, tool.jazzer
  seed: u64, seeded: bool,          // the run's seed; whether the fuzzer took it (Go's does not)
  cases: u64,                       // executions each target was given
  roots: [String],                  // project roots searched, relative to the repository root
  targets: [FuzzTarget {
    root, name, location: Option<Location>,             // where the target is defined
    status: passed | crashed | timed-out | failed,      // failed: did not build, or stopped without saying why
    detail: Option<String>,                             // failed: the output's tail
    executions: Option<u64>, coverage: Option<u64>,     // as the fuzzer reports them (libFuzzer's `cov`)
    crash: Option<FuzzCrash { message, location: Option<Location>, input, reproducer: Option<String>, reproduce: Option<String> }>,
  }],
}
```

   `input` is the failing input as text when it is printable UTF-8, else `hex:` and its bytes, at most 256 characters; `reproducer` the saved file relative to the repository root; `reproduce` the command that reruns the target on it from its project root.

2. **Producers** (`cupel-adapt/src/fuzz/`), one per fuzzer, each a `Fuzzer` that lists a root's targets and fuzzes one; `FuzzProducer` does the rest for all of them. Each needs `ProjectInfo`, cost `deep`, runs in every project root of its language that holds a file of the scope with the language's extension (every root for an empty scope), and records every root it searched. Each target runs in `.cupel/cache/fuzz/<lang>/<root>/<target>/`, emptied first, with `corpus/` and `inputs/` (saved crash and timeout inputs).

| Producer | Language | Targets | Run | Ready when |
|---|---|---|---|---|
| `tool.cargo-fuzz` | `rust` | the `[[bin]]`s of `fuzz/Cargo.toml` with `[package.metadata] cargo-fuzz = true` (a root that is itself such a crate is not searched) | `cargo +nightly fuzz build <t>` with `CARGO_TARGET_DIR` under the cache, then the libFuzzer binary directly | `cargo-fuzz` on `PATH` and `rustc +nightly` runs; else `cargo install cargo-fuzz --locked` or "install a nightly toolchain" |
| `tool.go-fuzz` | `go` | `FuzzXxx` functions `go test -list ^Fuzz -json <packages>` lists, located by their `func` line in a `_test.go` file | `go test -run=^$ -fuzz=^<Name>$ -fuzztime=<cases>x -parallel=1 -timeout=300s <package> -args -test.fuzzcachedir=<cache>` in a copy of the root (outside `.git`, `.cupel`, `node_modules`, `target`, `__pycache__`, and what `.gitignore` ignores), paths in its output put back to the root's | `go` on `PATH` |
| `tool.atheris` | `python` | `fuzz_*.py` files that call `atheris.Fuzz(` | `python3 <script> <corpora> <flags>` with `PYTHONDONTWRITEBYTECODE=1` | `python3 -c "import atheris"` succeeds; else `python3 -m pip install atheris` |
| `tool.jazzer` | `javascript` | `*.fuzz.js` modules (exporting `fuzz`) | `jazzer <module without .js> <corpora> -- <flags>` | `jazzer` in the root's `node_modules/.bin` or on `PATH`; else `npm install --save-dev @jazzer.js/core` |

   libFuzzer's flags: `-runs=<cases> -seed=<seed> -timeout=10 -max_total_time=300 -artifact_prefix=<inputs>/ -print_final_stats=1`. The seed is the run's when it is 1 to 2³²−1, else its remainder by 2³²−1 plus one (libFuzzer reads 0 as "random"). A run that saved a `timeout-` input timed out, one that saved any other input (`crash-`, `oom-`, `leak-`) crashed, one that saved none passed on exit 0 and failed otherwise; a Go run passed on exit 0, crashed when it printed `--- FAIL:`, and failed otherwise (Go has no per-input limit, so it never times out). Executions come from `stat::number_of_executed_units` (else `Done N runs`, Go's last `execs:`), coverage from libFuzzer's last `cov:`.

   **Messages and places.** Rust: the line after `panicked at <file>:<line>:<col>:`, at that place. Go: the first message under the innermost `--- FAIL:` without its `<file>.go:<n>:` prefix; a `panic:` at the first stack frame in the repository, any other message at the line it names. Python: the line after ` === Uncaught Python exception: ===`, at the innermost traceback frame in the repository. JavaScript: the text after `Uncaught Exception: `, at the first `at …` frame in the repository. Otherwise the fuzzer's or a sanitizer's `ERROR: …` line, with addresses masked; libFuzzer's `timeout after N seconds` becomes `timeout: an input ran past the per-input limit`, since N is the machine's, not the code's. A place outside the repository is left out.

   **Reproduction.** cargo-fuzz: `cargo +nightly fuzz run <t> <input>`; atheris: `python3 <script> <input>`; Jazzer.js: `npx jazzer <module> <input>`; Go: `mkdir -p <pkg>/testdata/fuzz/<Name> && cp <input> <pkg>/testdata/fuzz/<Name>/ && go test -run=<Name>/<hash> ./<pkg>` — Go's own convention, under which the input becomes a regression case of `go test`. A Go input is shown as its value when the corpus file holds one `string("…")` or `[]byte("…")` without escapes, else as its encoded lines.

   **TypeScript** has no producer: Jazzer.js appends `.js` to the module it loads and has no TypeScript loader, so `gen.fuzz` is `unsupported(fuzz-results)` for TypeScript until a producer compiles targets or drives Jazzer.js through Jest.

3. **`gen.fuzz`** (`cupel-checks/src/gen_fuzz.rs`): tier 4, cost `deep`, needs `FuzzResults`, no settings (the budget is `--cases`), stability `experimental`. Findings through `SubjectFinding`:
   - **`fuzz.crash`** (`fail`, confidence 1, `fix_bug`) per crashed target, **`fuzz.timeout`** (`warn`, confidence 0.7, `fix_bug`) per timed-out one; evidence `{target, tool, message, input, reproducer, reproduce, seed, cases}` (`seed` null for a fuzzer that takes none); subject `<root>|<target>`; located at the crash's place, else the target's; title `` fuzz target `<t>` crashes on <input>: <message> `` (`the empty input` for an empty one); the action names the `reproduce` command; repro `cupel check --only gen.fuzz --paths <target file, else root> [--seed <seed>] --cases <cases>`.
   - **`fuzz.target-missing`** (`info`, `write_test`) per searched root with no target; evidence `{root, tool}`; subject the root; no location; repro `cupel check --only gen.fuzz --paths <root>`.
   - Metrics, summed across languages: `targets`, `crashes`, `timeouts`, `missing`; score `{value: crashed + timed-out targets, unit: "failing", n: targets}`; each failed target is a note (`<t> could not be fuzzed: <detail>`); status `fail` with any crash.

## Consequences

- `gen.fuzz` is supported for Rust, Go, and JavaScript where their fuzzers are installed, `unavailable` for Python without atheris (which builds only against a clang that ships libFuzzer, so not with Apple's), and `unsupported` for TypeScript.
- A run is repeatable by `--seed` and `--cases` for the libFuzzer fuzzers on one toolchain; Go's fuzzer is not seedable, so its runs differ, but a minimized input is stable.
- Fuzzing never writes in the tree. The price is a copy of a Go module per target (large `testdata` is copied each time) and a module whose `go.mod` `replace`s a path outside the root fails to build in the copy (`failed`, with Go's message). cargo-fuzz's `fuzz/Cargo.lock` is written by cargo the first time the fuzz crate builds, as cargo always does; the fixture commits it.
- The Rust fixture needs `libfuzzer-sys` from crates.io and a nightly toolchain; CI installs a pinned nightly and cargo-fuzz.
- Fuzzing cost is per target and per root; `--changed` fuzzes every target of a root with a changed file, since a target exercises code across files.
- Not done: AFL++ and libFuzzer harnesses outside cargo-fuzz, Jazzer (JVM) for P6.4's Java, coverage reports per target, minimization of libFuzzer inputs (`-minimize_crash`), and `fuzz.timeout` for Go. The atheris report reader is tested on a hand-written report in the shape atheris documents, not a capture.
