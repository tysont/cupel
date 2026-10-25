# CUPEL_DECISION_144: The dead-code module pass reads the whole language's import edges

**Date:** 2026-10-22 · **Status:** accepted · **Phase:** P9.F3

## Context

`static.dead-code`'s module pass (ADR-058) marks a module entered when an import edge from a file outside it leads to one of its files, and reports an exported symbol of a module nothing enters and nothing roots as `dead.export` (`info`, confidence 0.4, ADR-059). It reads its edges from `ModuleGraph`, whose producer resolves the scope's files only. Under `--changed` or `--base` the scope holds the changed files, so the edge from an unchanged importer to a touched file is not in the fact, and a touched `pub` item that only unchanged files import is reported `dead.export`: a finding invented, not missed (follow-up item 21, P9.6 open item 6, which ADR-143 does not defer). ADR-142 narrows a range to the symbols its changed lines touch, which limits the false finding to touched exports but does not remove it. ADR-058 recorded the caveat ("findings on a narrowed scope should be confirmed on the tree"); ADR-135 avoided it for the symbol pass by having `References` cover every file of the language whatever the scope.

The fixture case (`crates/cupel-fixtures/tests/dead_export_range.rs`): a commit edits `price` in Python's `src/shop/prices.py` (and `Price` in Go's `prices` package), which only the unchanged `checkout.py` (`main.go`) imports. `check --base main --only static.dead-code` reported `dead.export` on it in both languages; a tree run reports nothing on it.

## Options

**(a) `ModuleGraph` covers every file of the language, as `References` does.** The one change is in `ImportGraph::produce`, and every consumer sees the whole graph. The consumers are `static.dead-code`, `static.deps`, the mutation engine (`cupel-mutate`, which wants the graph to choose the tests a mutant's file can reach), and `tests --impacted` (its fallback when no `PerTestMap` exists). `static.deps` filters nothing by scope: it reports every cycle and layering violation in the fact and measures every module's coupling, so under a range it would report the tree's cycles and the tree's distances, and `dep.cycle` would stop meaning "a cycle among the changed files" (ADR-142 §5) unless `static_deps.rs` learned to filter by scope. The engine and `tests --impacted` would see importers outside the scope; that is arguably better test selection, but it changes two modules this step must leave alone, through a fact, without a test of either.

**(b) The dead-code check reads the whole language's edges from the fact that already covers the whole language.** The `References` producer already resolves every file of the language with the same resolver (`register_resolved` registers both producers around one `Resolve`), builds that `ModuleGraph` to resolve imported names, and drops it. It keeps its edges as `References.imports`, and the module pass reads them. Nothing else reads `References`, so no other consumer changes. The cost is a second copy of the edges in the store (the references goldens grow by each fixture's edges) and a field on a fact.

**(c) Derive importers from resolved references.** A reference whose target is `imported {file}` shows that its file imports the target. A module imported whole and used through members (`import ledger.report` then `ledger.report.summary()`, Rust `mod a;`, a Go package called as `prices.Price`) has no such reference, so the module pass would still invent findings for those.

**(d) Tell the check the scope is narrowed and say nothing about exports outside the tree scope.** Hides the true findings too: an export of a touched file that nothing imports anywhere would go unreported under a range.

## Decision

Option (b).

1. **`References.imports`**: every import edge of the files `References` covers (every file of the language, whatever the scope), as `ModuleGraph` holds them (`ImportEdge`), in the producer's order. The field is optional (`default`, absent from JSON when empty), as ADR-133's additions were, and the fact's version stays 1: a producer that does not fill it (a plugin, a hand-written test fact) still deserializes.
2. **The module pass reads the edges of both facts**: `ModuleGraph`'s (the scope's) and `References.imports` (the language's). A module is entered when either holds an edge into it from a file outside it; the unresolved and external specifiers that lower confidence (ADR-059's suspect imports) come from both. On a tree scope the two hold the same edges and nothing changes; under a range the importers outside the scope count. Reading `ModuleGraph` as well keeps the check correct with a `References` that carries no edges. The check's needs, kinds, evidence, and version are unchanged.
3. **Only the module pass changes.** Symbols, roots, and module namespaces still come from the scope's `Symbols` and `Roots`; findings are still reported only for symbols in scope. An importer outside the scope has no symbols in the fact, so it has no module namespace and always counts as outside the module it imports (the Go and Java same-package case below).

## Consequences

- A touched export that an unchanged file imports is no longer `dead.export` under a range (both fixture languages). A touched export that nothing in the repository imports is still reported, as on the tree.
- `static.deps`, the mutation engine, and `tests --impacted` see exactly what they saw: `ModuleGraph` is unchanged. Option (a) remains available if a consumer wants the whole graph; it would need its own filter by scope.
- The references fact goldens gain each fixture's import edges (the module-graph goldens' edges, over the same files); the fact schema gains the optional `imports`. No check golden moves on a tree scope.
- Still invisible under a range: a root outside the scope that keeps a multi-file module alive (a Go or Java package whose only test file, or only entry point, is unchanged reads as unrooted). Roots come from the scope's `Roots`; a Go package that something imports is entered and unaffected, so this needs a package nothing imports whose rooting file is unchanged. An import from an unchanged file of the same Go or Java package (legal in Java, redundant) now counts as entering it, which can only hide a finding.
- ADR-058's narrowed-scope caveat holds now only for roots, not for importers; ADR-135's note that "the module pass still reads only the scope's import edges" no longer holds.
