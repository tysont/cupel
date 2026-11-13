# CUPEL_DECISION_182: Gate ratchets have a per-metric tolerance, and the baseline advances only on a passing default-branch run

**Date:** 2026-11-11 · **Status:** accepted; detailed by [195](CUPEL_DECISION_195.md); amended by [199](CUPEL_DECISION_199.md) (owner rulings, 2026-11-13) · **Phase:** v0.2.7, the owner's rulings on the Molten field trial · Amends [175](CUPEL_DECISION_175.md) and [130](CUPEL_DECISION_130.md)

## Context
ADR-175 made a range's base the touched symbols at the base, and left open how much a ratcheted metric may move. Sampled ratios such as the mutation score move between runs with no change in the code, so a zero tolerance on them fails at random, while any tolerance on counts lets regressions through.

## Options
(a) Zero everywhere. (b) One global tolerance. (c) Per metric: counts exact, sampled ratios with a small δ (the owner's ruling).

## Decision
Each ratcheted metric has a tolerance δ. Counts are exact, δ = 0: `tests.weakening`, `static.api-surface`, and dependency cycles. Sampled ratios have δ = 0.02: `tests.mutation.score`. The reference baseline advances only on a passing run on the default branch; a failing run, or any run on another branch, never moves it. Molten's proposed `[gate.changed]` (M8-3 in its `cupel.md`) uses these values.

## Consequences
Rules already accept an offset against `base`; the tolerance makes it the default per metric. v0.2.7 implements it; the plan's batch v0.2.7 Notes record the implementation and its tests.
