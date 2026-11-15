<!-- ABOUTME: Property tests, differential testing, metamorphic relations, and fuzzing. -->
<!-- ABOUTME: Part of Cupel's guide, linked from the README. -->
# Generative checks

## Property tests

`gen.property` finds the property tests among the tests in scope (Hypothesis's `@given`, each test in a `proptest!` block, `proptest!` inside a test, `#[proptest]`, `#[quickcheck]` and `quickcheck(…)`, and fast-check's `fc.assert` and `fc.check`), runs them once more after the baseline with the same runner, and reads back what each framework reports. Every framework keeps its own shrinking; the counterexample in a finding is the framework's minimized input, as it printed it.

Each framework gets the run's `--seed` and the case budget `--cases` (default 1 000 at `--depth standard`, 10 000 at `deep`, 100 000 at `exhaustive`) its own way, and writes nothing outside `.cupel/` while it runs:

| Framework | Runners | How the budget reaches it |
|---|---|---|
| proptest | `cargo test`, `cargo nextest` | `PROPTEST_CASES`, `PROPTEST_RNG_SEED`, and `PROPTEST_DISABLE_FAILURE_PERSISTENCE` (no `proptest-regressions/` files) |
| quickcheck | `cargo test`, `cargo nextest` | `QUICKCHECK_TESTS`; quickcheck takes no seed, so its findings say `seed: null` |
| Hypothesis | `pytest` | A plugin Cupel writes under `.cupel/` loads a settings profile with `max_examples` the budget, unpinned and without an example database, after the project's conftests; the seed is `--hypothesis-seed`, and Hypothesis's files stay under `.cupel/` |
| fast-check | `vitest`, `jest` | A setup file under `.cupel/` calls `fc.configureGlobal` with `numRuns` and `seed`; a configuration file beside it, passed as `--config`, loads the project's own vitest or jest configuration and appends that setup file to its setup files |

A property whose own code fixes its count or seed (`ProptestConfig::with_cases`, `@settings(max_examples=…)`, `fc.assert(p, { numRuns })`) keeps them: `--cases` applies only to the properties that set no count. Each one that sets its count is `property.cases-pinned` (`info`), naming the test and the count (null when a variable or an expression gives it), and a falsified one's `evidence.cases` is the count it ran (ADR-183). Cupel reads the count from the test's source: fast-check's `numRuns` in the test, Hypothesis's `max_examples` in the test or its decorators, proptest's `with_cases` or `cases` in the `#![proptest_config(…)]` of the test's `proptest!` block or its `#[proptest(…)]` attribute, and quickcheck's `QuickCheck::new().tests(…)`; a count set elsewhere (a shared settings object, a Hypothesis profile, a `fc.assert` parameters variable) runs as set but is not reported. A project whose everyday suite runs few cases with a pinned seed is the usual case, and the one `gen.property` exists for: on the fixtures, the suite's ten pinned cases pass and Cupel's thousand find the bug:

```
$ cupel check --only gen.property --seed 3 --format text
cupel check: pass (exit 0)

checks
  gen.property  fail     tier 4 · standard · score 1 falsified (n 1) · metrics falsified 1, missing 1, properties 1 · 2 findings · 1027 ms

fail (1)
  gen.property
    `test_digits_counts_decimal_places` is falsified by `n=100000`
      fnd_2341b0 · property.falsified · tier 4 · confidence 0.95
      at tests/test_digits.py:7:1-8:36
      evidence: cases 1000 · counterexample "n=100000" · framework "hypothesis" · runner "tool.pytest" · seed 3 · test "tests/test_digits.py::test_digits_counts…" (full: cupel explain fnd_2341b0)
      repro: cupel check --only gen.property --paths tests/test_digits.py --seed 3 --cases 1000

info (1)
  gen.property
    `units.percent.clamp_percent` has no property test
      fnd_7307dd · property.missing · tier 4 · confidence 0.5
      at src/units/percent.py:1:1-3:35
        nearest_test_file: tests/test_percent.py
```

