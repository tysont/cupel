# CUPEL_DECISION_095: Property tests: found by the language, run by the property engine with the run's seed and a case budget, read back as `PropertyResults`

**Date:** 2026-10-16 · **Status:** accepted · **Phase:** P5.4

## Context

Design §5.3: `gen.property` "runs existing property tests (Hypothesis, proptest/quickcheck, fast-check, jqwik) with seed and budget; reports counterexamples minimized". The catalog (§8.3.2) has a `PropertyResults` row — "per property: passed/falsified, minimized counterexample, seed", produced by "property tool" — and the toolchain table (§8.7) names each language's property tools (`property = ["proptest", "quickcheck"]` for Rust, `["hypothesis"]` for Python, `["fast-check"]` for JavaScript and TypeScript, `["testing/quick", "rapid"]` for Go). `FactKind::PropertyResults` exists; its payload does not. `--cases N` ("generative case budget", §6.4) parses and is ignored (P3 follow-up 9).

The design is silent on four things. Which tests are property tests: a check never sees a language, and nothing in the facts says a test case generates its inputs. How a property runs with Cupel's seed and budget: the frameworks run inside the project's own test runner, and none of them takes a seed and a case count from one shared place. How the counterexample comes back. And what the fact holds.

## Options

**Finding property tests.** (a) From the runner's output alone: a failing test whose output carries a framework's falsification report is a falsified property. It finds failures anywhere, but a passing property is indistinguishable from any other passing test, so nothing can say which properties ran or which functions lack one. (b) A new syntactic fact of property cases: a new catalog row and conformance inventory for every language for what is one attribute of a test case. (c) A field on `TestFrames`' test cases, filled by each language's `tests.scm`, which already knows each framework's syntax (decorators, attributes, macros, calls) and attributes captures to their case. A shape change (version 2) and a conformance construct, no new kind.

**Running them with a seed and a budget.** Every framework reads its own settings, not Cupel's: proptest reads `PROPTEST_CASES`, `PROPTEST_RNG_SEED`, and `PROPTEST_DISABLE_FAILURE_PERSISTENCE` from the environment; quickcheck reads `QUICKCHECK_TESTS` and `QUICKCHECK_MAX_TESTS` and takes no seed; Hypothesis reads settings profiles and pytest's `--hypothesis-seed`, and keeps an example database and other files under `.hypothesis/` (relocated by `HYPOTHESIS_STORAGE_DIRECTORY`); fast-check reads `fc.configureGlobal`, a module-level value set only by code running in the test's own module graph (a setup file), and vitest 4 and jest take setup files only from their configuration, not their command line. A property engine can (a) drive each framework itself, bypassing the project's runner, which loses the project's configuration, paths, and transforms; or (b) run the property tests through the baseline's registered runner, as the flake and mutation engines do (ADR-078, ADR-080), adding each framework's settings: environment variables, and, where a framework is configured only from code, a file Cupel writes under `.cupel/` and points the runner at. The runner's `RunOptions` carries environment but no arguments; pointing vitest or jest at a configuration file needs one.

**Counterexamples.** Every framework already shrinks; re-shrinking is theirs to do. Each reports the shrunk input in its failure message in its own words: proptest `minimal failing input: n = 100000`; quickcheck `[quickcheck] TEST FAILED. Arguments: (3195450000)`; Hypothesis `Falsifying example: test_f(` (older) or `Failing test case: test_f(` (6.1xx), the arguments one per line, then `)`; fast-check `Counterexample: [100000]`. `TestResult.output` already keeps each failure's message and output tail.

## Decision

