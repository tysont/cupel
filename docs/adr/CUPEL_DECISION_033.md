# CUPEL_DECISION_033: Go module graphs and API surface: package imports as file edges, receiver-qualified API paths

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P1.7–P1.8

## Context

ADR-028 makes `ModuleGraph` edges point at files (`File { path }`), but a Go import names a package, which is a directory of files. ADR-029 builds API paths from lexical `@scope`s, but a Go method is declared at the top level, outside its receiver type, so `Account.Deposit` and `Notifier.Deposit` would share the path `ledger.Deposit`. ADR-029's token-joining rule also renders Go types badly (`[] int64`, `map[string] int`, `(_[]int64)`).

## Options

**Package targets.** (a) A `File` whose path is the package directory: breaks the meaning of `File` for every consumer. (b) One edge per non-test `.go` file of the package. (c) A new `Package` target: a fact change and a `VERSION` bump for one language.

**External names.** The standard library as each package path (`fmt`, `net/http`) or as one name; a third-party import as its package path or as the required module that provides it.

**Receiver segment in API paths.** (a) A hand-written Go `ApiSurface` producer duplicating `QueryApi`. (b) Reuse the mechanism `symbols.scm` already has: an `@scope.name` in the item's own match adds one non-lexical innermost segment. `api.scm` already permits `@scope.name`, and no existing `api.scm` puts one in an item's match.

**Signature spacing.** (a) Leave Go signatures with stray spaces. (b) Change the shared rule: Rust needs a space after `]` (`-> [u8; 4] where`). (c) A per-language `[api] tight_after` list, empty by default.

## Decision

- An import of a package inside any module found in the repository (the root `go.mod`, the root `go.work`'s `use` directories, and the nearest `go.mod` above each file; the longest module path wins) yields one `File` edge per non-test `.go` file directly in the package directory, in path order, all with the import's specifier and line; a directory without such files is `Unresolved`. Repository modules resolve whether or not the importer requires them, as Rust resolves workspace packages without a dependency entry.
- A path whose first element has no dot is the standard library: `External { package: "std" }`, as Rust reports `std`. Any other path is `External` with the longest module the importer's `go.mod` requires as its prefix, else `Unresolved`. `replace` directives are not read. No Go edge is `relative`: module mode has no relative imports, and `conformance/module-graph.toml`'s `relative` construct is not applicable.
- Roots are module directories, with `.` for files outside every module.
- `api.scm` items honor an `@scope.name` in their own match as the innermost path segment (generic, additive). Go method paths are `package.Receiver.Method`, and a method's signature spans from its receiver to its result (`(_ *Account) Deposit(_ int64)`), so a pointer-to-value receiver change is an API change.
- `[api] tight_after` (default empty) lists tokens after which signatures have no space; Go lists `]` and `...`. The `_` that stands for a local is no longer treated as a name before `(`, `[`, or `<` (generic; no Rust signature contains that sequence).
- Go reports capitalized package-level functions (except `Test`, `Benchmark`, `Example`, and `Fuzz` functions), types, aliases, interfaces with their capitalized methods, constants, and variables, and capitalized methods of capitalized receiver types. Packages are not API items (`module` is not applicable): Go declares none in source.

## Consequences

- `static.deps` sees a package import as edges to every file of the package, which over-approximates file-level dependency but never misses one; `static.dead-code` sees every file of an imported package as reachable.
- Exported items in `_test.go` files (test doubles, helpers) appear in `ApiSurface` because queries cannot see paths; filtering them needs `TestConventions`, a cross-fact join that belongs in the check, not the producer.
- A constant or variable spec declaring several names (`const A, B = 1, 2`) reports only its first name, since the earliest pattern wins per item node.
- Build constraints (`//go:build`, `_linux.go`) are ignored: every non-test file of a package is a target.
