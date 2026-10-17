# CUPEL_DECISION_093: The differential engine: `DivergenceResults`, entry points, generated inputs, harnesses, and minimization

**Date:** 2026-10-16 · **Status:** accepted · **Phase:** P5.3

## Context

Design §5.3: `gen.differential` compares "old vs new (or impl vs registered oracle) on generated inputs", emitting `diff.divergence` "with minimized input, both outputs"; it is "build harness + wrap generators", cost `deep`, tier 4 with a verified oracle. The fact catalog (§8.3.2) already names the kind: `DivergenceResults`, "per generated input: both outputs, minimized", made by a "differential engine". §8.3.1 puts every execution in a producer and keeps checks to computing over facts. ADR-090 defines the oracles (`ref`, `command`, `spec`) and leaves "same output as the implementation on N known inputs" to this step; ADR-091 has the run choose one oracle and caps an unverified one's results.

The design is silent on what "the implementation" is and how it is called, where that is declared, what the inputs are and how a language's types reach a generator, how `--seed` and `--cases` reach the generator, how the old side of a `ref` oracle is built and run, what counts as the same output, how a divergence is minimized, what the fact holds, and how a producer learns the oracle at all: producers see `[lang.<id>]`, never `[checks.*]` or `[oracles.*]`.

## Options

**Where the comparison runs.** The check cannot run a process (§8.3.1). A check plugin could, but would put the harness outside the binary and outside the producer contract the scheduler accounts for. An engine producer of `DivergenceResults`, as the catalog says, keeps `gen.differential` a pure function of a fact, and lets a later producer (a PIT-style wrapper of an existing differential tool) feed the same check.

**How the engine learns the oracle.** (a) From `[lang.<id>]`: the oracle's home is `[oracles]` (ADR-090), so it would be defined twice. (b) As an upstream fact: an "oracle" fact kind would be a catalog row for a value the run chooses, not a fact about the code. (c) As a run input on `ProduceCtx`, beside the seed (ADR-078) and the focus (ADR-080): the run's oracle is chosen once per run (`--oracle`, ADR-091 §2), exactly like the seed. (c).

**What the implementation is.** (a) A command (`[lang.<id>] differential_command`): language-neutral, but nothing ties it to the code in scope, inputs are untyped strings, and old-vs-new needs the command to run against the old tree too. (b) Every function in scope: zero configuration, but calling arbitrary functions with arbitrary arguments runs whatever side effects they have (deleting files, network calls). (c) Functions named in configuration, called in the language's own runtime by a generated harness with arguments generated from their signatures: the user says which functions are safe to call and worth comparing; the language's types decide what to generate.

**Where entry points are declared.** `[checks."gen.differential"]` is check configuration, which producers never read; on the oracle, a `ref` oracle compares the whole code at a commit, not one function, and every entry would change the oracle's identity; `[lang.<id>]` is the producer configuration of the language whose files hold the functions. `[lang.<id>]`.

**Types.** Declared in configuration beside each entry, or derived from `ApiSurface` signatures, which every language producer already emits normalized (`(_: i64) -> i64`, `(_ []int64, _ int64) int64`, `(_: number[]): string`). Derived: nothing to keep in step with the code. Untyped parameters (JavaScript, unannotated Python) get a generator of mixed JSON values.

**The old side of a `ref` oracle.** `git worktree add` registers state inside the repository; `BaseTree` (ADR-056) holds only the changed files, not a tree that runs; `git archive <commit>` extracted into a temporary directory is a full, side-effect-free copy.

**Minimization.** Integrated shrinking (Hypothesis-style choice sequences) needs a generator per type that records its choices; greedy shrinking on the generated values themselves (try every one-step-smaller input, keep the first that still diverges, repeat) needs only a shrink function per JSON value shape and is what QuickCheck does. Greedy, in batches, so each round is one harness run per side.

## Decision

