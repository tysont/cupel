# CUPEL_DECISION_067: CRAP exists only where coverage was measured; a `CoverageMap` payload

**Date:** 2026-10-14 · **Status:** accepted; supersedes the `no_coverage` mode and the `unconfigured` skip of [048](CUPEL_DECISION_048.md) · **Phase:** P2.10

## Context

ADR-048 gave `static.crap` two behaviours without a `CoverageMap`: a per-language `skipped` result (`unconfigured`, fact `coverage-map`), or, with `[checks."static.crap"] no_coverage = true`, CRAP computed with coverage counted as 0, which is `CC² + CC`. The project owner ruled for crapper's semantics instead: CRAP is nil when coverage is nil. CC-only CRAP is a monotone function of cyclomatic complexity, which `static.complexity` already reports; scoring it as CRAP fabricated `crap.watch` for every function with one branch (P2.3 open item) and made a gate rule on `static.crap.max` read as if coverage had been measured. `CoverageMap` had no payload type, so the check could not even be tested with coverage.

## Options

1. Keep ADR-048 (skip, or CC-only on request).
2. CRAP only with coverage: without a `CoverageMap` the check runs, reports no findings, and has no score; a payload type for `CoverageMap` lets the with-coverage path be tested against hand-built facts until P3.2 adds producers.

## Decision

Option 2.

- **`CoverageMap` payload** (`cupel_core::coverage`, version 1): `files[{file, lines[{line, hits}], branches: null | [{line, branch, hits}]}]`, the design §8.3.2 shape (line hits plus optional branch hits). `CoverageMap::line_coverage(file, start, end)` is the share of instrumented lines in the range that ran at least once, `None` when none is instrumented. No producer exists until P3.2; `describe fact coverage-map` now shows its schema and version.
- **`static.crap` version 2.** Per language: without a `CoverageMap`, `status: pass`, `score: null`, `metrics: {}` (empty, so a gate rule on `static.crap.max` is `skipped` and fails the gate rather than passing on nothing), `findings: 0`, and no measurements. With one, a symbol is scored only when its span holds an instrumented line; CRAP = `CC² × (1 − coverage)³ + CC`; bands, actions, test-file hints, score (`max`, `n` = symbols scored), and metrics (`max`, `mean`) are as in ADR-048. Evidence `coverage` is now always a number.
- **Settings** are `high` and `watch`; `no_coverage` is no longer a setting (a configuration that sets it is exit 2), and `settings::Limits` loses its boolean flags.
- The manifest summary states the limitation: CRAP exists only where coverage was measured.

## Consequences

- Until P3.2, `static.crap` reports nothing in any repository, Cupel's own included; no `static.crap.max` gate rule can pass, so Cupel's `cupel.toml` carries none until then (the plan's `static.crap.max < 30` rule waits for P3.2).
- The fixture `crap_scores` files and the `static.crap` goldens now record "no coverage, no CRAP"; the with-coverage behaviour is covered by unit tests over hand-built `CoverageMap` facts. The binary-level `explain` and text-rendering tests of `crap.high` findings were removed with the behaviour they exercised; P3.2 restores them with a real coverage producer.
- P3.2 adds producers and `tests.coverage` on top of this payload; branch coverage is already representable.
