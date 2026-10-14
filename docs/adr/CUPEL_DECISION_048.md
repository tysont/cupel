# CUPEL_DECISION_048: `static.crap` in CC-only mode: the `no_coverage` switch, bands, findings, and score

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P2.3

## Context

Design §5.1 defines `static.crap` as `CC² × (1 − coverage)³ + CC` per function, wanting `CoverageMap`, with `--no-coverage` forcing CC-only scoring, and kinds `crap.high` (≥ 30) and `crap.watch` (5–30). Plan P2.3 adds that without a `CoverageMap` producer (P3.2) the check reports `skipped: "no coverage; use --no-coverage"` rather than silently computing CC-only. Three things are missing today: the `--no-coverage` flag does not exist in the CLI, `CoverageMap` has no payload type (so no check can read one), and `Skipped` has no free-text reason — `reason` is the closed `SkipReason` enum (`unsupported`, `unavailable`, `unconfigured`, `budget`, `timeout`). The plan also leaves open the evidence, severities, the action hint, settings and their validation, the score, and whether test code is scored.

## Options

**The switch.** Add `--no-coverage` now (a CLI flag that must reach one check's configuration, touching `cupel-cli` and the flag registry for a single check), or read a boolean `no_coverage` from the check's own `[checks."static.crap"]` slice, which `CheckCtx` already merges with per-language overrides. The flag can later set the same key.

**The skip.** Add a free-text field to `Skipped` (a schema change to every envelope), or express "no coverage; use `no_coverage`" with the existing fields: reason `unconfigured` (the check needs configuration it does not have), `fact: coverage-map` (what is missing), the language, and `see` pointing at `describe`, whose summary says how to enable CC-only scoring.

**Test code.** Score every symbol, or skip symbols in test files when `TestConventions` is available. A `write_test` finding on a test function is advice no one can follow, and every Go test with one branch would be a `crap.watch`.

## Decision

- **Manifest:** `static.crap`, version 1, tier 0, cost `quick`; needs `Symbols`, `Decisions`, `NormTree` (the hash, as in ADR-042); wants `CoverageMap` (consumed from P3.2) and `TestConventions` (the test-file hint and the test-code filter).
- **CRAP** = `CC² × (1 − coverage)³ + CC`, with CC computed exactly as `static.complexity` computes cyclomatic complexity (the shared `metrics::cyclomatic`, as ADR-042 anticipated). Until coverage is measured, coverage counts as 0, so CRAP = CC² + CC: CC 4 → 20 (watch), CC 5 → 30 (high).
- **Settings** under `[checks."static.crap"]`, per-language overrides allowed: `high = 30`, `watch = 5` (non-negative integers), and the boolean `no_coverage` (default false). Any other key or a value of the wrong type is a usage error (exit 2) at plan time. `settings::Limits` gained boolean flags (`with_flags`, `flag`) for this, listing flags among the valid names in errors.
- **The skip.** Without `no_coverage = true`, a language with symbols in scope yields a per-language result `status: skipped` with `skipped: {reason: unconfigured, fact: coverage-map, lang, see: "cupel describe static.crap"}`, and no findings. The manifest's summary names the key. A language with nothing in scope passes with `n = 0`, since it needs no coverage. This needs the scheduler change in ADR-049.
- **Test code** is not scored: when `TestConventions` is present, symbols in files it calls tests are excluded from findings and from `n`.
- **Findings**, one per symbol: `crap.high` when CRAP ≥ `high`, severity `warn`, action `write_test` with `hints.nearest_test_file` = the first test file `TestConventions` pairs with the symbol's file (absent when none); `crap.watch` when `watch` ≤ CRAP < `high`, severity `info`, action `review`, no hints. Confidence 1; IDs `FindingId::compute("static.crap", kind, hash, None)`; repro `cupel check --only static.crap --symbols <hash>`. Evidence (both kinds): `crap` (number), `cyclomatic`, `coverage` (null when not measured), `watch`, `high`.
- **Metrics for gate keys:** `max` (gate key `static.crap.max`, as in design §6.7) and `mean`. Like every P2 check, values are not yet emitted; P2.9 decides how gates read them.
- **Score:** `{value: largest CRAP, unit: "max", n: symbols scored}`; per-language scores combine by maximum. **Status:** `fail` when any `crap.high` was emitted; `crap.watch` alone leaves the check `pass`.

## Consequences

- The fixture repositories carry `cupel.toml` with `no_coverage = true` so the matrix test scores them (ADR-049); every check's goldens echo that configuration.
- **P3.2 follow-up:** give `CoverageMap` its payload type; when the fact is present, compute per-symbol line coverage over the symbol's span, fill `coverage`, and stop skipping; keep `no_coverage` (and add `--no-coverage` setting the same key) to force coverage 0. Bump the check version then, since results change for the same configuration.
- With coverage 0, any function with a branch is at least `crap.watch` (CC 2 → 6): `info` findings are numerous on real code. Raising `watch` per repository is the remedy; the default follows the design.