1. **`DivergenceResults`** (`divergence-results`, `cupel_core::DivergenceResults`, version 1), an execution fact with no conformance inventory (as `TestResults`, `TestReruns`):

   ```text
   DivergenceResults {
     engine: ProducerId,                  // engine.differential
     oracle: Option<{name, kind: ref|command|spec, commit: Option<String>}>,   // null: the run has none
     seed: u64,                           // the run's seed
     cases: u64,                          // generated cases per entry
     entries: [EntryComparison {
       entry: String,                     // as configured: "src/calc.py:quarter"
       symbol: Option<SymbolKey>,         // the function, when it resolved
       compared: u64,                     // inputs both sides answered
       skipped: Option<String>,           // why the entry was not compared
       divergence: Option<Divergence {
         input: [Value],                  // the minimized arguments
         found: [Value],                  // the arguments that first diverged
         case: u64,                       // that input's index among the entry's inputs
         shrinks: u32,                    // minimization steps taken
         implementation: Answer,          // what the working tree answered on `input`
         oracle: Answer,                  // what the oracle answered on `input`
       }>,
     }],
   }
   Answer = {"returned": text} | {"raised": text} | {"crashed": text}
   ```

2. **The run's oracle reaches producers.** `ProduceCtx` gains `oracle: Option<RunOracle>` (`{name, definition: OracleConfig}`), and `Focus` gains `cases: Option<u64>` (`--cases`); `Job` carries both from the CLI, `ProduceCtx::new` has neither. The seed is `ProduceCtx::seed` (ADR-078).

3. **`engine.differential`** (`cupel-adapt/src/differential/`): one `Producer<DivergenceResults>` per supported language — `python`, `javascript`, `typescript`, `go`, `rust` — each needing `Symbols` and `ApiSurface`, cost `deep`. Availability: `unconfigured` with key `lang.<id>.differential_entries` until that list is set; else `unavailable` without the language's program (`python3`, `node`, `go`, `cargo`); else ready.

4. **Entry points** are `[lang.<id>] differential_entries = ["<file>:<function>", …]`, the file relative to the repository root. An entry is compared when its file is in scope (and, under `--symbols`, its function is selected). It resolves to the `Symbols` function of that name in that file and the `ApiSurface` item at its line, whose signature gives the parameter types; it is recorded `skipped` when the function is missing, is not public (the harness calls what a module exports), or has a parameter type no generator covers.

5. **Types and generators.** Each language maps its signature text to one neutral model: integers with their range (`int`, `i8`…`u64`, `int64`, `uint8`, …), JavaScript numbers (mostly integers within ±2^53, some halves), floats, booleans, strings, lists, optionals (`Optional[T]`, `T | None`, `Option<T>`), and `any` for an untyped parameter (integers, strings, booleans, null, lists of integers). Anything else (maps, objects, pointers, generics, `&mut`) skips the entry. Inputs per entry: a command oracle's known `inputs` first (decoded as below), then boundary cases (each parameter's `0`, `1`, `-1`, extremes of its range, `""`, `[]`, `null`, …, by index), then random cases whose size grows with the case index, `cases` of them in all after the known inputs (`--cases`, default 100). The stream is SplitMix64 seeded by BLAKE3 of the run seed and the entry, so `--seed` reproduces it and adding an entry changes no other entry's inputs. Values never include NaN or infinities (JSON cannot carry them).

