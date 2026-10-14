# CUPEL_DECISION_072: `static.complexity` uses modified cyclomatic complexity: a match counts once

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P2.10

## Context

On the first Cupel dogfood `complexity.high` fired on functions whose only complexity is one exhaustive `match` over an enum: `VerbArgs::verb` (cyclomatic 12, cognitive 1), the text renderer's `description::render` (cyclomatic 11, cognitive 1), `FactKind::as_str` (cyclomatic 21, cognitive 1). Each arm maps one variant to one value; the function reads as a table. Cyclomatic complexity counts every arm as a decision (ADR-024 gives each non-default arm cyclomatic weight 1), so any dispatch over more than ten variants is "too complex", while cognitive complexity, which ADR-024 already modelled on SonarSource's rules, counts the match once. Rewriting such matches to dodge the metric (lookup tables over enums carrying data, splitting a dispatch in two) makes the code worse, so the finding is a false positive of the check, not a fault of the code.

## Options

1. Keep standard cyclomatic complexity for `complexity.high` and raise `cyclomatic_max` where needed: hides genuinely branchy functions too.
2. Fire `complexity.high` on cognitive complexity alone: drops the cyclomatic signal entirely.
3. Use *modified* cyclomatic complexity (NIST SP 500-235's variant, `lizard --modified`): a multiway branch (`match`, `switch`, Python `match`) counts as one decision however many arms it has. Arms still count for CRAP, whose cyclomatic term estimates the tests a function needs.

## Decision

Option 3.

- `Decision` gains `continues: bool`: true for a `case` arm after the first among its siblings (the same rule that already gives later arms no cognitive weight), false for every other decision. Produced generically by `cupel_lang::decisions` for all five languages; `Decisions` stays at version 2 (this step introduced it).
- `static.complexity` (version 2) computes cyclomatic complexity as `1 + Σ cyclomatic` over decisions that do not continue a multiway decision. This value is what `complexity.high` compares with `cyclomatic_max`, what its evidence and title report, the `cyclomatic_max` gate metric, and the per-symbol `cyclomatic` ranking; the manifest and metric summaries say "modified".
- `static.crap` keeps standard cyclomatic complexity (`metrics::cyclomatic`), so its CRAP values are unchanged.
- Each language's complexity fixture gains a twelve-arm month-name dispatch that must not be `complexity.high`.

## Consequences

- A function with one large flat dispatch is no longer reported on cyclomatic grounds; arms that contain branches still add those branches, and cognitive complexity still reports deep or tangled matches.
- `static.complexity` and `static.crap` report different cyclomatic numbers for a function with a match; each check's evidence documents which it uses.