With no gate the run passes (exit 0) while the check fails; a `[gate]` rule such as `"gen.property.falsified" = "== 0"` makes it exit 1.

`property.falsified` gives the framework, the runner, the counterexample, the seed, and the case budget as evidence, and a `repro` that runs the same property with the same seed and budget. A property that failed without a counterexample (it errored before any case ran) is reported the same way with `counterexample: null` and lower confidence.

`--scaffold` turns each `property.missing` into a skipped stub that names the function, generates one input per parameter (typed by the function's signature where the language's table maps the type), and calls it, for you to state the property and remove the skip marker: a Hypothesis test appended to the source's nearest test file in Python, a fast-check test in JavaScript and TypeScript (imported the way the file imports), and an ignored proptest test in an inline `#[cfg(test)]` module of the function's own file in Rust, where a parameter type the table does not map (a struct, a generic) gets no stub. Without `--write` nothing is written and `plan.would_write` holds each file's diff; with it the stubs are written, a file's header lines once, and a function already stubbed is left alone.

```sh
cupel check --only gen.property --scaffold          # preview the stubs
cupel check --only gen.property --scaffold --write  # write them
```

## Differential testing

`gen.differential` compares functions with an [oracle](usage.md#cupel-oracle) on generated inputs. You name the functions, per language, as `<file>:<function>`; the file is relative to the repository root:

```toml
[oracles.release]
kind = "ref"
ref = "v1"

[lang.python]
differential_entries = ["shipping.py:shipping"]
```

For each entry in scope, the differential engine reads the function's parameter types from its signature, generates inputs from them (boundary values first: 0, 1, -1, the type's extremes, empty strings and lists; then random values that grow in size), and calls the function on every input in the working tree and through the oracle. A `ref` oracle runs the same function at the commit its ref names, exported into a temporary directory; a `command` oracle runs once per input with `{input}` replaced by the argument (a string's text, any other value's JSON) or, for several parameters, the JSON array of them, and its standard output is its answer; a command oracle's known inputs (`--input`) are compared first. The first input on which the answers differ is shrunk step by step (integers toward 0, strings and lists shorter) while the answers still differ, so the finding shows the smallest such input. Nothing is written in the working tree.

```console
$ cupel oracle verify release
$ cupel check --only gen.differential --seed 7 --format text
cupel check: pass (exit 0)

summary   1 finding · fail 1 · warn 0 · info 0 · suppressed 0 · new and resolved need a baseline (`cupel baseline set`)

checks
  gen.differential  fail     tier 4 · deep · score 1 divergent (n 1) · metrics cases 100, divergences 1 · 1 finding · 5993 ms · oracle release

fail (1)
  gen.differential
    `shipping(20001)` returned `2500`, but oracle `release` returned `2600`
      fnd_fb42fe · diff.divergence · tier 4 · confidence 1
      at shipping.py:1:1-3:52 in shipping.shipping (python, sh_d5d28ffa7e1a1f68)
      …
      action fix_bug (priority 2): Decide which answer is right for `shipping(20001)`: fix the implementation, or, if the change is intended, the oracle.
      repro: cupel check --only gen.differential --symbols sh_d5d28ffa7e1a1f68 --oracle release --seed 7 --cases 100
```

The change capped the cost at 2500; the boundary input `9223372036854775807` diverged first and shrank in 53 steps to `20001`. The finding's evidence holds the `entry`, the `oracle` (`name`, `kind`, and a ref's `commit`), the minimized `input`, the input as `found`, its `case` index, the `shrinks`, and both answers, `implementation` and `reference`, each one of `{"returned": …}`, `{"raised": …}`, or `{"crashed": …}`. The repro regenerates the same inputs: the seed decides them, and `--cases` sets how many are generated per entry (100 by default). The gate keys are `gen.differential.divergences` (entries that diverged) and `cases` (inputs compared); when no entry could be compared, the check passes with no score and no metrics, and its notes say why.

