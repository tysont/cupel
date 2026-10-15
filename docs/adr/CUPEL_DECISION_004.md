# CUPEL_DECISION_004: Equivalent mutants in v1

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P3.4

## Context

A mutant is equivalent when no test could kill it: the mutated program behaves exactly as the original. Equivalent mutants survive every suite, so reporting them as `mutant.survived` asks an agent to write a test that cannot exist and lowers the score for nothing. Deciding equivalence is undecidable in general. Design open question 2 asks whether v1 ships a low-confidence `mutant.equivalent?` kind or waits for a classifier, leaning toward emitting it at `confidence ≤ 0.3`, `severity: info`. The `default` operator set (design §8.4.3) is chosen to keep the rate low, but it still mutates `x + 0`, `x - 0`, `x * 1`, and `x / 1`, whose operator swap never changes a value.

## Options

**No kind until a classifier exists.** Every survivor is `mutant.survived`; agents meet identity mutants as unkillable survivors.

**Trivial compiler equivalence.** Compile the mutant and compare the binary with the original's; identical binaries are equivalent. Needs a compiled language and a reproducible build per mutant, which the file-copy engine does not have (schemata are P4.1).

**A lexical identity rule.** The engine marks a mutant as possibly equivalent when the operator's right operand is the identity of both the original and the replacement operator: `+`/`-` (and `+=`/`-=`) with a right operand that is the literal `0`; `*`/`/` (and `*=`/`/=`) with a right operand that is the literal `1`. It reads only the source text after the site, so it needs no language knowledge beyond what the sites already carry. It misses every equivalent mutant of another shape.

## Decision

The lexical identity rule, in the engine. A mutant it matches carries `equivalent: Some(reason)` in `MutationOutcomes` (ADR-080), whatever its outcome. `tests.mutation` reports a *surviving* marked mutant as `mutant.equivalent?` — `severity: info`, `confidence: 0.3`, action `review` — instead of `mutant.survived`, and leaves it out of the score (`killed / (killed + survived)` counts unmarked survivors only). A marked mutant that was killed is a kill like any other: the rule was wrong about it, and the evidence of the kill wins.

## Consequences

- Agents are not sent to kill `x + 0 → x - 0`.
- Equivalent mutants of other shapes (a relational boundary on a value that never reaches it, a mutation in dead code) are still `mutant.survived`; a classifier, or trivial compiler equivalence on schemata builds (P4.1), can add to the rule without changing the fact.
- The mark is the engine's; another producer of `MutationOutcomes` (a PIT wrapper) may mark mutants by its own rule and the check treats them the same.
