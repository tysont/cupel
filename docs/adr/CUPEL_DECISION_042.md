# CUPEL_DECISION_042: `static.complexity`: metrics, default thresholds, findings, and score

**Date:** 2026-10-13 · **Status:** accepted · **Phase:** P2.1

## Context

Design §5.1 says `static.complexity` measures "cyclomatic, cognitive (nesting-weighted), nesting depth, param count, length per symbol" and emits `complexity.high`, `nesting.deep`, `params.many`, `function.long`; §6.7 shows `[checks."static.complexity".lang.go] cyclomatic_max = 15`. ADR-024 fixed how the `Decisions` producer weighs decisions and how the check should sum them. Nothing fixes the thresholds, the metric names usable as gate keys, which facts the check needs, the finding severities and actions, or the score.

Two facts the plan's step text leaves out turn out to be required. A finding ID is `FindingId::compute(check, kind, symbol_hash, …)` and a repro is `--symbols sh_…`, but `SymbolHash` lives only in `NormTree` (ADR-014: `Symbols` cannot carry it, since it is derived from `NormTree`). And `Decision` carries its kind, nesting, and weights, but not whether its kind nests (`Weight.nests` stays in the language tables).

## Options

**Thresholds.** McCabe's 10 for cyclomatic complexity is the long-standing default (and crapper's CC column); SonarSource's 15 for cognitive complexity is the one widely deployed default for that metric. For nesting, parameters, and length there is no standard: common linters use nesting 4 (ESLint `max-depth`), 5–7 parameters (ESLint `max-params` 3, Sonar 7, Pylint 5), and 50–100 lines per function. Stricter defaults flood a first run with findings; looser ones never fire.

**Nesting increments without `nests`.** Add `nests` to `Decision` (a fact-shape change in every language and every golden for a value no language overrides today), or let the check apply the nesting increment to the kinds that structure control flow (`branch`, `loop`, `case`, `catch`, `ternary`), which are exactly the kinds whose default `nests` is true (ADR-024). Kinds are semantic, so the check still names no language.

## Decision

- **Needs** `Symbols`, `Decisions`, and `NormTree` (for the symbol hash); tier 0, cost `quick`, version 1.
- **Metrics per symbol:** cyclomatic = 1 + Σ `cyclomatic`; cognitive = Σ `cognitive`, plus the decision's `nesting` for the structural kinds above when its `cognitive` weight is non-zero (so only a switch's first arm adds nesting); nesting depth = the largest `nesting + 1` over decisions of structural kinds (0 for straight-line code); parameters = `SymbolDef.parameters` (ADR-039); length = the symbol span's line count.
- **Thresholds**, under `[checks."static.complexity"]` with per-language overrides merged by `CheckCtx` (ADR-017): `cyclomatic_max = 10`, `cognitive_max = 15`, `nesting_max = 4`, `params_max = 5`, `length_max = 60`. A finding fires when a metric *exceeds* its maximum. Values must be non-negative integers; any other key or value is a usage error (exit 2) raised when the check plans, before any work runs.
- **Findings**, one per symbol per kind, all severity `warn` and confidence 1: `complexity.high` when cyclomatic or cognitive exceeds its maximum (evidence `cyclomatic`, `cognitive`, `cyclomatic_max`, `cognitive_max`; action `reduce_complexity`); `nesting.deep` (`depth`, `nesting_max`; `reduce_complexity`); `params.many` (`parameters`, `params_max`; `refactor_extract`); `function.long` (`lines`, `length_max`; `refactor_extract`). IDs are `FindingId::compute("static.complexity", kind, symbol hash, None)`; the repro is `cupel check --only static.complexity --symbols <hash>`; the location is the symbol's span. Two structurally identical symbols share a hash and therefore a finding ID; both are reported, each at its own location.
- **Metrics for gate keys** (one segment each, ADR-017 §3): `cyclomatic_max`, `cognitive_max`, `nesting_max`, `params_max`, `length_max`, each the largest value over the symbols in scope.
- **Score:** `{value: largest cyclomatic complexity, unit: "cyclomatic_max", n: symbols in scope}`; per-language scores combine by maximum (ADR-041).
- **Status:** `fail` when any finding was emitted, else `pass`.

## Consequences

- A language that overrode `nests` for a kind would see that kind nest other decisions but not receive the nesting increment itself; if that ever matters, `Decision` gains `nests` with a fact version bump.
- The defaults can be tightened per repository or per language without a release; gate rules (P2.9) can bound the five metrics.
- `static.crap` (P2.3) reuses the cyclomatic computation; when it lands, the shared arithmetic moves to a private module of `cupel-checks` (ADR-001).
