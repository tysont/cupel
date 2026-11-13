# CUPEL_DECISION_195: Where a metric's ratchet tolerance is declared and how it applies, and which runs a baseline may name

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.7, item 17 · Details [182](CUPEL_DECISION_182.md), [130](CUPEL_DECISION_130.md), [046](CUPEL_DECISION_046.md)

## Context
ADR-182 gives each ratcheted metric a tolerance δ (counts 0, `tests.mutation.score` 0.02) and rules that the reference baseline advances only on a passing run on the default branch. It leaves open where δ is declared, which way it moves a bound, how a rule's written offset relates to it, how a gate result says which δ it used, and what "passing" and "the default branch" mean for `cupel baseline`, the only thing that moves a baseline. Cupel has no metric direction: a rule's operator says which way is better.

## Options
1. Where δ lives. (a) A table in the gate evaluator: plugins could not declare one. (b) `cupel.toml`: every project restates the owner's values. (c) The check's manifest, on the metric's declaration, which is where its name, unit, and kind are.
2. Which way δ moves the bound. (a) A declared direction per metric. (b) The rule's operator: `>=`/`>` is a lower limit, `<=`/`<` an upper one.
3. "Passing" for a baseline run. (a) Its gate passed. (b) Its status is `pass`: the gate passed or none was evaluated, and the run was not cut short (`partial`) and did not error. (c) As (b), and also a run whose gate failed only on rules against `base` skipped for want of a reference: the run before the first baseline always fails so, and could otherwise never become one.
4. The default branch. (a) A setting of its own. (b) What `--changed` already compares with: `default_branch` in `cupel.toml`, else `Git::default_branch` (the branch `origin/HEAD` names, else a local `main` or `master`, else `HEAD`'s branch).

## Decision
1. **(c).** `MetricSpec.tolerance` is an optional number on a `scope` metric; absent is 0. `tests.mutation` declares 0.02 on `score`; no other native metric declares one, so every count is exact. A plugin manifest may declare one the same way. The gate reads the registered checks' tolerances (`Tolerances::declared`).
2. **(b).** A rule against `base` that writes no offset is loosened by δ: the bound is base − δ under `>=` and `>`, base + δ under `<=` and `<`. `==` takes no tolerance. A written offset, including `+ 0` or `- 0`, replaces δ: `>= base - 0` is exact. `GateCondition::Relative`'s `offset` is `None` when no offset is written, and the rule's display keeps what was written.
3. **Reporting.** A rule that applied δ carries `tolerance` in `summary.gate.rules[]` (0 for a count), beside `bound`, which already has δ applied. A rule with a written offset, `==`, a threshold, or no bound has no `tolerance`. Text prints `tolerance δ` after the bound.
4. **(c)** for passing, and **(b)** for the default branch. `baseline set` (the latest result run, or `--from`) and `baseline import` refuse, as usage errors, a run that did not pass in sense (c), and a run recorded in a git work tree whose branch is not the default branch, a detached `HEAD` included. A run recorded outside a work tree has no branch and is accepted. When a run has a branch but no default branch can be found, the refusal says to set `default_branch`. The preview without `--write` refuses the same way.

## Consequences
- `tests.mutation.score >= base` no longer fails at random on a run whose score moved by sampling alone. `static.deps.cycles <= base`, `tests.weakening`, and `static.api-surface` counts stay exact.
- A gate result written before this has no `tolerance`; the field is optional and the schemas stay `v1`.
- CI that runs `cupel baseline set --write` on every branch now fails there instead of moving the baseline: run it on the default branch after a passing check, as `make baseline` on `main` does. An exported baseline made on the default branch imports on any branch, since the run, not the importer, carries the branch.
