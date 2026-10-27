# CUPEL_DECISION_054: `tests.quality`: facts, finding rules, thresholds, SCRAP evidence, and score

**Date:** 2026-10-13 · **Status:** accepted; the consequence that table-driven Go tests are `test.logic` superseded by [069](CUPEL_DECISION_069.md); a comprehension clause is not a loop or branch of `test.logic` per [138](CUPEL_DECISION_138.md) §9 · **Phase:** P2.6

## Context

Design §5.2 defines `tests.quality` as "SCRAP-style: assertion density, weak assertions, mock density, logic in tests, duplicated setup" with kinds `test.weak-assertion`, `test.no-assertion`, `test.mock-heavy`, `test.logic`, `test.duplicate-setup`, cost `quick`, tier 2; plan P2.6 adds that each finding carries SCRAP's remediation mode and actionability class in its evidence (design §3). `TestFrames` (ADR-027) gives each test case its assertions with semantic `kind` and `strength`, its mocks, and its skip marker, but no symbol hash (needed for a finding ID, ADR-014) and no control flow. Nothing fixes which facts the check reads, when each kind fires, the thresholds, the evidence, the actions, the metrics, or the score.

Two traps were found in the fixtures. In Go's standard library runner the assertion *is* a guarded failure call (`if got != want { t.Errorf(…) }`, ADR-032), so every idiomatic Go test contains an `if`; counting every branch as logic would report every Go test. And a test whose framework treats a thrown error as the assertion (`#[should_panic]`, `pytest.raises`, `expect(f).toThrow(…)`, `assert.Error`) must not read as having no assertion.

## Options

**Joining a case to its symbol.** `TestCase` carries a span but no symbol key. Test cases are symbols in all five languages (named functions, and JavaScript/TypeScript test callbacks per ADR-030), and the `@test.case` node is the symbol's node, so the case's span start is the symbol's `SymbolKey`. The alternative, a symbol key on `TestCase`, is a fact change in every language for something the spans already give.

**Logic in Go-style tests.** (a) Count every branch, loop, case, handler, and conditional expression: every Go standard-library test is reported. (b) Count only loops: misses `if`/`else` around assertions in every language. (c) Discount one `branch` per assertion and per skip marker: a guarded failure call or a guarded skip (`if testing.Short() { t.Skip() }`) is the shape of one assertion, so only branches beyond those count, plus every loop, case, handler, and conditional expression. `Decision` has no span, so the check cannot tell which branch guards what; (c) is the bound it can compute.

**Mock density.** Mocks per assertion is unbounded when a test has no assertion; mocks as a share of mocks plus assertions is bounded and reads as a percentage. Settings are non-negative integers (the shared `settings::Limits`), so the ratio is configured as a percentage.

## Decision

- **Manifest.** `tests.quality`, version 1, tier 2, cost `quick`; `needs = [TestFrames, Symbols, NormTree]` (`NormTree` for the symbol hash, as ADR-042 found for `static.complexity`, and for ADR-055's setup comparison); `wants = [Decisions]`, so a language without a `Decisions` producer still gets every kind but `test.logic`.
- **Cases.** A case is the symbol whose `SymbolKey` is the case's span start. A case is measured when that key is in scope (`Scope::includes`); a case without a symbol or hash is measured but not reported. Skipped cases are measured like any other.
- **`test.weak-assertion`** when a case has at least one assertion and every one is `weak`. Evidence `assertions`, `weak`, `kinds` (the distinct kinds of its assertions). Action `strengthen_assertion`; remediation `strengthen`; actionability `guided` (the fix needs the expected value).
- **`test.no-assertion`** when a case has no assertion of any kind. A `throws` assertion (`#[should_panic]`, `pytest.raises`, `toThrow`, `assert.Error`) is an assertion. Evidence `mocks`, `skipped`. Action `write_test`; remediation `add-assertion`; actionability `guided`.
- **`test.mock-heavy`** when a case has at least `mocks_min` doubles and its mock density, ⌊100 · mocks ÷ (mocks + assertions)⌋, exceeds `mock_density_max`. Evidence `mocks`, `assertions`, `density`, `mock_density_max`, `mocks_min`, `targets` (what is replaced, as written). Action `review`; remediation `decouple`; actionability `judgment` (which collaborator to make real is a design choice).
- **`test.logic`** when a case's logic exceeds `logic_max`, where logic = `loop` decisions + `case`, `ternary`, and `catch` decisions (`alternatives`) + `branch` decisions beyond its guards, and guards = assertions + 1 if the case has a skip marker. `boolean-op` and `early-return` decisions are not logic. Evidence `logic`, `logic_max`, `loops`, `branches`, `alternatives`, `guards`. Action `refactor_extract`; remediation `simplify`; actionability `guided` (split into cases or parametrize).
- **`test.duplicate-setup`**: ADR-055. Action `refactor_extract`; remediation `extract-fixture`; actionability `mechanical`.
- **Evidence.** Every kind's evidence starts with `remediation_mode` and `actionability` (`mechanical | guided | judgment`), each a string enum in the kind's schema.
- **Thresholds** under `[checks."tests.quality"]`, per-language overrides merged (ADR-017), non-negative integers, any other key a usage error at plan time: `mock_density_max = 50` (percent; more doubles than assertions), `mocks_min = 2` (one double never makes a test mock-heavy), `logic_max = 0`, `setup_statements_min = 3`.
- **Findings** are `warn`, confidence 1, one per case per kind (one per group for duplicate setup), identified by `FindingId::compute("tests.quality", kind, hash, None)`, located at the case's span, reproduced by `cupel check --only tests.quality --symbols <hash>`.
- **Metrics** (gate keys): `assertion_density` (assertions per measured case), `weak_ratio` (weak assertions ÷ assertions, 0 with none), `no_assertion` (cases with no assertion).
- **Score** `{value: assertion density, unit: "assertion_density", n: cases}`; per-language scores combine by the default `n`-weighted mean (ADR-041), which is the density over all cases.

## Consequences

- A Go test with one guarded failure call per assertion is not logic; a table-driven loop is (`judgment` is the user's: `[checks."tests.quality".lang.go] logic_max = 1`). A test elsewhere with `if x { assert a } else { assert b }` is under-counted by the guard rule (one branch, two assertions); a `Decision` span would make the rule exact, at the cost of a fact change in every language.
- A test with doubles and no assertion is reported twice when it has `mocks_min` or more doubles (`test.no-assertion` and `test.mock-heavy`): both are true and have different fixes.
- Assertion strength is the language tables' judgment (ADR-027); a weak-only test in one language is weak-only in all, since the check reads only kinds and strengths.
