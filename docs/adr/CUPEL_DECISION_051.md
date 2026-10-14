# CUPEL_DECISION_051: Family-wide checks: one task per language family, with every member's facts

**Date:** 2026-10-13 · **Status:** accepted · **Phase:** P2.4

## Context

Design §8.3.5 says multi-language scopes run each check once per language present, "except where the fact is cross-language by nature (`NormTree` comparisons within a family)", and §8.4.3 says a mixed JavaScript and TypeScript tree has clones detected across the two languages because they share the `ecma` family. Scheduler v1 (ADR-041) makes one task per supported (check, language) and builds each task's `Facts` from that language's productions only, so `static.clones` running for `typescript` cannot see a JavaScript symbol. Nothing in `Manifest`, `Check`, or `Facts` expresses "this check compares across a family".

## Options

**A manifest field** (`spans_family: bool`). Visible in `describe`, and a plugin check could declare it. But `Manifest` is serialized into every `describe` golden and the run's dry-run plan, so every check's description changes for one check's need, and plugins do not exist until P5.1.

**A provided `Check` method** (`fn spans_family(&self) -> bool { false }`), like `combine` (ADR-041). Additive: no existing check, golden, or schema changes. Plugins would need it in their manifest later, exactly as ADR-041 already notes for `combine`.

**Merging payloads in the scheduler** (one `Symbols` holding both languages' symbols). The scheduler would need to know how to merge each payload type, which is per-kind code in core for every fact a family-wide check might need.

**Serving the other members' facts beside the task's own.** `Facts::get::<F>()` keeps meaning "this task's language"; a new `Facts::family::<F>()` returns this language's fact followed by each other member's. The check iterates; the scheduler stays payload-agnostic.

## Decision

- `Check::spans_family(&self) -> bool`, provided, default `false`. `static.clones` returns `true`.
- For a family-wide check, the scheduler groups the languages in scope that the support matrix marks `supported` by `Lang.family` and makes one task per family. The task's language is the family's first member in ID order (`javascript` before `typescript`); its scope is the union of the members' scopes (files are shared and `SymbolKey`s carry the file, so the union is unambiguous); its configuration is the check's slice for that first language, per-language override included. Unsupported members are still skipped per language with the matrix's reason, as before.
- `run_task` serves the task language's facts as today, then produces each `needs` kind for every other member and adds it with `Facts::insert_shared_sibling`; a member whose needed fact fails makes the task `error`, as a failed need does today. `wants` of other members are added when they can be produced.
- `Facts` gains `insert_sibling<F>`, `insert_shared_sibling`, and `family::<F>() -> Result<Vec<&F>>` (own fact first, then members in language-ID order). Both refuse undeclared kinds, so the matrix test's "only declared facts" guarantee holds for siblings too.
- Results are unchanged in shape: a family task contributes one per-language result to `combine`, so a family-wide check's combined entry is still one per check.

## Consequences

- JavaScript and TypeScript symbols are compared in one `static.clones` run; Go and Python, being different families, never meet.
- A per-language override under `[checks."static.clones".lang.typescript]` is ignored when JavaScript is also in scope (the family's slice is JavaScript's). Design §8.4.3's "where only one is given it applies to the whole `ecma` family" is for `[lang.*]` tables; per-family check overrides can follow if anyone needs them.
- `describe` does not yet say a check is family-wide; a manifest field can add that when plugin checks (P5.1) need to declare it.
- The matrix test's `run_with_declared_facts` serves one language and no siblings, which a family-wide check must handle (`family` then returns one fact).
