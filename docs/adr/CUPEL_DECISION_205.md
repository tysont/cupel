# CUPEL_DECISION_205: Which rules lack a reference, and what a largest-value ratchet's watch threshold is

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.8, items 2 and 3 · Details [198](CUPEL_DECISION_198.md), [199](CUPEL_DECISION_199.md)

## Context
ADR-198 skips a rule against `base` "when no baseline exists" and lets the gate pass. A rule can lack a reference value three ways: no reference run (no baseline is set, or `--against` names no stored run), a reference run that did not report the metric, or a range whose changed files did not exist at its base. Each is a `skipped` rule whose reason starts `no reference:` (ADR-130).

ADR-199 gives `static.complexity`'s `cyclomatic_max`, `cognitive_max` and `nesting_max` δ = +2 under `<= base` and no failure "while the actual value is below the check's watch threshold". `static.complexity` has one threshold per metric, the setting of the same name (10, 15, 4), above which it reports `complexity.high` or `nesting.deep`; it has no separate watch band as `static.crap` does. A setting may differ per language. ADR-199 also says an offset written in the rule replaces δ, and leaves open whether the watch floor still applies then. Its Consequences say a change "that takes a function past watch, or more than two above its base, fails", while its Decision passes a value within δ of its base whatever the watch.

## Options
1. Which rules lack a reference. (a) Only a missing reference run. (b) Every `no reference` reason.
2. The watch threshold. (a) A separate watch setting. (b) The metric's existing limit setting, the value at which the check's finding fires.
3. At the threshold. (a) The rule passes only below it. (b) It passes at or below it: no finding fires there.
4. Per-language limits. (a) The language-neutral setting. (b) The lowest effective limit among the scope's languages.
5. With a written offset. (a) The floor still applies. (b) The floor comes with the default tolerance only.

## Decision
1. **(b).** Every rule skipped with a `no reference: …` reason is skipped without failing the gate and has a `baseline.missing` finding at `info` (check `gate`, evidence `rule` and `reason`). Each is the same situation for the gate: nothing to ratchet against yet. A rule skipped for any other reason (its check did not run, or did not report the metric) still fails the gate.
2. **(b).** A metric's manifest names the setting that is its watch threshold (`MetricSpec.watch`); `static.complexity` names `cyclomatic_max`, `cognitive_max` and `nesting_max` for its three largest-value metrics and declares `tolerance = 2` on them.
3. **(b).** A function within the check's limit is one the check does not report; "past watch" is above it.
4. **(b).** The value is the setting in `[checks."<id>"]` with each language's override merged, or its default, and the lowest across the run's languages, so the floor never passes a value some language in scope would report.
5. **(b).** The floor applies only to a rule against `base` under `<=` or `<` that writes no offset. A written offset, `+ 0` included, is the whole rule: `<= base + 0` is exact.
6. The Decision of ADR-199 holds over its Consequences: the rule passes when the actual is within base + δ or within the watch threshold. A value past watch that is also within δ of its base passes the ratchet; `complexity.high` still reports it, and a `none` rule on the check fails on it.
7. A rule that passed only because of the floor carries `watch`, the threshold, in `summary.gate.rules[]`; text prints `within watch N` after the tolerance.

## Consequences
The first run before any baseline, and a change whose files are all new, pass a gate of ratchets. A project that lowers `cyclomatic_max` lowers the ratchet's floor with it. Gate results written before this have no `watch`; the field is optional and the schemas stay `v1`.
