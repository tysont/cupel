# CUPEL_DECISION_058: `static.dead-code` reachability: files and modules, not calls

**Date:** 2026-10-13 · **Status:** accepted · **Phase:** P2.8

## Context

Plan P2.8 asks for unreferenced symbols and exports "using the symbol table, the resolved import graph, and `roots()` as the live set; conservative". A symbol is dead when nothing that runs can reach it. Deciding that needs references: which symbol calls or names which. What the facts carry today:

- `Symbols`: every function, method, and named closure with its file, span, namespace, kind, and visibility. No references.
- `ModuleGraph`: one edge per import, from a file to a `File`, `External`, or `Unresolved` target, with the specifier as written. Edges are file-to-file; the imported *names* (`RawImport.names`) do not survive into the fact. Rust and Python specifiers happen to end in an item name (`crate::report::format::render`, `ledger.decide.parse_cents`), JavaScript and TypeScript specifiers never do (`./format.js`), and Go imports name packages.
- `Roots`: symbols live by convention, by reason (`entry-point`, `export`, `test`, `framework-hook`).
- `NormTree`: the structural hash (finding identity, ADR-014). Its identifier labels are normalized to `$id`, so names are gone.

No fact says that `balance` calls `scale` in the same file, or that a Go file calls a function of another file in its package. The facts cannot see a single intra-file reference, nor any reference between files of one Go package. A symbol-level answer ("this private helper has no caller") is therefore not available without a new fact.

## Options

**A `References` fact** (per symbol, the names it mentions, resolved where possible). The honest answer for symbol-level dead code, but it is a new fact kind: plan §2.5 requires an ADR and producers for all five languages, which is outside P2.8.

**Symbol-level guesses over existing facts** (a private symbol with no root is dead). Every private helper called by a public function in the same file would be reported. Wrong on most of every fixture.

**File-level reachability over existing facts.** Treat what the facts can see — imports between files — as the only references, and the file as the smallest unit whose liveness is known: since calls inside a file are invisible, any live symbol in a file may call any other, so a file with one live symbol keeps all of its symbols. Report only symbols in files that nothing can be seen to reach.

**Following import edges transitively from roots** (a breadth-first search from entry points and tests) finds dead clusters (two orphans importing each other) but marks everything a library exposes, and everything only the library imports, as unreached whenever nothing in the repository runs it.

## Decision

`static.dead-code` decides reachability per **module** from imports and roots, never per call:

1. **Module.** A file's module namespace is the shortest namespace among its symbols, after removing the last segment from a method's namespace (its receiver or class). The segment separator is derived from the symbols themselves (`display` = `namespace` + separator + `name`), so no language is named. Files with the same module namespace form one module: a Go package's files share their package clause, while Rust, Python, JavaScript, and TypeScript namespaces follow paths, so their modules are single files (two packages with a same-named module merge, which only hides findings).
2. **Entered.** A module is entered when an import edge from a file outside the module targets one of its files. Edges from files with no symbols count (`src/index.ts` re-exporting `geometry`).
3. **Rooted.** A module is rooted when one of its symbols is a root for a reason other than `export`: an entry point, a test, or a framework hook. Hooks count because a framework, not an import, loads their file.
4. **Dead.** A module that is neither entered nor rooted is unreachable as far as the facts show. If it has exported symbols, those exports are `dead.export` findings and its other symbols are not reported: an export may be a library's public API, and anything the module keeps private may be what that API calls (ADR-059). If it has none, every symbol in it is a `dead.symbol` finding. Only symbols not nested in another symbol are reported (a dead function's closures are part of it).
5. Only symbols in the check's scope are reported; the analysis reads every symbol and edge the facts carry.

What the check can see: imports between files (resolved by the language's resolver), the roots and visibility each language assigns, and namespaces. What it cannot see, recorded so the findings' `detail` and confidence can say so:

- **Calls.** References inside a file and between files of one module. A private function nobody calls, in a file that something imports, is never reported (Rust fixture `src/lib.rs::scale` is such a function and goes unreported).
- **Imported names.** An imported file keeps every symbol live, even those no importer names.
- **Dead clusters.** Two unreachable files that import each other keep each other live.
- **Implicit loading.** An import resolves to one file, but a language may run others with it: importing a Python submodule runs its package's `__init__.py`, whose edge goes to the submodule. Such a file is reported unless something imports the package itself (the Python fixture's `ledger.report.summary`, which nothing calls, is reported as an unimported export).
- **Code outside symbols.** Module-level statements, scripts run directly, files a tool loads by convention (configuration files, plugin directories), and dynamic loading by computed name (`importlib`, `require(variable)`, `getattr`, string-keyed registries, reflection) are invisible. An `Unresolved` or `External` import whose specifier names the file is the one trace of such loading the facts carry; ADR-059 lowers confidence on it.
- **Narrowed scopes.** Producers build `ModuleGraph` over the files in scope, so under `--paths` or `--changed` a file's importers outside the scope are invisible and the file reads as unimported. `CheckCtx` does not say whether the scope is the whole tree, so the check cannot tell; findings on a narrowed scope should be confirmed on the tree. `tests.weakening` (P2.7) also needs the scope mode, and a shared field on `CheckCtx` is the proposed fix.

## Consequences

- No false positives from invisible intra-file calls, at the cost of finding only unreachable files and packages. On the fixtures every `dead.symbol` is in a file nothing imports and that holds no root.
- The proposed `References` fact (per symbol: names referenced, and for each import the names it binds) would let the check report unreferenced symbols inside live files, honor named imports, and follow calls transitively from roots. It is recorded here for P6.5 (extension drill: adding a fact kind) or P9 (dogfood findings) to revisit, with its ADR and all five producers.
- A Go file whose package is imported is live as a whole; only packages nothing imports can contain dead code, matching ADR-033's note that `static.dead-code` sees every file of an imported package as reachable.
