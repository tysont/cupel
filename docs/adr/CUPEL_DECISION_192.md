# CUPEL_DECISION_192: How `ApiSurface` reads a JavaScript or TypeScript package's entries at each side of a range

**Date:** 2026-11-12 · **Status:** accepted · **Phase:** v0.2.7, item 13 · Details [176](CUPEL_DECISION_176.md), [174](CUPEL_DECISION_174.md), [056](CUPEL_DECISION_056.md)

## Context
ADR-176 rules that a JavaScript or TypeScript package's API is what its `package.json` `exports` map exposes, or without one its `main` and `types` (or `typings`), with their re-exports, read from the manifest at each side of the range. It leaves open where the rule lives, how an `exports` map's forms (strings, subpaths, conditions, wildcards) and built output map to source files, and how the base side reads a manifest and entry files that the range did not change. ADR-056's base tree holds only the base versions of the changed files a language owns, so neither an unchanged entry nor a `package.json` is there.

`ApiSurface` has other readers (`static.deps`' abstractness, `gen.property`, `gen.differential`), which ADR-176 does not rule on.

## Options
**Where.** (a) The check reads manifests: checks see only facts, and the base side's files are not facts. (b) The producer drops internal items: changes every other reader of `ApiSurface`. (c) The producer marks them, and `static.api-surface` leaves marked items out: the other readers see what they saw.

**The base side's other files.** (a) Check out every file of each affected package at the base: one `git show` per file, hundreds in a large package. (b) Read them with git at the base commit: every probe of the resolver (a file, its TypeScript source, each extension) becomes a git lookup. (c) A path the range did not change is the same at the base as in the repository, so read the base tree for changed paths and the repository for the rest; the base tree also holds the changed manifests.

**Built output.** (a) Read `tsconfig.json`'s `outDir` and `rootDir`: exact for `tsc` builds, not for bundlers. (b) A target that names no file stands for the same path under `src`: cheap, and the layout most packages use. (c) None, falling back to `types`: a `types` under `dist` maps no better.

## Decision
1. **A mark on the item.** `ApiItem` gains `internal` (optional, absent when false). The `ApiSurface` producer of JavaScript and TypeScript sets it, after merging the files' items, on each item the entries of its package do not reach (`Language::mark_internal`, a no-op for every other language). `static.api-surface` leaves internal items off both sides.
2. **The package** of a file is the nearest directory above it with a `package.json`. Its entries are every target string of `exports` (a string, an array, subpath keys, and conditions, all of them and nested; `null` ignored), else `main`, `types`, and `typings`. No manifest, or none of these keys, is no entries.
3. **Targets** resolve as imports do (ADR-068's resolver: the file, the TypeScript source of a `.js` name, an extension, a directory's index). A target naming no file is built output: its extension (`.d.ts`, `.js`, …) is dropped and its leading directories replaced by `src`, one more at a time (`./dist/esm/index.d.ts` → `src/esm/index`, then `src/index`), until one resolves. A wildcard target exposes every file of the language it matches, `*` matching across directories; a built one is matched under `src` the same way, by path without extension.
4. **Names.** An entry exposes every name it exports. A re-export (`export … from`, ADR-174's edges) of an exposed file exposes what it names of its target, transitively: `export *` passes on what its file exposes, `export { a as b }` exposes `a` when `b` is exposed. An item is exposed when its file is and its top-level name (its path's first segment after the file's namespace) is.
5. **Each side.** The base tree also holds the base version of every changed manifest a language's ecosystem reads (`package.json`, …), not listed among its files. A producer on the base side gets `Focus::base_side`: the repository root and every path the range changed. Its file view (`Imports`) reads a changed path from the base tree (absent if the base tree lacks it) and every other path from the repository.

## Consequences
- Area 11's move inside `apps/agent` reports nothing; removing an item an entry re-exports is still `api.breaking`, and a range that drops a subpath from `exports` removes what only it exposed.
- A JavaScript or TypeScript file under no manifest, or in a package whose manifest names no entries (a workspace root with only `workspaces`), has no API surface.
- Not followed: `import { f } from './x'; export { f }` (an export of an import, not `export … from`), `export *` from a bare specifier that resolves to no file, a package that re-exports another package's internal file (an item counts only its own package's entries), `exports` exclusions (`null` targets do not hide what another target exposes), and built output whose source is not under `src`.
- `--paths` narrows the range's changes, so a changed manifest outside it is read from the repository on the base side.
- **Tests:** `cupel-lang/tests/ecma_entries.rs` (an `exports` map with subpaths and conditions, `main`/`types`/`typings` and built output, wildcards, the nearest manifest, no manifest, other languages unmarked); `cupel-checks/tests/static_api_surface.rs` (internal items left out); `cupel-fixtures/tests/api_surface.rs` (the Molten move, a removed re-exported item, the manifest read at each side, through the binary with `--base` and `--changed`).
