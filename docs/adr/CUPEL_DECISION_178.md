# CUPEL_DECISION_178: Abstractness counts exported functions as concrete

**Date:** 2026-11-11 · **Status:** accepted · **Phase:** v0.2.7, the owner's rulings on the Molten field trial · Amends [052](CUPEL_DECISION_052.md)

## Context
`dep.unstable-abstract` called `packages/shared/extensions/validate.ts` (42 functions, 1 exported interface) fully abstract (A = 1.0), because A counted interfaces and types over public types only.

## Options
(a) Types only, as before. (b) Exported functions count as concrete members (the owner's ruling). (c) Count every top-level item: makes unexported helpers move a module's position.

## Decision
A module's abstractness A is abstract exported members over all exported members. Abstract: interfaces, abstract classes, and type-only exports (type aliases, `export type`). Concrete: exported functions, concrete classes, and exported values.

**Adaptation from Martin's metric.** Martin defined A over the classes of a package (abstract classes and interfaces over all classes), for languages where all behavior lives in classes. In module-based languages most behavior is free functions, so leaving them out reads a module of functions as fully abstract. Counting exported functions as concrete keeps the metric's intent: how much of what a module offers is contract rather than implementation.

## Consequences
A, and with it distance and `dep.unstable-abstract`, changes for every module that exports functions. Languages whose exports are all classes are unaffected. v0.2.7 implements it; the plan's batch v0.2.7 Notes record the implementation and its tests.
