# CUPEL_DECISION_234: A Leiningen `project.clj` is a Clojure root

**Date:** 2026-11-14 · **Status:** accepted; §3 (Cloverage by coordinate where no plugin is declared) and §4 (Leiningen runs the tests) amended by [239](CUPEL_DECISION_239.md) · **Phase:** v0.3.0, item 12 (Leiningen roots) · Amends [109](CUPEL_DECISION_109.md) (Clojure roots) and [215](CUPEL_DECISION_215.md)'s note that Leiningen is no root; uses [162](CUPEL_DECISION_162.md)'s `defer` and [228](CUPEL_DECISION_228.md)'s `not` rule

## Context
Validating v0.2.9 on ring-clojure/ring, a Leiningen project (`project.clj` at the top and in each of `ring-core/`, `ring-servlet/`, and the rest; no `deps.edn`), `init` wrote a `cupel.toml` with no `[lang.clojure]` table and no root: only the Clojure CLI's `deps.edn` made a Clojure root, so ring had static checks only and no test, coverage, or mutation (NOTES G2). ADR-215 already reads `project.clj`'s `:source-paths` for the API and says "Leiningen is not otherwise a Cupel root".

The owner's ruling: "Leiningen projects are detected as roots."

Open: which commands `init` writes for such a root, which coverage tool it names, and what a run does with a root no Cupel runner can run.

## Options
1. **Roots.** (a) A directory with a `project.clj` is a Clojure root, as one with a `deps.edn` is; a directory below it with a manifest of its own is a member (a Leiningen subproject, as ring's are). (b) Only a `project.clj` without `deps.edn`: a directory with both is still one root, so (a) and (b) differ only in which runner it gets, decided below.
2. **The test command.** (a) `lein test`, Leiningen's own task, which a person runs and doctor probes (`lein`). (b) A command that runs Cupel's runner script through Leiningen (`lein run -m clojure.main <script>`): Cupel's runner would need a producer for it, which nothing here can run (Leiningen is not installed where this was built, and fetching it or a project's dependencies needs network).
3. **Coverage.** (a) Cloverage's Leiningen task, `lein cloverage --lcov`, only where the project declares the lein-cloverage plugin: as ADR-197 and ADR-228 write a coverage tool's command only where the project declares it when another choice exists, and Cupel otherwise writes no coverage command where no row pairs with the runner. (b) Cloverage by coordinate (`-Sdeps`) as for tools.deps: there is no Leiningen equivalent of `-Sdeps` that adds a plugin without editing `project.clj`, and the ruling sets no default.
4. **At run time.** Without a producer for `lein test`, the Clojure CLI's runner (`tool.clojure-test`, the row's fallback) would be detected in a Leiningen root and run `clojure -M:test` without the project's dependencies, failing the baseline. (a) Set it aside there with a `defer` rule, as ADR-162 sets aside a runner the root's own entry point contradicts: no runner runs, and the checks that need test results skip. (b) Let it run.

## Decision
1. **(a).** Clojure's manifests are `deps.edn` and `project.clj`.
2. **(a).** The Clojure row's first runner is `tool.lein-test`, `lein test`, detected where the root has a `project.clj` and no `deps.edn`. A directory with both keeps the Clojure CLI's runner and commands.
3. **(a).** `tool.lein-cloverage`, `lein cloverage --lcov` with `tool.lein-test`, is detected where `project.clj` mentions `lein-cloverage`. Otherwise `init` writes no coverage command for the root.
4. **(a).** `tool.clojure-test` defers where the root has a `project.clj` and no `deps.edn`. No producer is registered for `tool.lein-test` or `tool.lein-cloverage`: they are the commands `init` writes and a person runs, and `describe language clojure` lists them with their rules.

## Consequences
- ring gets a Clojure root (its subprojects as members), `test = "lein test"`, and the starter gate (ADR-233); `doctor` probes `lein`. Its static checks run as before; `tests.coverage`, `tests.flaky`, and `tests.mutation` skip, since no Cupel runner reads `lein test` (observed on the fixture: the checks needing test results skip, the static ones pass).
- Running Leiningen projects' tests (a runner producer for `lein`, Cloverage's report through it, mutation) is not done: it needs a producer verified against a real Leiningen, which this change could not run.
- A `deps.edn` project is unchanged.
- Fixture: `crates/cupel-fixtures/repos/leiningen/clojure` (a `project.clj` with a `:dev` profile, `src/` and `test/`), with its `init --dry-run` golden `golden/init/leiningen.json`; `cupel-adapt/tests/toolchain.rs` covers the plugin, both manifests, and the deferred CLI runner.
