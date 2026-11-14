# CUPEL_DECISION_210: Cupel's Clojure runs take the `:test` alias's paths and dependencies, not its `:main-opts`

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.9, item V12 · Amends [109](CUPEL_DECISION_109.md) §12 and §13 (the Clojure runner and coverage commands)

## Context
ADR-109 runs Cupel's runner script with `clojure -M:test <script> <report>` and Cloverage with `clojure -Sdeps '{:deps {cloverage/cloverage …}}' -M:test -m cloverage.coverage …`: the `:test` alias supplies the test paths and dependencies. The tools.deps convention for that alias also gives it `:main-opts`, the project's own runner: clj-commons/clj-yaml's is `{:extra-paths ["test"] :extra-deps {io.github.cognitect-labs/test-runner …} :main-opts ["-m" "cognitect.test-runner"]}`, and Kaocha's setup is the same shape. With `-M`, the CLI puts the alias's `:main-opts` before the command line's arguments, so on clj-yaml `-M:test <script> <report>` ran `cognitect.test-runner` with the script's path as an argument: 38 tests passed, Cupel's script never ran, no report was written, and the baseline was `baseline.run-incomplete` with exit code 0. A `-m` on the command line does not replace the alias's: `-M:test -m cloverage.coverage …` ran the alias's main too (observed with Clojure CLI 1.12.5.1664 and a stand-in main), so Cloverage wrote no report.

Open: how Cupel gets the alias's classpath without its main.

## Options
(a) `-A:test -M <script>`: the CLI warns that `:main-opts` with `-A` is deprecated and still runs them (observed). (b) `clojure -Spath -M:test` for the classpath, then `java -cp <classpath> clojure.main <script>`: two processes, Cupel finds and runs `java` itself, and the JVM options the alias's `:jvm-opts` give are lost. (c) An alias on the command line, `-Sdeps '{:aliases {:cupel/no-main {:main-opts []}}}'`, named after `:test` (`-M:test:cupel/no-main`): the CLI merges aliases with the last `:main-opts` winning, so the empty one replaces the alias's while its paths, dependencies, and `:jvm-opts` stay (observed: the script runs with the alias's paths, and with `-m cloverage.coverage` Cloverage's main runs). A qualified alias name is one a project does not declare by accident.

## Decision
1. **The runner** `tool.clojure-test` runs `clojure -Sdeps '{:aliases {:cupel/no-main {:main-opts []}}}' -M:test:cupel/no-main <script> <report>` (option c). The toolchain row's `test` command, which `init` writes and a person runs, stays `clojure -M:test`, the project's own way to run its tests.
2. **Coverage** `tool.cloverage` runs `clojure -Sdeps '{:deps {cloverage/cloverage {:mvn/version "1.2.4"}} :aliases {:cupel/no-main {:main-opts []}}}' -M:test:cupel/no-main -m cloverage.coverage --src-ns-path src --test-ns-path test --lcov --no-html --no-summary`.
3. **Fixture**: `crates/cupel-fixtures/repos/tools-build/` (clj-yaml's shape: a `:test` alias with `:main-opts ["-m" "cognitect.test-runner"]`, whose main is a stand-in under `runner/` that runs nothing, so no git dependency is fetched).

## Consequences
- A project whose `:test` alias runs cognitect's test-runner, Kaocha, or any other main gets Cupel's report from the runner and an LCOV report from Cloverage.
- A project without `:main-opts` in `:test` runs as before: the empty `:main-opts` replaces none.
- A `cupel.toml` already holding the earlier Cloverage command keeps it; running `init` again writes this one.
