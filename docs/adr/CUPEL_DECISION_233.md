# CUPEL_DECISION_233: `init` writes a starter `[gate.changed]` of ratchets

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 12 (starter gate) · Amends [021](CUPEL_DECISION_021.md) (the `cupel.toml` `init` writes); rules as [017](CUPEL_DECISION_017.md), [130](CUPEL_DECISION_130.md), [199](CUPEL_DECISION_199.md) and [205](CUPEL_DECISION_205.md) define them

## Context
Validating v0.2.9 on eight repositories, `init` wrote no `[gate]` table, so `check --changed --gate` on a freshly initialized repository refused with exit 2 (`--gate` without the applicable table is a usage error). The validation appended Molten's adopter-written `[gate.changed]` to every clone to replay pull requests (NOTES G1).

The owner's ruling: "init writes a starter [gate.changed] with ratchet rules — tests.weakening none, tests.mutation.score >= base - 0.02, complexity max <= base + 2 while under watch, static.clones.pairs <= base, static.api-surface no-breaking — each line commented."

Open: how each rule is written in the gate's grammar, in particular "complexity max <= base + 2 while under watch".

## Options
For the complexity rule: (a) `"static.complexity.cyclomatic_max" = "<= base + 2"`: a written offset is the whole rule and turns the watch floor off (ADR-205 §5), so a function at 9 could go to 11 past the limit of 10, which is not "while under watch". (b) `"static.complexity.cyclomatic_max" = "<= base"`: the metric's default tolerance δ = +2 and the watch floor apply (ADR-199, ADR-205 §2–6): the rule passes while the value is within `cyclomatic_max`, and past it only when the base was already past it and the value is within base + 2. That is the ruling's sentence. (c) Also `cognitive_max` and `nesting_max`: the ruling names one complexity rule, Molten's adopted gate has `cyclomatic_max` alone, and a default the ruling does not set is not added.

Chosen: (b), with `cyclomatic_max` alone.

## Decision
Every `cupel.toml` that `init` writes ends with this table, each rule under a one-line comment saying what it holds:

```toml
[gate.changed]
# No test is weakened: no assertion removed, matcher loosened, test skipped or deleted, or snapshot rewritten.
"tests.weakening" = "none"
# The mutation score is at least the base's less 0.02.
"tests.mutation.score" = ">= base - 0.02"
# The largest cyclomatic complexity stays within cyclomatic_max, or, past it at the base already, grows by 2 at most.
"static.complexity.cyclomatic_max" = "<= base"
# No more pairs of cloned functions than the base has.
"static.clones.pairs" = "<= base"
# No breaking change to the public API.
"static.api-surface" = "no-breaking"
```

It is written for every repository, whatever its languages, after the `[lang.*]` tables. `init` writes no `[gate.tree]`.

## Consequences
- `check --changed --gate` on a freshly initialized repository evaluates these five rules instead of refusing. A rule against `base` compares with the range's base (ADR-175), and one with nothing to compare with is skipped with `baseline.missing` without failing (ADR-205 §1); a rule skipped for any other reason fails the gate as before (ADR-205 §1). On a fresh repository whose change touches no source, all five pass (observed on a repository holding one text file).
- `"static.clones.pairs" = "<= base"` has no tolerance: a change that extends deliberately parallel code fails it (NOTES Q-java-2), and such a project relaxes its own table.
- The table is the project's to edit; `init --force` writes it again.
- `crates/cupel-cli/tests/init.rs` checks that the written file loads and that `check --changed --gate` evaluates exactly these rules; every `init --dry-run` golden shows the table.
