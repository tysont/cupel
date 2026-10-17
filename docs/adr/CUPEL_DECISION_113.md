# CUPEL_DECISION_113: `spec.contracts`: the `ContractViolations` fact, a contract run through the baseline's runner, and one producer per contract library

**Date:** 2026-10-17 · **Status:** accepted · **Phase:** P7.1

## Context

Design §5.5: `spec.contracts` "runtime pre/post/invariant checking enabled during test and generative runs", kind `contract.violated`, cost standard, tier 5, "build (instrumentation hooks) + wrap"; §5.8 runs it at `standard`. The catalog (§8.3.2) names `ContractViolations` among the facts P7 adds, with no row, payload, or producer. Plan P7 asks for its schema and conformance inventory before its first producer, and for each wrapped tool to be a producer whose parser is tested on committed real output.

Open: how contracts are declared in each language and which of them Cupel can recognize; what "enabled during test and generative runs" means given the baseline (ADR-076): a red suite refuses the run with exit 4 and every production of `TestResults` withholds a red fact, so a contract that fails the project's own test run never reaches a check; where the violation is read from; what the fact holds; and what is honestly unsupported.

## Options

**Where violations come from.** (a) The baseline's `TestResults`: its failed tests' output holds every panic and exception. But the baseline is green whenever a check runs, so it holds no violation; a contract the project's own run checks and violates is already reported, as `baseline.test-failed` with the library's message. (b) Instrumenting the code (rewriting functions to check declared conditions, the design's "build (instrumentation hooks)"): a per-language source transformation for each contract syntax, with the schemata machinery's cost and risk, for conditions the libraries already check. (c) A second run of the suite through the baseline's runner, with what the project's own run leaves off turned on, read with the runner's own report parser: the libraries' "debug-only" and "slow" contracts (Rust's `debug_requires` under a profile without debug assertions, icontract's `enabled=icontract.SLOW`), and property tests given the run's case budget, where a contract is the oracle for generated inputs. A failure of that run is information the baseline did not have. (c) reuses the runners (ADR-075), their parsers, and the property engine's budget settings (ADR-095) unchanged.

**Which contracts are recognized.** Rust: the `contracts` crate (`#[requires]`, `#[ensures]`, `#[invariant]`, and `debug_`/`test_` forms) panics with `Pre-condition of <fn> violated: <description>: <condition>` (and `Post-condition`, `Invariant (as pre-condition)`/`(as post-condition)`) at the attribute; plain `assert!`/`debug_assert!` panics are indistinguishable from a test's own assertions. Python: icontract raises `ViolationError: File <path>, line <n> in <scope>:` with the condition and the values it saw, at the decorator, but not which of `require`, `ensure`, `invariant` it was; deal raises `PreContractError`/`PostContractError`/`InvContractError` but names neither the function nor the contract's place; plain `assert` is a test assertion. Go, JavaScript, TypeScript: no runtime contract library in common use; a panic or thrown error says nothing about pre or post. Java: Guava `Preconditions` throw ordinary `IllegalArgumentException`/`IllegalStateException`; JML runtime checking needs the OpenJML compiler. Clojure: `:pre`/`:post` maps are built in, but both throw `AssertionError: Assert failed: <form>`, as `assert` does, so the kind is a guess from `%` in the form.

**Producer granularity.** One engine for every language with a table of message formats, or one producer per library (the plan's "each new tool is one producer"). Libraries differ in what turns them on, which manifests declare them, and whether their report needs the source to classify; one producer each keeps those apart and lets a second library of a language (deal) arrive as its own producer.

**Running when nothing is declared.** A project that does not use the library would pay a whole suite run to learn nothing; Cupel's own dogfood would run its suite once more. The library's package name in a root's manifests says whether to run.

## Decision

1. **`contract-violations` v1** (`cupel_core::ContractViolations`), providable by producer plugins; its row in design §8.3.2:

   ```text
   ContractViolations {
     tool: ProducerId,         // tool.rust-contracts, tool.icontract
     runner: ProducerId,       // the baseline's runner, which ran the contract run
     declared: bool,           // a project root's manifests name the library; false: nothing ran
     runs: [RootRun],          // each root's invocation, ran | failed with its output tail
     tests: u64,               // tests the contract run reported, skipped ones aside
     violations: [ContractViolation {
       kind: precondition | postcondition | invariant,
       function: Option<String>,                 // what the contract is declared on
       message: String,                          // the library's description, condition, values; addresses masked
       place: Option<ContractPlace { file, line }>,   // the attribute or decorator; None outside the repository
       test: String,                             // the runner's test ID
     }],
   }
   ```

   Violations are kept, without exact duplicates, when the contract or the test is in a file in scope, or the contract is outside the repository. The conformance inventory `cupel-fixtures/conformance/contract-violations.toml` lists `precondition`, `postcondition`, and `invariant`; each producer's capture test requires all three on its committed real output (`captures/contracts/<tool>/ledger/`). Execution facts have no per-language fixture inventory, so the conformance test admits this inventory as one checked on captures.

2. **Producers** (`cupel-adapt/src/contracts/`), one `Library` each: language, package name, manifest globs, switches, and a reader of one failed test's output. Each needs `TestResults` (green, naming the runner) and `TestFrames` (its property cases), wants `ProjectInfo`, costs `standard`. When a root (or member) manifest names the package (the toolchain table's `mentions` rule), it runs `TestSelection::All` through the baseline's runner with the property budget options (`property::budgeted`, the engine's seed and `--cases`/depth budget for every framework of the cases in scope; none for a framework the runner cannot take, whose properties keep their own budget) plus the switches, and reads every failed test's output. Places printed relative to a root are resolved in the first root that has the file.

   | Producer | Language | Declared in | Switches |
   |---|---|---|---|
   | `tool.rust-contracts` | `rust` | `Cargo.toml` names `contracts` | `CARGO_PROFILE_DEV_DEBUG_ASSERTIONS=true`, `CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true` (no rebuild when the profile already has them) |
   | `tool.icontract` | `python` | `pyproject.toml`, `setup.cfg`, `setup.py`, `requirements*.txt` name `icontract` | `ICONTRACT_SLOW=1` |

   - Both are always ready: a project that declares the library cannot run its own tests without it, so a missing library is the baseline's red run (exit 4), and a project that does not declare it needs nothing. Probing for the library would have `doctor` and `--install` ask every Python project to install icontract.
   - Rust: a `thread '…' panicked at <file>:<line>:<col>:` line followed by one of the crate's four prefixes; the function is the name before ` violated`, the message what follows it.
   - icontract: pytest's `E   icontract.errors.ViolationError: File <path>, line <n> in …:` and the `E` lines after it, joined with `; ` and masked; the kind is the nearest decorator at or above that line in the source (`require`, `ensure`, `invariant`), the function the next `def`, `async def`, or `class`. A violation whose source cannot be read is left out.

