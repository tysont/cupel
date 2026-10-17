# CUPEL_DECISION_115: `meta.oracle-independence`: labeling changed functions by whether a test from before the change exercises them

**Date:** 2026-10-17 · **Status:** accepted · **Phase:** P7.6

## Context

Design §5.7: `meta.oracle-independence` "labels each passing check by whether its oracle is independent of the change author (reference impl, spec, pre-existing tests) vs self-authored", kind `evidence.self-authored-only`, cost quick; §5.8 runs it at `quick`. An agent that writes both the code and its tests can make every test-based check pass with tests that encode its own misunderstanding; the label tells it, and whoever reads the envelope, which evidence came from someone else.

The facts model (§8.3.1, §8.3.4) gives a check facts, its configuration, and nothing else: no other check's result, no store, no oracle registry. So the question is which facts carry "which tests pre-existed the change" and "which oracles are verified", and what "label each passing check" can mean for a check that cannot see the others.

## Options

**Labeling other checks' results.** (a) A post-run pass in the CLI that reads every result and adds a label field to each: outside the check catalog, a change to the envelope's `CheckResult` schema, and a list in the CLI of which checks use which oracle. (b) A check over facts that labels the evidence itself, by oracle: for checks whose oracle is the test suite (`tests.coverage`, `tests.mutation`, `tests.flaky`, `gen.property`, `spec.contracts`), the tests; for checks against a registered oracle (`gen.differential`), the oracle. The envelope's reader joins the label with the checks that consume that oracle. (b) keeps the model's rule and needs no schema change.

**Registered oracles.** Whether an oracle is verified is recorded by `cupel oracle verify` in the store (`verified_at`, ADR-090) and applied after the run: a result against an oracle that is not verified now is capped at tier 3 with one `oracle.unverified` finding (ADR-091 §4). No fact carries the verification, and adding one would duplicate that cap. A verified oracle is independent by construction (someone verified it against the reference), so the label for registered oracles already exists.

**Which tests pre-existed the change.** `Focus.base`/`Facts::range` give the range; `TestFrames` at the base (`base_needs`, ADR-056) holds the test cases of the changed files as they were. A head case with no matching base case (`tests.weakening`'s matching) was added by the range; a test in a file the range did not change is not in the head's facts at all, and existed before.

**Which tests exercise the change.** (a) Test files paired with changed sources (`TestConventions`): misses tests elsewhere, and Rust's tests live beside the code. (b) The per-test coverage map (`PerTestMap`, ADR-077): the tests that executed each changed function's lines, exactly. It costs a coverage run (standard), and JavaScript, TypeScript, Java, and Clojure have no producer of it. (c) Mutation kills: stronger, but the mutation engine's cost.

## Decision

1. **The check labels the test-suite oracle; registered oracles keep ADR-091's cap.** `meta.oracle-independence` (`cupel-checks/src/meta_oracle_independence.rs`): tier 2, cost `quick`, needs `Symbols`, `NormTree` (finding identity), `TestFrames`; `base_needs = [TestFrames]` (range only, `needs-range` otherwise); wants `PerTestMap` and `TestConventions`; no settings; experimental.

2. **Changed functions** are the symbols in scope (under a range, those of the changed files), less those in test files (`TestConventions`) and those overlapping a test case of their file (a test's own function).

3. **With a per-test map**, each changed function some test executes (the map's `covering` index over its lines) is `independent` when any of those tests is not one the range added, else self-authored. A map test is one the range added when it is a head case of its file (any file, when the map does not name it), by `names_case`, that matches no base case. Each self-authored function is an `evidence.self-authored-only` finding (`warn`, `review`, by symbol hash), evidence `{base, tests}` (the map's test IDs), asking to check those tests' expectations against the task, or to show the change against a test or verified oracle it did not write. Score `{independent / exercised, unit "independently tested share", n: exercised}` (weighted across languages), metrics `independent` and `self_authored_only` (summed), and a note stating the label and that an oracle's independence is its verification. A function no test executes is not labeled (that is `tests.coverage`'s finding). The status is `pass`: the label is information; a gate rule on the check (`none`) or its metrics makes it a stop condition.

4. **Without a per-test map** (at `quick`, where `PerTestMap`, a `standard` fact, is not made, or in a language with no producer of it) nothing is labeled: no score, no metrics, a note saying which tests exercise the change is unknown and where a map is made.

## Consequences

- The design's cost `quick` is kept for selection, and at `quick` the check says it cannot label; its finding needs `--depth standard` in Rust, Go, or Python, where the map shares the suite run that makes `TestResults` and `CoverageMap` (ADR-084).
- "Pre-existing" is by test name and file: a test that existed at the base and that the change edited (its expected values rewritten) still counts as independent; `tests.weakening` reports the loosened and removed assertions among those edits, not a changed expected value.
- A pre-existing test that executes a changed function's lines without asserting on them makes it independent; a mutation-kill version of the label (option (c)) would close that, at the engine's cost.
- The label is per function and per oracle kind, not per check: a reader maps "the test-suite oracle" to the checks that use it. Labeling results in the envelope (option (a)) would need a `CheckResult` field and an ADR of its own.
- In JavaScript, TypeScript, Java, and Clojure the check is supported (its needs are syntactic) but labels nothing until a `PerTestMap` producer exists for them.
