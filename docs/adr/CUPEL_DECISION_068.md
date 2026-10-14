# CUPEL_DECISION_068: Module-graph edges carry a kind: import, declaration, re-export

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P2.10

## Context

`ModuleGraph` edges (ADR-028) did not say what a statement does with the module it names. Rust's `mod a;` became an edge from parent to child, so a child that uses anything from its parent (`use crate::Error`, `use super::LIMIT`) formed a two-module cycle with it, and `static.deps` reported every such parent and child as `dep.cycle` (11 cycles on the first Cupel dogfood, one component of 25 files in `cupel-core`). The P2.5 notes recorded that telling declarations from uses needs a field on the edge in every language. The project owner ruled for an `edge_kind` field with `import | declaration | re-export`, produced by all five languages, and that cycle detection ignores declarations.

## Options

1. Drop declaration edges from `ModuleGraph`. `static.dead-code` needs them: a Rust file is live because a live module declares it (ADR-058).
2. A boolean `declaration` flag. It cannot say "re-export", which a reader of the graph (a future API or layering rule) wants to tell from a use.
3. An `edge_kind` enum on every edge, decided by the language's `imports.scm` through generic captures, so no check names a language.

## Decision

Option 3.

- `ImportEdge.edge_kind: EdgeKind` with values `import` (uses the module), `declaration` (declares a child module: structure, not use), `re-export` (imports names in order to export them again). `ModuleGraph` is version 2.
- `imports.scm` decides it with generic captures (`cupel_lang::imports`): `@import.module` (existing; Rust `mod a;`) → `declaration`; new `@import.reexport` on a statement that re-exports (Rust `use` with any visibility modifier, `pub use`/`pub(crate) use`; ECMAScript `export … from`) → `re-export`; new `@reexport.name` on names a file exports (Python `__all__` list or tuple entries), so an import whose `@import.name` is such a name is a `re-export`; everything else `import`. Go has neither declarations nor re-exports; every Go edge is an `import`.
- `static.deps` follows only edges that use a module (`import` or `re-export`) when it looks for cycles and the shortest cycle path; afferent/efferent coupling and layering still count every `File` edge, as ADR-052 defined them (a declaration is a dependency of the parent on the child). `static.dead-code` keeps reading every edge.
- Conformance: `module-graph` gains the constructs `declaration` (Rust; `not-applicable` elsewhere) and `re-export` (Rust, Python, JavaScript, TypeScript; `not-applicable` for Go). The Rust fixture gains `src/deps_tree/` (a parent declaring a child that uses the parent's constant), which must not be a cycle.

## Consequences

- A Rust child importing from its parent is no longer a cycle; a real use cycle through a parent (`lib.rs` *using* a child that uses `lib.rs`) still is.
- A re-export is a use: `pub use child::Thing` in `lib.rs` plus `use crate::Error` in `child.rs` is a cycle, as it should be (the parent cannot be understood without the child, nor the child without the parent).
- Python re-exports are recognised only through a literal `__all__` assigned at module level; `from .a import b as b` (the typing-convention re-export) is an `import`.
