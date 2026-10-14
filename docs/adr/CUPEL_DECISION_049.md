# CUPEL_DECISION_049: Checks may skip a language themselves; fixture repositories carry their configuration

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P2.3

## Context

ADR-041 lets the scheduler skip a (check, language) pair for support-matrix reasons and for timeouts, and a check's `run` returns a `CheckResult` whose `status` may be `skipped`. The scheduler treated every per-language result with status `skipped` as a timeout and marked the run interrupted (`partial`, exit 5). `static.crap` (ADR-048) is the first check that skips on its own — no coverage and no `no_coverage` — which is a configuration state, not a cut-short run.

The matrix test (ADR-043) runs `cupel check --only <check>` on a copy of each fixture repository with no flags and requires a supported pair not to be skipped. A check that needs configuration to run at all therefore needs it in the fixture repository.

## Options

**Interruption.** Keep "skipped means interrupted" and have `static.crap` report a passing result with a `skipped` object (a contradiction the envelope would show), or decide interruption by the skip's reason: only `timeout` (and a `partial` status) interrupts.

**Fixture configuration.** Teach the matrix engine per-check arguments or configuration (a second place that knows about one check), or commit a `cupel.toml` at the root of each fixture repository, as a user would.

## Decision

- A run is interrupted when a per-language result is `partial` or is skipped with reason `timeout`. A check that returns `status: skipped` with any other reason is reported as a `skipped` entry, and the run's status and exit code are unaffected. `cupel-core/tests/scheduler.rs` covers it.
- Each language fixture repository (`repos/{go,javascript,python,rust,typescript}/`) has a `cupel.toml` holding `[checks."static.crap"] no_coverage = true`. Checks that need configuration add their keys to the same files.

## Consequences

- Every check golden on those repositories echoes the file in `run.config` (`sources` gains `cupel.toml`, `effective.checks` its tables); a configuration change there changes every check's goldens, reviewed like any golden diff.
- Tests that copy a fixture repository into a subdirectory and write their own `cupel.toml` at the top (`check_runs.rs`) are unaffected, since discovery stops at the nearest file from the run's root.
