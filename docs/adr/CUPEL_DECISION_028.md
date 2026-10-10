# CUPEL_DECISION_028: Module graphs: raw imports from queries, resolution per language, and Rust's rules

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P1.7

## Context

Design §8.4.1 makes `ModuleGraph` resolution hand-written per language, fed by `QueryImports`, and plan P1.7 lists Rust's inputs (the `mod` tree, `use` paths, workspace members) and outputs (`File | External | Unresolved` with a `relative` flag). Not specified: how the generic and hand-written halves meet, how a resolver sees files outside the scope (a module tree crosses them), how edges are grouped "per root" before `ProjectInfo` exists (P1.9), what `relative` means for Rust, and what `External.package` names.

## Options

**Interface.** A resolver could receive parsed trees (and re-run queries itself) or receive the raw imports `imports.scm` yields. Raw imports keep every language's resolver free of tree-sitter and make the capture contract the only syntax-facing part; the resolver additionally needs to read manifests and parse files outside the scope, through the parse cache.

**Roots.** Waiting for `ProjectInfo` would leave P1.7 without roots; deriving them inside each resolver from the manifests it already reads (Cargo packages, `go.mod`, `package.json`, Python roots) is local and P1.9 can later share the detection.

**Rust leniency.** Rust requires a dependency entry to name another crate; a resolver that also resolves workspace package names without one is more useful on half-edited code and never wrong about the file it points at.

## Decision

- `imports.scm` yields `RawImport { specifier, names, module, scope, line }` per specifier (contract in `cupel_lang::imports`). A language registers `ImportGraph::new(language, cache, resolve)` with its `resolve(&Imports) -> ModuleGraph`; `Imports` exposes the scope, file and directory existence, file text, and on-demand imports of any file under the root through the parse cache.
- `RootImports.root` is a package directory relative to the repository root (`.` for the root, and for files no package contains); every discovered package is listed, even without edges; edges are sorted by file, line, and specifier.
- Rust: packages, crate roots, `mod` resolution, and `use` resolution as documented in `cupel_lang::langs::rust::resolve`. One edge per path a use tree names, its specifier the expanded path without aliases (`crate::cli::{self, parse as p}` gives `crate::cli` and `crate::cli::parse`). `relative` is true for `mod` declarations and for paths through `self`, `super`, or a child module of the importing module; false for `crate::`, dependencies, and the standard library. `External.package` is the dependency's package name (`package = "…"` when renamed, else the manifest key), or the standard crate (`std`, `core`, `alloc`, `proc_macro`, `test`).

## Consequences

- Items re-exported through `pub use` resolve to the module that re-exports them, not to their definition; dependency analysis is file-level, which is what `static.deps` needs.
- `#[path = "…"]` on `mod` and macro-generated modules are not followed; such modules are `Unresolved` and visible as such.
- JavaScript/TypeScript, Go, and Python resolvers implement the same interface with their own manifests (`package.json` `exports`/`main` and `tsconfig` `paths`; `go.mod`/`go.work`; `__init__.py` and project roots).
