# CUPEL_DECISION_026: Mutation sites: operator sets, replacement tables, and the `site_hash` path

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P1.5

## Context

Design §8.4.3 names a `default` operator set (boolean flips, relational/equality swaps, `+/-`, `*//`, `&&/||`, `0/1`) and an opt-in `extended` set (statement deletion, return-value replacement, constant replacement, boundary ±1, negation). Plan P1.5 puts replacement spellings in `tables.toml` and requires `site_hash` to be "the structural position within the symbol, not the line", stable under moving and reformatting. ADR-025 fixed the hash length; this ADR fixes what the generic producer does with captures and what goes into the hash.

## Options

**Sets.** Declaring each operator's set per language invites drift (Python's relational swap in `extended`, Go's in `default`). Deriving the set from the operator keeps "default" meaning the same thing everywhere.

**Replacements.** A per-operator map from the site's exact text to replacements covers token swaps (`<` → `<=`, `and` → `or`, `True` → `False`); templates with `{}` for the original text cover wrapping (`!({})`, `{} + 1`) and fixed values (`Default::default()`, `None`, `""`). A query-side mechanism (one capture per replacement) would multiply patterns.

**`site_hash` inputs.** Line and column change with every edit above the site. A path of child indices from the symbol's definition node survives moves and layout changes (comments are tree-sitter extras and are skipped), and changes only when the code around the site changes structurally, which is the only time a recorded mutant outcome should be invalidated.

## Decision

- An operator's set is fixed by the operator: `relational`, `equality`, `arith-add-sub`, `arith-mul-div`, `boolean-op`, `boolean-literal`, `zero-one` are `default`; `statement-deletion`, `return-value`, `constant`, `boundary`, `negation` are `extended`.
- `[mutations.<operator>]` holds `replace` (exact text → replacements) and `templates` (`{}` is the original text). Replacements equal to the original, and duplicates, are dropped; a site with no replacement is not reported. Recommended spellings: relational swaps at the boundary (`<`↔`<=`, `>`↔`>=`), equality negates, `+`↔`-` and `*`↔`/` (compound assignments too), short-circuit operators swap, boolean literals flip, `0`↔`1`, `constant` replaces other integer literals by `0`, `boundary` replaces an integer literal compared against by `{} + 1` and `{} - 1`, `negation` wraps a branch condition in the language's negation, `return-value` replaces a returned value by the language's empty value, and `statement-deletion` replaces a statement evaluated for its effect by nothing.
- `site_hash` is the first 64 bits of `blake3("<operator as JSON>" ∥ "/<kind>:<index>"…)`, the steps running from the symbol's definition node down to the site node, `<index>` counting the parent's children excluding extras.
- Every symbol has a `MutationSites` entry, possibly empty; sites are ordered by start, outer before inner, then by operator. Every replacement must leave a file that re-parses without syntax errors; a property test checks this over the fixture corpus for each language that produces the fact.

## Consequences

- Mutants are syntactically valid but may not compile (`Default::default()` for a type without `Default`, a deleted `let`-free statement whose value was needed); the mutation engine (P3.4) reports those as non-viable rather than surviving.
- A site's hash changes when an earlier sibling statement is inserted in the same block; outcomes for that site are then recomputed. That is the cost of a purely structural identity.