6. **Harnesses.** Each side of a comparison runs one process per batch of inputs: inputs are written as JSON lines to a file named by `CUPEL_DIFFERENTIAL_IN`, answers read from `CUPEL_DIFFERENTIAL_OUT` (so whatever the function prints is ignored), and the harness writes `{"ready": true}` once the function is loaded — a run without it is a harness failure, which skips the entry with the harness's standard error. A process that ends before answering every input marks the first unanswered input `crashed` (its exit, or `timed out` after 60 s per batch) and runs again on the rest.
   - **Python:** `python3 -B` runs a driver that puts the module's import root on `sys.path` (the file's directory less the module path `ApiSurface` names), imports the module, and calls the function; answers are canonical JSON (`sort_keys`), else `repr`; an exception is `raised` as `Type: message`.
   - **JavaScript and TypeScript:** `node` runs a driver that `import()`s the file (TypeScript by Node's type stripping, Node ≥ 22.18) and calls the named export, awaiting a promise; answers are `JSON.stringify`, else `String(value)` (`undefined`, `NaN`, bigints).
   - **Go:** a generated internal test file (`package` of the entry's file) is laid over the package with `go test -overlay`, so nothing is written in the tree; it decodes each argument with `encoding/json` into the parameter's declared type, calls the function, assigns every result (a non-nil `error` becomes its message), and answers the JSON of the one result or of the list of results; a panic is `raised`. `go test -c -vet=off` builds it once per side; the binary runs each batch.
   - **Rust:** a generated binary crate, with an empty `[workspace]` and a path dependency (renamed `entry`) on the crate whose `Cargo.toml` is nearest the entry's file, decodes each argument with a small built-in JSON reader into the parameter's type (`&str` and `&[T]` through an owned `String` and `Vec<T>`), calls `entry::<module path>::<function>`, and answers its `Debug` text; a panic (caught with `catch_unwind`, the hook silenced) is `raised`. Only public functions reachable from the crate root can be called. `cargo build` runs once per side in the entry crate's directory (so its toolchain file applies), with the user's `Cargo.lock` copied beside the harness and `CARGO_TARGET_DIR` under `.cupel/cache/differential/`.

7. **Sides.** The implementation is the working tree. A `ref` oracle's side is the commit its ref resolves to (`git rev-parse`), exported with `git archive` into a temporary directory removed after the run, running the same harness there; an entry the commit's harness cannot load (the function did not exist, or its signature differed) is `skipped` with that reason. A `command` oracle runs `sh -c` in the repository root once per input, `{input}` replaced by the shell-quoted input as `oracle verify` does (ADR-090 §3): for a one-parameter function the argument itself (a string's text, any other value's JSON), else the JSON array of the arguments; a known input is decoded the same way in reverse. Exit 0 answers `returned` with its standard output less one trailing newline; another exit `raised` (`exit N: <stderr tail>`); a signal `crashed`; exit 127 (the program is missing) is an error for the whole fact. A `spec` oracle compares nothing: the fact names it, with no entries (ADR-094).

8. **The same output.** Two answers agree when they are the same kind and, for `returned`, their texts are equal or parse as equal JSON values (object keys in any order). Against a command oracle, which speaks text, a returned JSON string also agrees with the same text unquoted, and any `raised` or `crashed` agrees with any other: an error is an error in two different programs. Against a `ref` oracle, a `raised` text must be equal.

9. **Minimization.** The first input that diverges (in input order) is minimized greedily: each round lists every one-step shrink of the current input, argument by argument (an integer toward 0 — to 0, then ever closer to itself by halves (`n - n/2`, `n - n/4`, …, `n - 1`) —, a float to 0 or its integer part, a string to empty, half, or one character shorter, a list to empty, half, one element shorter, or one element shrunk, `true` to `false`), runs them as one batch on each side (a command oracle stops at the first disagreement), and keeps the first candidate that still diverges, until a round keeps none or 100 rounds pass. An entry stops at its first divergence: one minimized divergence per entry.

## Consequences

- Differential testing needs no harness code from the user, only `differential_entries`; it calls functions in-process with their own types, so a divergence is reported in the language's own terms (`Debug` for Rust, JSON elsewhere).
- Entry points are module-level functions with simple parameter types. Methods, functions taking structs, maps, or callbacks, and generic functions are skipped with a reason; a declared argument model (or generators per user type) is the extension when a real repository needs it.
- The harness calls whatever the function does: side effects are the user's to avoid by choosing entries. The working-tree side runs in the repository (Python with `-B`, Go through an overlay, Rust from a separate crate) and writes nothing in the tree; Rust builds into `.cupel/cache/differential/`.
- A command oracle's known `inputs` become the first inputs of every comparison against it, so "same output on N known inputs" (ADR-090 consequences) is part of each `gen.differential` run rather than of `oracle verify`, which stays the oracle's own smoke test (ADR-094 §6).
- Python is run as `python3` and JavaScript as `node` from `PATH`; a launcher (`uv run`, a virtual environment) and Node's module resolution of extensionless TypeScript imports are not handled. The old tree of a `ref` oracle has no ignored files (`node_modules`, virtual environments), so an entry whose module imports installed packages may fail to load there.
- `gen.metamorphic` (P5.5) is listed as a second consumer of this kind in the design's catalog; relations over one implementation are not oracle comparisons, and a version 2 may add what they need.
