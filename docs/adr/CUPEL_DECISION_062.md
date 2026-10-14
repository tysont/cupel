# CUPEL_DECISION_062: `survey`, per-target measurements, and `rankings`

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P2.9

## Context

Design §6.5: `survey` uses the same scope and depth machinery as `check`, with no gate and no baseline required, and adds `rankings`: top-N per dimension (worst CRAP, largest clone clusters, weakest mutation score, most-coupled modules, flakiest tests, dependency cycles). §6.2 keeps it separate from `check` because ranking without a baseline is a different contract. ADR-018 §1 says each verb payload is one nullable, typed top-level envelope field. Nothing says what a dimension is, where per-target values come from (P2.1 checks report findings and one score; the store's per-symbol `metrics` table has no writer), what N is, or what a ranked entry holds. Rankings must not depend on which checks exist: six checks land in parallel with this step, and more in P3.

## Options

**Declaring dimensions.** A hard-coded list in the CLI (contradicts "adding a check is one module"), a new manifest list of rankings (a second vocabulary beside `metrics`), or a marker on each `MetricSpec` saying what one value describes: the whole scope (a gate key) or one target. The marker reuses the existing metric vocabulary, and `describe <check>` shows it.

**Reporting per-target values.** A list in `CheckResult` (the result is assembled once at the end and is per check, so a check holds every value until it returns, and plugins would need it in their result message), a new `Event` variant (would put thousands of values into `--events`, a progress stream), or a method on `EventSink` with a no-op default that the scheduler's sink collects (checks stream values as they compute them; sinks that do not care ignore them; nothing that implements `EventSink` today must change).

## Decision

1. **Metric granularity.** `MetricSpec` gains `per: scope | symbol | module | group`. `scope` metrics are gate keys and are reported in `CheckResult.metrics` (ADR-060). The others are ranking dimensions: `symbol` for a function or method, `module` for a file or module (coupling), `group` for a set of targets (a clone cluster, a dependency cycle).
2. **Measurements.** A check reports one per-target value by calling `sink.measure(Measurement { metric, value, target })`, where `target` is `Target { display, symbol, location }` (`symbol` the structural hash when the target is a symbol). `EventSink::measure` defaults to doing nothing; the scheduler collects measurements with the check and language that produced them in `Outcome.measurements`. Measurements never reach `--events` or the envelope of `check`.
3. **What a check must emit to appear in `rankings`:** a `MetricSpec` with `per` other than `scope`, and one `sink.measure` call per target in scope for that metric name. A declared dimension with no measurements is listed with no entries; a measurement whose metric is not declared that way is ignored.
4. **`rankings`.** A new nullable top-level `Envelope.rankings`, null except for a completed `survey`: `{top, dimensions[]}`, each dimension `{check, metric, per, summary, measured, entries[]}` in registry order then manifest order, for every check with an entry in `checks[]` (selected, run or skipped). `measured` counts the targets measured; `entries` are the `top` highest values, each `{rank, value, lang, target}`, ordered by value descending, then display name, file, and line, so equal values rank deterministically. Higher is worse for every dimension declared so far; a dimension where lower is worse (P3's mutation score) adds a direction to `MetricSpec` when it lands.
5. **N** is 10 per dimension, recorded in `rankings.top`. Paging flags (`--limit`/`--offset`) page findings only, as for every verb.
6. **`survey`** runs the same pipeline as `check` (`check::run`: one walk, scope resolution, selection, scheduler) and differs only in what it does with the outcome: it adds `rankings`, never evaluates a gate (`--gate` is a usage error on `survey`; `--no-gate` is accepted and changes nothing), and returns `pass` (exit 0) or `partial` (exit 5). Suppressions and deltas apply as for `check` (ADR-046 §5), so a baseline is used when set but never required. `--dry-run` is the same plan as `check`'s.
7. **`static.complexity`** declares two `symbol` dimensions, `cyclomatic` and `cognitive`, and fills `metrics` with its five `scope` metrics.

## Consequences

- A check joins `survey` by declaring a dimension and calling `measure`; neither the CLI nor `cupel-core` changes.
- `MetricSpec { … }` literals gain `per`; existing `scope` metrics keep their names and remain gate keys.
- The `survey` goldens change whenever a check is added or a fixture grows; each change is reviewed like any golden diff.
- Per-target measurements are not stored yet; when `explain <check-id>` wants per-symbol values, the store's `metrics` table can record `Outcome.measurements` without a contract change.