1. **The language says which cases are properties.** `TestCase` gains `property: Option<String>`, the framework that generates the case's inputs; `TestFrames` goes to version 2. `tests.scm` captures `@property.<framework>` on any node; it belongs to the match's case, else the innermost case containing it, like an assertion. Launch patterns: Python `@given(…)` (and `@hypothesis.given`) → `hypothesis`; Rust: each `#[test] fn` inside a `proptest! { … }` block is a case (its body token tree is the case node, its name the identifier after `fn`; `prop_assert*` and `assert*` macros at the body's top level and one level in are its assertions), `proptest!` inside a `#[test]` function and `#[proptest]` → `proptest`, `#[quickcheck]` functions (cases of their own) and `quickcheck(…)` calls → `quickcheck`; JavaScript and TypeScript `fc.assert(…)` and `fc.check(…)` inside a case → `fast-check`; Go `quick.Check(…)` and `quick.CheckEqual(…)` → `testing-quick`. A framework call that fails the test with the counterexample — `fc.assert`, `quick.Check`, `quickcheck(…)`, a `#[quickcheck]` function's result — is also an assertion, idiom `property` (`custom`, `strong`), so `tests.quality` does not read a property test as asserting nothing or only weakly. `conformance/test-frames.toml` gains the construct `property`, mapped in every language's inventory.

2. **`PropertyResults`, version 1** (`cupel_core::PropertyResults`):

   ```text
   PropertyResults {
     runner: ProducerId,          // the baseline's runner, which ran the properties
     seed: u64,                   // the run's seed (`--seed`, else random)
     cases: u32,                  // the case budget each property was given
     frameworks: [String],        // the language's property tools, from the toolchain table's `property` column
     properties: [PropertyResult {  // every property case of TestFrames in scope, by file then line
       file: String, name: String, span: Span,
       framework: String,         // TestCase.property
       test: Option<String>,      // the runner's test ID; null when no reported test is this case
       outcome: passed | falsified | failed | skipped | not-run,
       counterexample: Option<String>,  // the framework's shrunk input, as it printed it
       seeded: bool,              // whether the framework took the seed (quickcheck takes none)
     }],
   }
   ```

   `falsified` is a failure whose output carries the framework's counterexample; `failed` a failure that does not (an error before any case ran, a crash); `not-run` a case no test of the run reported.

3. **`engine.property`** (`cupel-adapt/src/property/`) is a `Producer<PropertyResults>` for `javascript`, `typescript`, `python`, and `rust` — the languages whose toolchain rows name a framework it budgets (Go's `testing/quick` has neither a seed nor shrinking, so Go has no producer and `gen.property` is `unsupported(property-results)` there). It needs `TestResults` (a red baseline refuses the run first, ADR-076, and names the runner) and `TestFrames`, and wants `ProjectInfo`. It runs the property cases in scope once, with the baseline's runner and its own `RunOptions`, narrowed to their test IDs when the runner narrows to IDs, else their files, else the whole suite, and reads each case's outcome from the test that names it (by file and name, as `tests.flaky` locates tests). With no property case in scope it runs nothing.
   - **The budget** is `--cases N`, else by depth: 100 at `quick`, 1 000 at `standard`, 10 000 at `deep`, 100 000 at `exhaustive`. **The seed** is the run's, so `--seed` repeats every property's generation; fast-check gets it modulo 2³¹.
   - **Per framework**: proptest — `PROPTEST_CASES`, `PROPTEST_RNG_SEED`, `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1` (no `proptest-regressions/` written into the tree); quickcheck — `QUICKCHECK_TESTS`, `QUICKCHECK_MAX_TESTS` (ten times the budget), unseeded; Hypothesis (runner `tool.pytest`) — a pytest plugin written to `.cupel/cache/property/python/cupel_property.py`, loaded by `PYTEST_ADDOPTS="-p cupel_property --hypothesis-seed=<seed>"` with its directory prepended to `PYTHONPATH`, which in `pytest_configure` (after the project's conftests) loads a profile derived from the active one with `max_examples` the budget, `derandomize=False`, and `database=None`, and `HYPOTHESIS_STORAGE_DIRECTORY` under `.cupel/cache/property/`; fast-check (runners `tool.vitest`, `tool.jest`) — a setup file under `.cupel/cache/property/` that calls `fc.configureGlobal({...fc.readConfigureGlobal(), numRuns, seed})`, and a configuration file beside it, passed as `--config`, that loads the project's own configuration (vitest: `vitest.config.*`, else `vite.config.*`; jest: `jest.config.{js,mjs,cjs,json}`, else `package.json`'s `jest`) and appends the setup file to its `setupFiles` (vitest) or `setupFilesAfterEnv` (jest), so the project's setup runs first and Cupel's budget wins. A framework whose budget the baseline's runner cannot take (Hypothesis under `unittest`) is an error naming the runners that can.
   - `RunOptions` gains `args`, arguments appended to the runner's command (after those that narrow and vary it).

4. **Counterexamples are the frameworks' own shrunk inputs**, read from the failing test's output by one parser per framework (the four reports above); Cupel never re-shrinks. A property whose explicit settings fix its own case count or seed (`ProptestConfig::with_cases`, `@settings(max_examples=…)`, `fc.assert(p, {numRuns})`) keeps them: the framework's precedence, not Cupel's.

## Consequences

- `gen.property` costs a baseline run plus one property run; under `--changed` it runs only the property tests in changed test files (the property engine sees `TestFrames` of the files in scope), like `tests.flaky`.
- A property case the language's queries do not recognize is not run as a property (its failures still redden the baseline). Not yet recognized: `@fast-check/vitest`'s `test.prop`, `quickcheck! { … }` blocks, `#[test]` functions inside `proptest!` that carry other attributes between `#[test]` and `fn`, and assertions nested two or more blocks deep in a `proptest!` body.
- quickcheck properties are not reproducible by `--seed`; their evidence says `seeded: false`.
- vitest and jest configuration written in TypeScript or as a function of the command is loaded through the wrapper as the runner would load it; jest configuration in TypeScript is not supported (jest needs `ts-node` for it) and the property run reports the configuration error as the case's failure.
- Hypothesis runs under pytest only. A project with its own `PYTEST_ADDOPTS` loses it for the property run.
- Making `proptest!` tests visible as `TestFrames` cases also lets `tests.quality`, `tests.weakening`, and `tests.flaky` see them.
