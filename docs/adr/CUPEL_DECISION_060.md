# CUPEL_DECISION_060: Check metrics and gate evaluation

**Date:** 2026-10-14 · **Status:** accepted; the `combine_metric` signature of §2 superseded by [063](CUPEL_DECISION_063.md), the `no-breaking` clause of §6 by [101](CUPEL_DECISION_101.md) · **Phase:** P2.9

## Context

Design §6.7 says `[gate.changed]` is evaluated for `--changed` scopes and `[gate.tree]` for whole-tree scopes, that rule keys are `<check-id>.<metric>` or `<check-id>` (presence), that the operators are `>= > <= < == none no-breaking`, and that `--gate` is the default when a gate exists; §7.1 gives `summary.gate { result, evaluated, rules[] { rule, actual, result } }`; §6.6 makes a failed gate exit 1. ADR-017 parses the rules and checks each metric name against `Manifest.metrics`. What is not decided:

1. **Where a metric's value comes from.** Manifests declare metric names, but a `CheckResult` carries only a `score`; `static.complexity` declares five metrics and reports one number.
2. **How per-language values combine** into the one entry per check of ADR-041.
3. **What "presence" means** for `none`: any finding, or only findings that matter after suppression (ADR-047 §6 says gates read severities after suppression).
4. **A rule whose value is missing**: the check was not selected, was skipped for every language, failed, or was cut short.
5. **`no-breaking`**, which is about `static.api-surface` (P6.2); no registered check has a notion of a breaking finding.
6. **`%` thresholds**, and which table a `--paths`, `--symbols`, or `--since-baseline` scope uses.
7. **`--gate` with no applicable table**, and how a failed gate combines with `partial` or `error`.

## Options

**(1) Metric values.** Put them in the existing `score` (one number per check, so four of `static.complexity`'s five metrics stay unreadable), read them from finding evidence (it exists only when something is wrong, so a passing rule could never show its actual value), or add a `metrics` map to `CheckResult` that the check fills with a value per declared metric. The map is the only option that gives every declared gate key a value; its cost is one more field in every `CheckResult { … }` literal.

**(2) Combining.** Evaluate the rule per language and fail when any language fails (no single `actual` to report), or combine per-language values into one by a rule the check owns, as `Check::combine` already does for scores.

**(4) Missing values.** Pass the rule (an agent can then satisfy any gate with `--skip` or `--only`), mark it unevaluated without affecting the result (the same hole), or fail the gate with the rule marked `skipped` (an agent iterating with `--only` must say `--no-gate`, which is explicit and visible in the envelope).

**(5) `no-breaking`.** Implement it now as "no finding of a kind the manifest flags breaking" (a new `FindingKindSpec` field nothing sets until P6.2), reject it at configuration load, or evaluate it to a distinct `unsupported` result.

## Decision

1. **Metrics.** `CheckResult` gains `metrics: {name: number}`: one value per metric the check measured over its scope. A check fills it in the result its `run` returns; the scheduler combines the per-language maps into the check's one entry. Keys are the manifest's metric names whose `per` is `scope` (ADR-062); only those are valid gate keys, which `Config::validate_registered` enforces. Skipped entries have an empty map. Stored envelopes without the field read as an empty map.
2. **Combining.** `Check::combine_metric(name, values) -> Option<f64>` combines one metric's per-language values; the default is the largest, because the metrics declared so far are bounds over a scope (`cyclomatic_max`, `crap.max`). A check whose metric is a ratio or a count overrides it (a mutation score: weighted mean; a count: sum).
3. **Tables.** The applicable table is `[gate.changed]` when `run.scope.mode` is `changed`, `range`, or `since-baseline`, and `[gate.tree]` for `tree`, `paths`, and `symbols`. `evaluated` is `gate.changed` or `gate.tree`.
4. **`--gate` / `--no-gate`.** Without either flag, `check` evaluates the applicable table when it exists (an empty table passes with no rules) and evaluates nothing otherwise (`summary.gate: null`). `--no-gate` never evaluates. `--gate` with no applicable table is a usage error (exit 2) on `--gate`, raised before any check runs, naming the table to add. `survey` never gates (ADR-062).
5. **Order.** The gate is evaluated after suppressions and deltas (`app::graded`), so suppressed findings (now `info`) never count, and before `next_actions` (ADR-061), which turns rules that could not pass into actions.
6. **Rule results** are `pass`, `fail`, `skipped`, or `unsupported`; the gate's `result` is `pass` only when every rule passes.
   - When the scope has no files (an unchanged `--changed`), every rule passes with `actual: null`: design §6.4 makes that run a `pass`, and no check had anything to measure.
   - A rule whose check has no entry that ran (not selected, skipped for every language, `error`, or `partial`) is `skipped` with `actual: null`. A check that ran for some languages and was skipped (unsupported) for others is evaluated on what ran.
   - **Comparisons** read `metrics[<metric>]` of the check's entry; a missing metric is `skipped`. A `%` threshold is divided by 100 (`< 5%` compares with 0.05): metrics are ratios, and percent is notation. `==` is exact.
   - **`none`** passes when the check produced no finding of severity `warn` or `fail` in the graded envelope; `actual` is the number of such findings. `info` findings, including every suppressed finding, are not "present".
   - **`no-breaking`** is `unsupported`, `actual: null`, until a check defines what a breaking finding is (P6.2 `static.api-surface`); the gate then fails and ADR-061 attaches a `configure` action. A configuration can only name a registered check, none of which reports breaking changes today, so failing loudly is safer than passing silently.
7. **Status.** A failed gate turns a `pass` envelope into `fail`, exit 1. A `partial` (exit 5) or `error` envelope keeps its status and exit code; its `summary.gate` still reports the evaluation (a `partial` run's interrupted checks show as `skipped` rules).

## Consequences

- Every gate key a manifest declares has a value or a visible reason it has none; the envelope shows `actual` for passing rules too.
- Checks that land after P2.1 must fill `metrics` for each `scope` metric they declare, or their gate rules are always `skipped`; a check whose metric is not a bound overrides `combine_metric`.
- Iterating with `--only` in a repository with a gate needs `--no-gate`; the rerun actions of ADR-061 say how to evaluate the rest.
- P6.2 supersedes the `no-breaking` clause of §6 with a definition of a breaking finding.
