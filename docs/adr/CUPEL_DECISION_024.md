# CUPEL_DECISION_024: Decision weights, nesting, and what counts as an early return

**Date:** 2026-10-10 · **Status:** accepted; the early-return rule amended on 2026-10-26 (P9.9 group 3, [ADR-138](CUPEL_DECISION_138.md) §10): a return that ends an anonymous function of `[decisions] functions` inside the symbol is final there · **Phase:** P1.3

## Context

Design §8.3.2 gives `Decision { kind, nesting }` and P0.2 added per-decision `cyclomatic` and `cognitive` weights; plan P1.3 says the weights follow "crapper's per-language tables". Those tables are not available to this project, and the design does not say how nesting treats else-if chains, decisions in a condition, switch arms, or which returns are early. Every language's `decisions.scm` and every complexity check depend on these answers.

## Options

**Weights.** (a) Per-language tables with no shared default: every language agent re-decides the same numbers. (b) One default per kind, overridable per language in `[decisions.weights]`: languages agree unless a construct genuinely differs.

**Nesting.** (a) Count every enclosing decision node: an `else if` would nest under the `if` it continues and `a && b` in an `if` condition would nest inside that `if`, inflating cognitive complexity for idiomatic code. (b) SonarSource's cognitive-complexity rules, expressed structurally: else-if chains are flat and condition operands do not nest. Requires two pieces of grammar knowledge per language: which parent node kinds mark a chain link, and which field holds a condition.

**Switches.** Cyclomatic complexity counts each arm; cognitive complexity counts the switch once. With one decision per arm, either the cognitive weight is per arm (over-counting) or only the first arm of a switch carries it.

**Early returns.** (a) Every `return`: a function's single final `return` would count. (b) Returns that are not the final statement of the body.

## Decision

- Default weights (`cyclomatic`, `cognitive`, `nests`): `branch`, `loop`, `case`, `catch`, `ternary` (1, 1, yes); `boolean-op` (1, 1, no); `early-return` (0, 1, no). `[decisions.weights.<kind>]` overrides a kind for one language. An early return does not add a path through the control-flow graph that its enclosing branch has not already counted, so it carries no cyclomatic weight; it does make code harder to follow.
- A switch or match contributes one `case` decision per non-default arm; only the first arm among siblings keeps its cognitive weight.
- Nesting counts enclosing decisions of nesting kinds within the owning symbol, skipping an enclosing decision when the path to it runs through one of its `condition_fields` (default `["condition"]`), and skipping the branch an else-if continues: a branch whose parent's kind is in `chain_parents` (Rust and JavaScript `else_clause`; Python's `elif_clause` sits under `if_statement`) is not nested under that branch, and neither is anything inside it.
- An `early-return` capture is dropped when it is the body's final statement: every node from it up to the body is its parent's last named, non-comment child. Rust's `?` is an early return.
- A decision belongs to the innermost symbol containing it; every symbol gets a `Decisions` entry, possibly empty, in symbol order.

## Consequences

- `static.complexity` (P2.1) computes `CC = 1 + Σ cyclomatic` and cognitive as `Σ (cognitive + nesting)` over nesting kinds and `Σ cognitive` over the rest; the fact already carries everything it needs.
- A conditional `return x` as the last statement of a body is final by this rule (Python `if c: return x` at the end of a function), which slightly under-counts; the rule is structural and needs no flow analysis.
- Numbers may diverge from crapper's; if its tables become available, a language overrides kinds in `[decisions.weights]` without a code change.
