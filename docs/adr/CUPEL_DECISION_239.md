# CUPEL_DECISION_239: Leiningen roots' tests run through Leiningen, and Cloverage is added by coordinate where no plugin is declared

**Date:** 2026-11-17 · **Status:** accepted · **Phase:** v0.3.1, item 1 (Leiningen execution) · Amends [234](CUPEL_DECISION_234.md) §3 (coverage) and §4 (no producer); uses [109](CUPEL_DECISION_109.md)'s runner script and [210](CUPEL_DECISION_210.md)'s Cloverage by coordinate

## Context

ADR-234 made a Leiningen `project.clj` a Clojure root and wrote `test = "lein test"` for it, but registered no producer that runs Leiningen. It also wrote a coverage command only where `project.clj` declares the lein-cloverage plugin. So on ring-clojure/ring, v0.3.0's replay showed `init` writing `lein test` and `doctor` warning that `lein` was missing, and `tests.coverage`, `tests.flaky`, and `tests.mutation` skipped. ADR-234 left execution undone because Leiningen was not installed where it was built.

The owner's v0.3.1 ruling (2026-11-16): add `lein` to the pinned dev tools, and verify that ring's test, coverage, and mutation checks run under Leiningen on the real repository.

What was observed on ring, by hand, under the validation harness's no-egress sandbox with Leiningen 2.12.0 and the dependencies fetched beforehand:

- `lein test` at the top runs its `sub test` alias: 248 tests across five subprojects, all passing, in 31 s.
- In `ring-core`, `lein with-profile +test run -m clojure.main <Cupel's runner script> <report>` loads the project's test classpath. It finds no tests, because ring's test files (`test/ring/middleware/test/cookies.clj`) are not named `*_test.clj`, which is how the script finds test namespaces on the classpath. `lein test` instead runs every namespace under `:test-paths`. Given that directory, the script finds the same 192 tests `lein test` runs, all passing.
- ADR-234 §3 rejected Cloverage by coordinate because "there is no Leiningen equivalent of `-Sdeps`". That premise is wrong: Leiningen's built-in `update-in` task changes the project map for one invocation. In `ring-core`, `lein update-in :dependencies conj '[cloverage/cloverage "1.2.4"]' -- with-profile +test run -m cloverage.coverage --src-ns-path src --test-ns-path test --lcov --no-html --no-summary` writes `target/coverage/lcov.info` for 28 files.

Open: how Cupel runs a Leiningen root's tests, and what coverage a root that declares no plugin gets.

## Options

1. **The runner.**
   - (a) A `tool.lein-test` producer that runs Cupel's runner script through `lein with-profile +test run -m clojure.main`. This is the classpath `lein test` uses. It gives the per-test JUnit results, test selection, shuffling, and listing that `tool.clojure-test` already has.
   - (b) Run `lein test` and read its output. It prints only failures and totals: no per-test results for passing tests, no selection by var, and no seed.
2. **Which namespaces run.**
   - (a) The script takes `--test-dir DIR`. Given it, every namespace below `DIR` runs, as `lein test` runs `:test-paths`. The producer passes `test`, Leiningen's default.
   - (b) Every namespace in every classpath directory. This would also load `src`, `resources`, and `dev-resources`.
   - (c) Read `:test-paths` from `project.clj`. No project validated declares other paths, and the Cloverage command below already names `test`.
3. **Coverage without a declared plugin.** The owner's ruling (2026-11-17): "Cloverage by coordinate".
   - (a) Cloverage 1.2.4 by coordinate through `lein update-in`, as a tools.deps root gets it through `-Sdeps` (ADR-210).
   - (b) Keep ADR-234 §3: no coverage command.
4. **A root that declares lein-cloverage.**
   - (a) Its own `lein cloverage --lcov`, as ADR-234 writes, now run by a producer.
   - (b) Cloverage by coordinate there too, beside the project's plugin.

## Decision

1. **(a).** `tool.lein-test` runs `lein with-profile +test run -m clojure.main <script> <report> --test-dir test`, followed by the narrowing. It reads the report as `tool.clojure-test` does. It needs `lein` (`lein version`). `tool.clojure-test` still defers in a Leiningen root (ADR-234 §4). With no project, `tool.lein-test` sorts after the Clojure CLI's runner, as a runner with a detect rule does.
2. **(a).** A tools.deps root keeps the classpath scan for `*_test.clj(c)` files.
3. **(a), the owner's ruling.** `tool.cloverage-lein` runs `lein update-in :dependencies conj '[cloverage/cloverage "1.2.4"]' -- with-profile +test run -m cloverage.coverage --src-ns-path src --test-ns-path test --lcov --no-html --no-summary`. It is paired with `tool.lein-test`, and `init` writes it for a Leiningen root that declares no lein-cloverage.
4. **(a).** `tool.lein-cloverage` is now a producer that runs `lein cloverage --lcov`, detected where `project.clj` mentions lein-cloverage. Both read `target/coverage/lcov.info`.

Leiningen 2.12.0 is pinned:

- `make dev-setup` installs it from Codeberg, where Leiningen's releases are published. The standalone jar is checked against the sha256 in the launcher's `LEIN_CHECKSUM`.
- `dev-env.sh` exports `LEIN_HOME`, whose `profiles.clj` uses the tools' Maven repository.
- CI's tools action installs the same release, checked the same way.
- 2.12.0 (2025-09-12) was chosen over 2.13.1, which was six days old. 2.13.0 is not a Codeberg release.

## Consequences

- ring gets `coverage` in its `cupel.toml` from `init`, and its test, coverage, flaky, and mutation checks can run under Leiningen. The validation run on ring is recorded in the plan (batch v0.3.1, item 1).
- A Leiningen project whose tests live outside `test/`, or whose sources live outside `src/`, runs no tests and measures no coverage. Reading `:test-paths` and `:source-paths` is not done.
- Leiningen downloads Cloverage and its dependencies on first use, as the Clojure CLI does for tools.deps. Under no egress they must already be in the local repository.
- Fixtures:
  - `test-results/leiningen`, whose namespace `tally.totals` is not named `*-test`, and its red overlay.
  - `coverage/leiningen` and `coverage/lein-cloverage`, with their committed reports.
- Tests:
  - `cupel-adapt/tests/runners.rs` `lein_test_runs_the_leiningen_fixture`, plus the registration, detection, and no-project tests.
  - `coverage_producers.rs`: both producers cover their fixtures, need `lein`, and are detected by fixture.
  - `toolchain.rs`: a Leiningen project gets Cloverage by coordinate or its plugin's task.
