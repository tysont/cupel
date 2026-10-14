# CUPEL_DECISION_055: `test.duplicate-setup` compares the leading statements of test bodies in `NormTree`

**Date:** 2026-10-13 · **Status:** accepted · **Phase:** P2.6

## Context

Design §5.2's `tests.quality` reports duplicated setup (`test.duplicate-setup`), SCRAP's "duplicated scaffolding" (design §3). A check reads facts only (plan §2.8), so it cannot read setup text. `TestFrames` lists a file's declared setup blocks (`beforeEach`, fixtures, `setUp`, `TestMain`) as spans, but those are already extracted setup, and JavaScript and TypeScript hooks are not symbols (ADR-030), so they have no normalized tree to compare. The duplication worth reporting is the arrange code each test repeats instead of sharing.

## Options

**Declared setup blocks.** Compare the `NormTree` of each setup block that is a symbol. Finds two identical fixtures, which is rare, and cannot see JavaScript or TypeScript hooks at all.

**Normalized setup text.** Not available: checks never read files, and no fact carries source text of a block.

**Leading statements of test bodies.** Every test case is a symbol with a `NormTree` in all five languages. Identifiers and literals are erased and call names, operators, and shape are kept (ADR-025), so two tests that open the same account and make the same deposits with different values have equal leading statements. The tree's labels are grammar node kinds, so the check must find the statement list without naming any.

## Decision

- **Statements of a body.** The body is the last child of the symbol's normalized root (a function's body follows its parameters in every grammar). Its statements are the body's children, or, when the body has exactly one child, that child's children (Go wraps statements in one list node; a one-statement body then yields that statement's parts). A list with any leaf node is not a statement list (statements are never leaves; a declaration's parts include leaf tokens such as `=`), and gives no statements.
- **Groups.** Within one test file, cases whose first `setup_statements_min` statements are equal (default 3) form a group, provided each has more statements than that, so the repeated part is followed by something the tests do differently (identical whole tests are `static.clones`' business). A group of two or more cases is one finding.
- **The finding** is about the group's first case by position, so its ID is stable while that case keeps its structure. Evidence: `remediation_mode = "extract-fixture"`, `actionability = "mechanical"`, `statements` (the leading statements every member shares, at least the minimum and fewer than any member's total), `setup_statements_min`, `tests` (each member's display name, in file order), `lines` (each member's first line).

## Consequences

- Works for every language with `NormTree` and needs no fact change.
- Comparison is structural: two tests that build different objects through the same calls and shapes are duplicates, which is the point of the extract-fixture remedy, and two tests that call different constructors are not.
- The statement-list rule is a heuristic over tree shape: a language whose body node holds non-statement children beside its statements, or wraps them more than once, finds no statements and never reports duplicate setup; the fixtures' goldens pin the rule for all five languages.
- Setup shared across files is not reported; a shared helper across test files is a different remedy.
