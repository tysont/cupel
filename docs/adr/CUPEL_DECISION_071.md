# CUPEL_DECISION_071: A Rust loop over a constant is a table of cases

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P2.10

## Context

ADR-069 recognises loops over a table written in place as parametrization rather than `test.logic`, and noted that a table held in a `const` or `static` item was not recognised. Cupel's own tests iterate over such tables (`for kind in FactKind::ALL`, `for (name, generate) in schema::PUBLISHED`, `for (id, paths) in GRAMMAR_PATHS`): exhaustive tables of cases declared once and shared between tests, whose items are literals by construction (a Rust `const` is evaluated at compile time).

## Decision

The Rust `decisions.scm` also marks as `@loop.table` a `for` over a path or identifier whose last segment is SCREAMING_CASE (`CASES`, `Kind::ALL`, `&schema::PUBLISHED`, or `.iter()`/`.into_iter()` of one). rustc's `non_upper_case_globals` lint makes SCREAMING_CASE the spelling of `const` and `static` items, so the name identifies the item kind without type information. The Rust fixture gains a test iterating over a `const` table; it must not be `test.logic`.

## Consequences

- A function returning a table (`Kind::all()`) is still not recognised; neither is a SCREAMING_CASE local, which the lint flags anyway.
- Other languages keep ADR-069's rule: their constant conventions are not enforced, so a capitalised name says nothing about the value.
