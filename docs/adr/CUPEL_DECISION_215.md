# CUPEL_DECISION_215: A Clojure project's API is what its published source paths hold

**Date:** 2026-11-14 · **Status:** accepted; its note that Leiningen is no Cupel root amended by [234](CUPEL_DECISION_234.md) (owner rulings, 2026-11-14) · **Phase:** v0.2.9, item V20 · Amends [101](CUPEL_DECISION_101.md); the Clojure analogue of [176](CUPEL_DECISION_176.md) and [192](CUPEL_DECISION_192.md)

## Context
Replaying clj-commons/clj-yaml PR #181 ("maintenance: dev, ci, test, doc", maintainer-merged), `check --changed --gate` failed `"static.api-surface" = "no-breaking"` with 10 `api.breaking`, every one in a babashka dev script: `ci-publish/clojars-deploy changed its signature`, `download-deps/-main was removed`, `lint/-main was removed`, `publish/-main was removed`, and so on. Those files are under `script/` and `build/`, on `bb.edn`'s `:paths ["script" "build"]`. clj-yaml's `deps.edn` publishes `:paths ["src/clojure" "target/classes" "resources"]`: no consumer of the library can call a var of `script/`. `static.api-surface` treated every public var of every Clojure file as API.

Open: what defines a Clojure library's API, and how that composes with ADR-176's `internal` items, V9's test-file exclusion, and V10's `[api] skip`.

## Options
(a) Every public var is API, as before: wrong for every project with dev scripts, build scripts, or dev namespaces. (b) Leave out files under known tooling directories (`script/`, `build/`, `dev/`): a guess, and wrong for a library that publishes one of them. (c) Read the project manifest, as ADR-176 reads `package.json`: a file is published when it is under a path the classpath of the library's consumers holds, which for tools.deps is the top-level `:paths` and for Leiningen the top-level `:source-paths`. (d) Also follow the namespaces the published ones `require`: a var of an unpublished file is not loadable by a consumer anyway, so it adds nothing.

Chosen: (c). It is the definition the build tools themselves use, and it needs no knowledge of what a directory is for.

## Decision
1. **The project** of a Clojure file is the nearest directory above it with a `deps.edn` or a `project.clj`. A directory with both publishes what either does.
2. **Published paths** are the strings of the vector the manifest's top-level key names, relative to the project: `deps.edn`'s `:paths`, `project.clj`'s `:source-paths` (in the `defproject` form). Absent, the key defaults to `["src"]`, as tools.deps and Leiningen default; an empty vector publishes nothing. Alias keywords inside `:paths` are not followed.
3. **Not published:** alias paths (`:extra-paths`, and any key inside `:aliases`), Leiningen profiles' `:source-paths`, `bb.edn`'s `:paths`, and anything else.
4. **Internal.** A public var whose file is under no published path of its project is `internal` on `ApiSurface`, as ADR-176's unexposed items are; `static.api-surface` already leaves internal items out on both sides, so changing, removing, or adding one is neither breaking nor widening. Each side reads its own manifests (the base side through ADR-192's base reading), so a range that drops a path from `:paths` removes that path's vars from the API.
5. **No manifest.** A file under neither manifest keeps every public var as API, as before: Cupel cannot tell what such a tree publishes, and a false break is visible where a missed one is not.

## Consequences
- clj-yaml's PR #181 passes the gate with no finding; its `src/clojure` vars are compared as before.
- **Test conventions** (V9) apply independently: a var in a test file is never API, even under a published path, and Clojure test directories are usually also outside `:paths`.
- **`[api] skip`** (V10) is unrelated: it shapes the tokens of a signature, not which items are compared.
- The rule lives in `cupel-lang` (`langs/clojure/published.rs`), the language's `mark_internal`; `static.api-surface` stays language-free.
- `deps.edn` and `project.clj` are read by a small reader of EDN-like text (strings, comments, nesting), not a full EDN parser: reader conditionals, `#_` discards, and computed `project.clj` values are not interpreted. Leiningen is not otherwise a Cupel root (its runner and roots are unchanged).
- Fixture: `crates/cupel-fixtures/repos/clojure-api-paths/clojure`.
