# CUPEL_DECISION_031: JavaScript and TypeScript module resolution and public API

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P1.7–P1.8

## Context

ADR-028 fixed the resolver interface and Rust's rules and left JavaScript and TypeScript to "`package.json` `exports`/`main` and `tsconfig` `paths`". Node's and TypeScript's actual algorithms have many modes (`moduleResolution` node10/node16/bundler, conditions, `typesVersions`, project references); the graph is file-level and syntactic, so it needs a fixed subset that is right for ordinary repositories and says `Unresolved` rather than guessing. ADR-029 fixed API signatures and roots in general but not what "exported" means for ECMAScript, where a module exports declarations, names declared elsewhere, other modules' bindings, and (in TypeScript) a single `export =` value.

## Options

**Resolution.** (a) Shell out to `node`/`tsc`: exact, but needs a toolchain and installed `node_modules`, and is slow. (b) A fixed, documented subset of both algorithms read from manifests: deterministic and offline.

**Bare specifiers that name no workspace package.** `External` only when `package.json` lists the dependency (like Rust), or `External` always. Hoisted and transitive dependencies make "listed" unreliable in JavaScript monorepos; a typo then reads as an external package, which `static.deps` shows by name.

**`node:` builtins.** Name the package `node`, the module (`fs`), or `node:<module>`; the last cannot collide with an npm package of the same name.

**API items.** Report only declarations exported in place, or also export clauses and re-exports, whose kind lives in another statement or file.

## Decision

- Resolution is as documented in `cupel_lang::langs::ecma::resolve` (relative paths; `#` subpath imports; builtins; nearest tsconfig/jsconfig `paths`, then `baseUrl`; workspace packages through `exports` conditions `import`, `require`, `node`, `default`, `types` in that order, then `main`, then `index`; every other bare specifier `External` by package name; URLs and absolute paths `Unresolved`). A `paths` pattern that matches but finds no file is `Unresolved`. Probing tries the name, TypeScript's `.js`→`.ts` substitution, appended extensions (TypeScript's first for TypeScript files), then a directory's `main` and `index`. Builtins are `External { package: "node:<module>" }` with or without the prefix. `relative` is true exactly for `./`, `../`, `.`, and `..` specifiers. `tsconfig` `extends`, project references, `typesVersions`, and `node_modules` are not read.
- Roots are the directories of `package.json` files: the root's, its `workspaces` (array or `{ packages }`, `*` per segment), and the nearest one above each file in scope.
- `ApiSurface` reports declarations exported in place (`export function`, `export class` and its public methods, `export const`/`let`/`var`, TypeScript `interface`, `type`, `enum`, `namespace`, and interface method signatures), `export default` functions as `default` (also a `framework-hook` root), CommonJS `exports.x = function …` as a function `x`, `export * as name from` as a `module`, and TypeScript `export = name` as a `module` named `name`. Export clauses (`export { a }`), named re-exports, and `export *` are not items; a top-level symbol named in an export clause or `module.exports` is still `public` in `Symbols` (the `@export.name` capture), so it is a `Roots` export.
- A `const` bound to a function is a `function` item; other `const`s are `constant`, `let` and `var` are `variable`. A class or interface is `type`/`interface` with its type parameters as signature; private (`#name`, `private`, `protected`) members are not reported.

## Consequences

- Hoisted or undeclared dependencies resolve as `External`; a misspelt package does too. Generated files and missing relative files are `Unresolved`, as in Rust.
- Projects that rely on `extends` to inherit `paths`, or on `moduleResolution: bundler` quirks, get `External` or `Unresolved` edges where the compiler would find a file; reading `extends` is an additive change to the resolver.
- An API check sees added, removed, and re-signed declarations, not re-export reshuffles.