| Language | How it is called | Parameter types it generates |
|---|---|---|
| Python | `python3 -B` imports the module and calls the function; answers are JSON (or `repr`), exceptions `raised` | `int`, `float`, `bool`, `str`, `list[T]`, `Optional[T]` / `T \| None`; unannotated parameters get mixed values |
| JavaScript, TypeScript | `node` imports the file (TypeScript by Node's type stripping, Node 22.18 or later) and calls the named export | `number`, `string`, `boolean`, `T[]`, `T \| null`; JavaScript's untyped parameters get mixed values |
| Go | an internal test file laid over the package with `go test -overlay`, built once | integer types, `float32`, `float64`, `bool`, `string`, `[]T` |
| Rust | a separate crate that depends on the entry's crate by path, built once into `.cupel/cache/differential/` | integer types, `f32`, `f64`, `bool`, `String`, `&str`, `Vec<T>`, `&[T]`, `Option<T>` |

An entry is called with whatever side effects it has, so name functions that are safe to call with any argument. It must be a module-level, public function (in Rust, one reachable from the crate root): an entry that is missing, private, a method, or has a parameter of another type is not compared, and the result's notes say why. Without `differential_entries` a language is skipped as `unconfigured`; without its program (`python3`, `node`, `go`, `cargo`) it is `unavailable`. A command answers in text, so against a command oracle a returned string matches the same text unquoted, and any error matches any error; against a `ref` oracle answers match when they are equal as text or as JSON.
## Metamorphic relations

A metamorphic relation says how a function's output changes when its input does, without saying what the output is: sorting a list and sorting its reverse give the same list. `gen.metamorphic` checks relations you declare in `cupel.toml`, per language, under `[lang.<id>.relations.<name>]`. Python is the language with a harness today.

```toml
[lang.python.relations.sort-reverse]
function = "sorting:sort_list"          # module:name, imported from the repository root
input = "list[int]"                     # bool, int, str, list[T], tuple[T, ...]
transform = "lambda xs: list(reversed(xs))"   # the follow-up input, from the input

[lang.python.relations.add-commutes]
function = "arith:add"                  # a tuple input is spread into the arguments: add(a, b)
input = "tuple[int, int]"
transform = "lambda pair: (pair[1], pair[0])"
relation = "lambda a, b: a == b"        # how the two outputs relate; default ==
cases = 50                              # inputs to generate; default 100
```

`transform` and `relation` are Python expressions evaluated in the function's module. For every input the harness calls the function on it, the transform on it, and the function on the transform's result, each on its own copy; the relation is violated when it returns false or when any of those calls raises. A relation whose function cannot be imported, or whose expressions do not evaluate to callables, makes the check `error` with the reason in its `notes`; a key that is not one of these five, a missing one, or an input type outside the list is a usage error (exit 2) when `cupel.toml` loads.

Inputs are generated from `--seed`, the relation's name, and the case number alone, starting small (lists of at most one element, integers within ±16) and growing to lists and strings of up to eight elements and integers within ±1024. `--cases N` sets how many inputs every relation gets, over its `cases`. The first input that breaks a relation is minimized: integers move toward zero, lists and strings shrink and their elements get simpler, as long as the relation still breaks. The finding's evidence has the minimized input, the transformed input, both outputs (each `{"value": …}`, `{"raised": "Type: message"}`, or `{"repr": …}` for a value JSON cannot hold), the generated input it started from, and the seed; its `repro` reruns the same inputs. A relation is one finding however many inputs break it, located at the function's definition. In the fixture, `sort_list` makes a single bubble pass:

```text
$ cupel check --only gen.metamorphic --seed 3 --format text
checks
  gen.metamorphic  fail     tier 4 · deep · score 1 violated (n 2) · metrics relations 2, violated 1 · 1 finding · 1042 ms

fail (1)
  gen.metamorphic
    `sorting:sort_list` breaks the relation `sort-reverse`
      fnd_30a135 · metamorphic.violated · tier 4 · confidence 1
      at sorting.py:1-7
      evidence: case 11 · followup_output {1} · function "sorting:sort_list" · input [3] · input_type "list[int]" · original_input [3] · relation "sort-reverse" · seed 3 · shrinks 10 · source_output {1} · transformed {1} (full: cupel explain fnd_30a135)
      action fix_bug (priority 1): Fix `sorting:sort_list` so that `sort-reverse` holds for the input [0,1,1]; the evidence has the follow-up input and both outputs.
      repro: cupel check --only gen.metamorphic --seed 3
```

The minimized input `[0, 1, 1]` sorts to `[0, 1, 1]`, its reverse `[1, 1, 0]` to `[1, 0, 1]`. A failing check does not fail the run by itself; a gate rule such as `"gen.metamorphic.violated" = "== 0"` does. Without any relation declared for a language, the check is skipped there as `unconfigured` (`lang.python.relations`); the support matrix above, which knows no repository's configuration, shows Python that way. The other four languages have no harness yet, so the check is `unsupported` for them. Every relation is evaluated whatever the scope, and only those whose function is defined in a file in scope are reported.

## Fuzzing

`gen.fuzz` fuzzes every target in each project root that holds a file in scope, each with the same execution budget: `--cases`, else 1 000 inputs at `--depth standard` (with `--only`), 10 000 at `deep`, and 100 000 at `exhaustive`, each input limited to 10 s and each target to 300 s.

| Language | Fuzzer | Targets | Ready when |
|---|---|---|---|
| Rust | cargo-fuzz (libFuzzer) | the binaries of a `fuzz/Cargo.toml` with `cargo-fuzz = true`, built with `cargo +nightly fuzz build` and run directly | `cargo-fuzz` and a nightly toolchain are installed |
| Go | Go's native fuzzer | `FuzzXxx` functions, run with `go test -fuzz=^<Name>$ -fuzztime=<cases>x` | `go` is installed |
| JavaScript | Jazzer.js | `*.fuzz.js` modules exporting `fuzz` | `jazzer` is in `node_modules/.bin` or on `PATH` |
| Python | atheris | `fuzz_*.py` scripts that call `atheris.Fuzz(` | `python3` imports `atheris` |

libFuzzer-based fuzzers get the run's `--seed`, so `--seed` and `--cases` repeat a run on one toolchain; Go's fuzzer takes no seed. Nothing is written in the tree: each target's corpus and saved inputs live in `.cupel/cache/fuzz/` and are emptied before each run (a corpus the project commits is read as well), and Go, which saves a failing input into the package's `testdata/`, fuzzes a copy of the module. atheris builds only against a clang that ships libFuzzer, so on macOS with Apple's clang the Python pair is `unavailable`; TypeScript has no producer (Jazzer.js loads only `.js` modules) and is `unsupported`. On the Rust fixture, whose `first` target indexes its input:

```text
$ cupel check --only gen.fuzz --format text
fail (1)
  gen.fuzz
    fuzz target `first` crashes on the empty input: index out of bounds: the len is 0 but the index is 0
      fnd_d59cd3 · fuzz.crash · tier 4 · confidence 1
      at src/lib.rs:5:5-5
      action fix_bug (priority 1): Fix `first`'s failure on the saved input (`cargo +nightly fuzz run first .cupel/cache/fuzz/rust/root/first/inputs/crash-da39a3ee5e6b4b0d3255bfef95601890afd80709` reruns it), then keep the input in the target's corpus as a regression case.
      repro: cupel check --only gen.fuzz --paths fuzz/fuzz_targets/first.rs --seed 3 --cases 1000
```

The evidence holds the `target`, the `tool`, the `message`, the `input` (as text, or `hex:` and its bytes), the `reproducer` file and the `reproduce` command (for Go, the commands that make the input a regression case under `testdata/fuzz/`), the `seed`, and the `cases`. An input that runs past the per-input limit is `fuzz.timeout` (`warn`, confidence 0.7: a slow path under load can cross it). A target that does not build is a note. The gate keys are `gen.fuzz.crashes`, `timeouts`, `targets`, and `missing`.
