# CUPEL_DECISION_165: An import of types alone is a `type` edge, and a cycle follows only edges used at run time

**Date:** 2026-11-09 · **Status:** accepted · **Phase:** v0.2.6, Molten field trial Area 4 · Extends [068](CUPEL_DECISION_068.md) (edge kinds) and [052](CUPEL_DECISION_052.md) (cycles)

## Context

On the owner's Molten repository, `static.deps` reported seven import cycles. Five closed only through an `import type` edge:
- `agent-do.ts` ↔ `backup-workflow.ts` (both directions type-only);
- `logger.ts` ↔ `env.ts`;
- `constants.ts` ↔ `models.ts`;
- a three-module cycle whose runtime edges are acyclic;
- 21 skill modules that each import only `type DefaultSkill` back from the index that imports them.

TypeScript erases such imports. They create no module-initialization order and no runtime dependency, so none of these is a cycle a reader could break to any runtime effect. The graph had no way to tell such an import from a value import: every `import` was an `Import` edge.

## Options

- (a) Keep counting them. Cycles then report design coupling that the language says is not a dependency, and an agent would restructure code to cut a no-op edge.
- (b) Record type-only imports as their own edge kind, and leave them out of cycle detection only. They still count toward coupling and layering, which are about design.
- (c) Report type-only cycles as a separate, softer kind.

Chosen: (b). (c) needs a new finding kind and a reason someone would act on it; none has appeared.

## Decision

1. **`EdgeKind::Type`**: an import of types alone. It is any of:
   - `import type … from`;
   - `export type … from`;
   - a named import whose every name is marked `type` and that imports no default (`import { type A, type B }`).

   A mixed import (`import { type A, b }`) is an `Import`. The TypeScript query marks these with two capture-only patterns: `@import.type` on such a statement, and `@import.typed` on each typed specifier.
2. **Cycles follow run-time edges.** `static.deps` finds cycles, the shortest cycle it reports, and the cheapest import to cut over edges that are neither declarations (ADR-068) nor type-only. Afferent and efferent coupling, distance, and layer rules still count type-only edges.

## Consequences

- Molten's cycles: 7 → 2, both value-import cycles (`tools/builtin.ts` ↔ `tools/index.ts`, `extension-authoring.ts` ↔ `extension-drafting.ts`).
- A TypeScript `import type` across layers is still a `dep.layer-violation`: a type is part of a layer's design.
- `ModuleGraph` gains an edge kind. Stored facts carry their producer's version, so cached graphs are reproduced.
- JavaScript has no type-only imports, and its query is unchanged.
