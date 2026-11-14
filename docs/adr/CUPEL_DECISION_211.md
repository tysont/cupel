# CUPEL_DECISION_211: A Python import under a `TYPE_CHECKING` guard is a `type` edge

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.2.9, item V13 · Extends [165](CUPEL_DECISION_165.md) (type-only imports) to Python

## Context
On encode/httpx, `static.deps` reported two import cycles, `httpx/__init__.py → _client.py → _transports/default.py → httpx/__init__.py` and `_auth.py → _models.py → _types.py → _auth.py`. Each closes through an import under a guard that is false at run time: `_transports/default.py` has `if typing.TYPE_CHECKING:\n    import ssl\n    import httpx`, and `_types.py` has `if TYPE_CHECKING:` (from `from typing import TYPE_CHECKING`). `typing.TYPE_CHECKING` is `False` when the program runs and true only for a type checker (PEP 484), so these imports create no module-initialization order and no run-time dependency: both cycles are false. ADR-165 made such imports a `type` edge for TypeScript and said JavaScript has none; it left Python open.

Open: which Python imports are type-only, and whether they still count for uses other than cycles.

## Options
(a) Drop guarded imports from the module graph. Then a module named only for a type checker would look unimported to coupling, layering, and `static.dead-code`, though the names it defines are referenced in annotations. (b) Record them as `EdgeKind::Type`, as ADR-165 does for `import type`: cycles leave them out, every other reader keeps them. (c) Also follow the guard through a flag (`TYPE_CHECKING = False` defined by hand, or `from typing import TYPE_CHECKING as TC`): a data-flow question a query cannot answer, and not seen.

Chosen: (b).

## Decision
1. **What is type-only.** An `import` or `from … import` statement directly in the block of an `if` whose condition is the name `TYPE_CHECKING` or an attribute ending in it (`typing.TYPE_CHECKING`, `t.TYPE_CHECKING` after `import typing as t`). Python's `imports.scm` marks such a statement `@import.type`, so it is an edge of kind `type` (ADR-165 §1). Its `else:` branch, which runs, is unchanged; so is a condition naming the flag in a larger expression (`if TYPE_CHECKING or …:`) and an alias of the flag.
2. **What counts it.** As ADR-165 §2: `static.deps` finds cycles, the shortest one it reports, and the cheapest import to cut over run-time edges only; afferent and efferent coupling, distance, and layer rules still count type edges. `static.dead-code` reachability and the references resolver treat a type edge as they treat an import, so a module or name imported only for a type checker stays reachable: an annotation does use it.

## Consequences
- httpx's two cycles are not reported; a cycle of imports that run is.
- A type-only import across layers is still a `dep.layer-violation`, as in TypeScript.
- Fixture: `crates/cupel-fixtures/repos/type-checking-imports/python` (both httpx shapes and a run-time cycle beside them).