3. **`spec.contracts`** (`cupel-checks/src/spec_contracts.rs`): tier 5, `standard`, needs `ContractViolations`, no settings, experimental. Each violation is a `contract.violated` finding (`fail`, `fix_bug`, confidence 1) at the contract's place (none outside the repository), titled ``<kind> of `<function>` violated in `<test>`: <message>``, evidence `{tool, kind, function, message, test}`, ID by `<kind>|<function>|<message>|<test>` (repeats numbered), so it survives line moves; the action says whose obligation the kind is (the caller's for a precondition, the function's for a postcondition, the method's for an invariant). Score `{violations, unit "violations", n: tests}` and metric `violations`, summed across languages, only when the library is declared and a root's run completed; a failed run is a note, and an undeclared library a note that nothing was checked.

4. **Support.** `rust` and `python` supported; Go, JavaScript, TypeScript, Java, Clojure `unsupported(contract-violations)` for the reasons under Options.

## Consequences

- `spec.contracts` costs one more suite run where the library is declared, and none elsewhere; under `--changed` the run is still the whole suite (a contract in changed code may be violated from any test), and only its violations are narrowed.
- What it can detect is what the libraries report: Rust's `contracts` crate (any form) and icontract (any decorator); it cannot see a violation the code catches (`catch_unwind`, a broad `except`), a contract in log mode (the crate's `override_log`), or plain assertions. A contract the project's own run checks and fails is the baseline's `baseline.test-failed` (exit 4), not this check's.
- Deferred, each a producer of this fact: deal (its errors name neither function nor place; the fact needs both); Clojure `:pre`/`:post` (built in, but the kind is not reported); Java (Guava preconditions are ordinary exceptions; JML needs OpenJML, a separate compiler, not an install of minutes); Go, JavaScript, TypeScript (no contract library in common use to recognize); Cupel-side instrumentation of declared conditions (the design's "instrumentation hooks"), which would need a per-language rewriter and is left until a language has contracts no library checks.
- The panic header format is rustc's since 1.73 (`panicked at <place>:` then the message); older toolchains' one-line form is not read. icontract's message format is pinned by CI's install (2.7.3).
- The toolchain table gets no column: the producers have no command, detection order, or install that the table's columns describe; `describe language` lists them as `ContractViolations` producers.
