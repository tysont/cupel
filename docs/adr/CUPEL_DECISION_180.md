# CUPEL_DECISION_180: String and regex mutation operators, in the extended set, off by default

**Date:** 2026-11-11 · **Status:** accepted · **Phase:** v0.2.7, the owner's rulings on the Molten field trial · Amends [026](CUPEL_DECISION_026.md)

## Context
Neither operator set mutated string or regex literals, so escaping and quoting helpers (Molten's `shellQuote`) got no default mutants. ADR-026 left them out to keep equivalent mutants down.

## Options
(a) Leave them out. (b) Add them to the default set: many equivalent mutants (log messages, labels). (c) Add them to the extended set, opt-in, off by default (the owner's ruling).

## Decision
A `strings` operator group joins the extended set and is off unless enabled: a non-empty string literal to `""`, and in regex literals, quantifier edits (`*`↔`+`, `?` removed, `{n,m}` to `{n}`) and class edits (a negated class un-negated, a class's range narrowed to its first member). Enabling it is explicit configuration; `--depth deep` does not turn it on.

## Consequences
Default mutation scores do not change. v0.2.7 implements it; the plan's batch v0.2.7 Notes record the implementation and its tests.
