# External dogfood: cljfx/cljfx

Group 3 of P9.9 ([ADR-138](../adr/CUPEL_DECISION_138.md)): the Clojure repository. Cupel's release build (`target/release/cupel`, ADR-138 §2) of branch `p9-dogfood-ts-clojure`, rebuilt after each fix: the surveys at `6a582a5` (the first two), `3003be2`, `f6b9d3c`, and `32c7687` (the final one, and the deep check); every run went through `scripts/heavy.sh`.

## Repository

- **URL:** https://github.com/cljfx/cljfx, commit `6bee510` (2026-10-08), default branch `master`, version 1.10.10.
- **Lines:** 17,214 lines of Clojure in 311 files: 9,863 in `src/` (220 files, most of them `cljfx.fx.*`, one namespace per JavaFX class), 2,247 in `test/` (13), 4,698 in `examples/` (50), 207 in `jdk11/` and `jdk8/` (the JDK-specific namespaces; `jdk11` is on `deps.edn`'s `:paths`), 90 in `dev/`, and `build.clj` and `example-projects/`; plus three Java files under `example-projects/`.
- **License:** MIT.
- **Suite:** `clojure -M:test:runner` (the documented command; cognitect's test-runner) passes in 16.5 s with dependency downloads: 54 tests, 267 assertions (Temurin-compatible OpenJDK 21 and the Clojure CLI 1.12.5 from `make dev-setup`, dependencies from Maven Central and Clojars into the dev environment's repository; the tests start the JavaFX toolkit, which on macOS shows a Dock icon and no window). Selection: the most depended-on permissively licensed Clojure library with a `deps.edn` whose `:test` alias only adds `test` to the classpath, which is what Cupel's runner runs (`clojure -M:test <runner script>`). Ruled out: EPL-licensed libraries (most of the ecosystem: malli, honeysql, next.jdbc, sci, core.async); `clj-commons/rewrite-clj` (MIT) and `juxt/tick` (MIT), whose `deps.edn` has no `:test` alias; `cognitect-labs/aws-api`, `nextjournal/clerk`, `babashka/cli`, and `yonureker/stripe-clojure`, whose `:test` alias runs cognitect's test-runner through `:main-opts`, which would take Cupel's arguments as its own (an open item below); `clj-commons/pretty` (5.5k lines) and `nextjournal/markdown` (2.6k) were under the size floor; `djblue/portal` needs a ClojureScript build before its JVM tests; `scicloj/tablecloth` tests with midje.

## Commands

From Cupel's worktree, with `C=target/release/cupel` and `R=<scratchpad>/p9-group3/repos/cljfx`, each line one `scripts/heavy.sh` invocation:

```
$C init --write --cwd $R                                                        # kept, two keys changed (below)
$C survey --depth standard --budget 30m --no-install --format json --cwd $R    # 132 s, exit 0, at 6a582a5: tests.coverage and tests.mutation error
$C survey --depth standard --budget 30m --no-install --format json --cwd $R    # 111 s, exit 0, at 6a582a5: coverage key 1, still error
$C survey --depth standard --budget 30m --no-install --format json --cwd $R    # 330 s, exit 0, at 3003be2: coverage key 2, every check ran
$C survey --depth standard --budget 30m --no-install --format json --cwd $R    # 37 s, exit 0, at f6b9d3c (mutant outcomes reused)
$C survey --depth standard --budget 30m --no-install --format json --cwd $R    # 37 s, exit 0, at 32c7687
$C check --depth deep --no-install --format json --cwd $R \
   --paths test/cljfx/event_handlers_test.clj test/cljfx/local_state_test.clj src/cljfx/coerce.clj \
           src/cljfx/composite.clj src/cljfx/lifecycle.clj \
           examples/e30_devtools_via_event_filters.clj src/cljfx/fx/transition.clj   # 36 s, exit 0, at 32c7687
```

`cupel init` wrote two roots: `clojure` (`[lang.clojure] test = "clojure -M:test"`, `coverage` the Cloverage command) and `java`, from the `pom.xml` cljfx keeps for publishing (`mvn test` and the JaCoCo command); kept as written, so the Java root's coverage reports `coverage.run-failed` (there is no Java test). Set by hand: `mutation = { jobs = 2 }` in both tables (the brief's two slots), and the Cloverage command, twice. The command `cupel init` wrote failed in the first survey: Cloverage loads every source namespace before any test runs, and a `javafx.scene.control` class cannot initialize before the JavaFX toolkit starts (`Could not initialize class javafx.scene.control.IndexedCell`), so `tests.coverage` was `error` and `tests.mutation` with it (`fact mutation-outcomes is declared but was not produced`). Starting the toolkit first (`-e '(javafx.application.Platform/startup (fn []))'` before `-m cloverage.coverage`) got past it and then crashed on macOS's AppKit (`uncaught exception of type NSException`); the repository's own `:headless` alias (Monocle, which its CI uses) avoids AppKit, and the final command is `clojure -Sdeps '{:deps {cloverage/cloverage {:mvn/version "1.2.4"}}}' -M:test:headless -e '(javafx.application.Platform/startup (fn []))' -m cloverage.coverage --src-ns-path src --test-ns-path test --lcov --no-html --no-summary`. The ten targets of the final survey are in seven files (two test files, one example, four sources); the deep check ran on them. It mutates what the survey did and reuses every stored outcome (`reused 27`, `executed 0`): Cupel's `clojure.test` runner has no per-test map (ADR-147 §5), so `deep` adds no mutant there. Raw envelopes (0.3–1.4 MB) stay in the agent's scratchpad.

## Survey

Whole tree at `32c7687`, status `pass`, exit 0, 654 findings (11 fail, 177 warn, 466 info), scope 314 files and 624 symbols, 37 s. The rankings, verbatim (`rank. target value (location)`):

- `static.clones.cluster_size` per group, highest worst, 12 measured: 1. `cljfx.fx.table-cell/create and 2 others` 3 (src/cljfx/fx/table_cell.clj:33); 2. `cljfx.composite/observable-list and 1 other` 2 (src/cljfx/composite.clj:143); 3. `cljfx.ext.web-view/coerce-callback and 1 other` 2 (src/cljfx/ext/web_view.clj:15); 4. `cljfx.fx.combo-box/cell-factory and 1 other` 2 (src/cljfx/fx/combo_box.clj:17); 5. `cljfx.fx.combo-box/observable-list and 1 other` 2 (src/cljfx/fx/combo_box.clj:31); 6. `cljfx.fx.table-view/table-sort-policy and 1 other` 2 (src/cljfx/fx/table_view.clj:26); 7. `e12-interactive-development/root-view and 1 other` 2 (examples/e12_interactive_development.clj:202); 8. `e13-re-frame-like-state/ingredient-badge and 1 other` 2 (examples/e13_re_frame_like_state.clj:111); 9. `e13-re-frame-like-state/potion-list and 1 other` 2 (examples/e13_re_frame_like_state.clj:133); 10. `e20-markdown-editor/md->fx and 1 other` 2 (examples/e20_markdown_editor.clj:94)
- `static.complexity.cyclomatic` per symbol, highest worst, 624 measured: 1. `cljfx.lifecycle/wrap-many` 27 (src/cljfx/lifecycle.clj:324); 2. `cljfx.coerce/font` 15 (src/cljfx/coerce.clj:211); 3. `cljfx.coerce/border-widths` 14 (src/cljfx/coerce.clj:346); 4. `user/infer-props` 13 (dev/user.clj:8); 5. `cljfx.coerce/duration` 10 (src/cljfx/coerce.clj:195); 6. `cljfx.coerce/paint` 10 (src/cljfx/coerce.clj:110); 7. `cljfx.coerce/corner-radii` 9 (src/cljfx/coerce.clj:235); 8. `cljfx.watcher-test/updates-during-child-initialization-test` 9 (test/cljfx/watcher_test.clj:219); 9. `e30-devtools-via-event-filters/devtools-highlight-filter` 9 (examples/e30_devtools_via_event_filters.clj:27); 10. `cljfx.coerce/border-stroke-style` 7 (src/cljfx/coerce.clj:331)
- `static.complexity.cognitive` per symbol, highest worst, 624 measured: 1. `cljfx.lifecycle/wrap-many` 58 (src/cljfx/lifecycle.clj:324); 2. `e30-devtools-via-event-filters/devtools-highlight-filter` 24 (examples/e30_devtools_via_event_filters.clj:27); 3. `cljfx.coerce/border-widths` 17 (src/cljfx/coerce.clj:346); 4. `cljfx.watcher-test/updates-during-child-initialization-test` 17 (test/cljfx/watcher_test.clj:219); 5. `cljfx.coerce/font` 14 (src/cljfx/coerce.clj:211); 6. `user/infer-props` 14 (dev/user.clj:8); 7. `cljfx.composite/advance-composite-component` 11 (src/cljfx/composite.clj:52); 8. `cljfx.lifecycle/map-of` 11 (src/cljfx/lifecycle.clj:735); 9. `cljfx.coerce/duration` 9 (src/cljfx/coerce.clj:195); 10. `cljfx.coerce/paint` 9 (src/cljfx/coerce.clj:110)
- `static.crap.crap` per symbol, highest worst, 263 measured: 1. `cljfx.coerce/font` 240 (src/cljfx/coerce.clj:211); 2. `cljfx.coerce/border-widths` 210 (src/cljfx/coerce.clj:346); 3. `cljfx.coerce/duration` 182 (src/cljfx/coerce.clj:195); 4. `cljfx.fx.dialog-pane/button-type` 182 (src/cljfx/fx/dialog_pane.clj:11); 5. `cljfx.fx.transition/coerce-interpolator` 182 (src/cljfx/fx/transition.clj:11); 6. `cljfx.coerce/string-converter` 132 (src/cljfx/coerce.clj:420); 7. `cljfx.coerce/paint` 110 (src/cljfx/coerce.clj:110); 8. `cljfx.coerce/corner-radii` 90 (src/cljfx/coerce.clj:235); 9. `cljfx.coerce/border-stroke-style` 56 (src/cljfx/coerce.clj:331); 10. `cljfx.coerce/key-combination` 56 (src/cljfx/coerce.clj:446)
- `static.deps.distance` per module, highest worst, 5 measured: 1. `src/cljfx/prop.clj` 0.92 (src/cljfx/prop.clj:1); 2. `src/cljfx/lifecycle.clj` 0.85 (src/cljfx/lifecycle.clj:1); 3. `src/cljfx/component.clj` 0 (src/cljfx/component.clj:1); 4. `src/cljfx/context.clj` 0 (src/cljfx/context.clj:1); 5. `src/cljfx/mutator.clj` 0 (src/cljfx/mutator.clj:1)
- `static.deps.coupling` per module, highest worst, 302 measured: 1. `src/cljfx/lifecycle.clj` 238 (src/cljfx/lifecycle.clj:1); 2. `src/cljfx/composite.clj` 213 (src/cljfx/composite.clj:1); 3. `src/cljfx/fx.clj` 177 (src/cljfx/fx.clj:1); 4. `src/cljfx/coerce.clj` 140 (src/cljfx/coerce.clj:1); 5. `src/cljfx/api.clj` 71 (src/cljfx/api.clj:1); 6. `src/cljfx/mutator.clj` 43 (src/cljfx/mutator.clj:1); 7. `src/cljfx/fx/control.clj` 25 (src/cljfx/fx/control.clj:1); 8. `src/cljfx/prop.clj` 25 (src/cljfx/prop.clj:1); 9. `src/cljfx/fx/shape.clj` 16 (src/cljfx/fx/shape.clj:1); 10. `src/cljfx/fx/node.clj` 14 (src/cljfx/fx/node.clj:1)
- `static.deps.cycle_size` per group, highest worst, 1 measured: 1. `src/cljfx/ext/cell_factory.clj → src/cljfx/fx.clj → src/cljfx/fx/combo_box.clj → src/cljfx/ext/cell_factory.clj` 10 (src/cljfx/ext/cell_factory.clj:6)
- `tests.coverage.uncovered_lines` per symbol, highest worst, 263 measured: 1. `cljfx.coerce/border-widths` 32 (src/cljfx/coerce.clj:346); 2. `cljfx.lifecycle/wrap-let-refs` 31 (src/cljfx/lifecycle.clj:661); 3. `cljfx.fx.transition/coerce-interpolator` 28 (src/cljfx/fx/transition.clj:11); 4. `cljfx.lifecycle/map-of` 25 (src/cljfx/lifecycle.clj:735); 5. `cljfx.coerce/string-converter` 21 (src/cljfx/coerce.clj:420); 6. `cljfx.coerce/font` 18 (src/cljfx/coerce.clj:211); 7. `cljfx.coerce/image` 16 (src/cljfx/coerce.clj:84); 8. `cljfx.coerce/key-combination` 16 (src/cljfx/coerce.clj:446); 9. `cljfx.fx.dialog-pane/button-type` 15 (src/cljfx/fx/dialog_pane.clj:11); 10. `cljfx.coerce/duration` 14 (src/cljfx/coerce.clj:195)
- `tests.coverage.line_coverage` per symbol, lowest worst, 263 measured: 1. `cljfx.api/fn->lifecycle` 0 (src/cljfx/api.clj:366); 2. `cljfx.api/make-deref-co-effect` 0 (src/cljfx/api.clj:528); 3. `cljfx.api/make-ext-with-props` 0 (src/cljfx/api.clj:247); 4. `cljfx.api/make-reset-effect` 0 (src/cljfx/api.clj:550); 5. `cljfx.api/reset-context` 0 (src/cljfx/api.clj:504); 6. `cljfx.api/sub` 0 (src/cljfx/api.clj:620); 7. `cljfx.api/swap-context` 0 (src/cljfx/api.clj:498); 8. `cljfx.api/unbind-context` 0 (src/cljfx/api.clj:509); 9. `cljfx.api/unmount-renderer` 0 (src/cljfx/api.clj:446); 10. `cljfx.api/wrap-async` 0 (src/cljfx/api.clj:636)
- `tests.mutation.survivors` per symbol, highest worst, 163 measured: 1. `cljfx.lifecycle/binding-prop` 2 (src/cljfx/lifecycle.clj:135); 2. `cljfx.composite/advance-composite-component` 1 (src/cljfx/composite.clj:52); 3. `cljfx.composite/capitalize` 1 (src/cljfx/composite.clj:127); 4. `cljfx.composite/create-composite-component` 1 (src/cljfx/composite.clj:35); 5. `cljfx.context/reset` 1 (src/cljfx/context.clj:140); 6. `cljfx.lifecycle/perform-render` 1 (src/cljfx/lifecycle.clj:809); 7. `cljfx.lifecycle/request-render` 1 (src/cljfx/lifecycle.clj:833); 8. `cljfx.prop/Prop/equals` 1 (src/cljfx/prop.clj:27); 9. `cljfx.renderer/or-new-promise` 1 (src/cljfx/renderer.clj:33); 10. `cljfx.renderer/render-component` 1 (src/cljfx/renderer.clj:49)
- `tests.mutation.mutation_score` per symbol, lowest worst, 162 measured: 1. `cljfx.coerce/background-position` 0 (src/cljfx/coerce.clj:278); 2. `cljfx.coerce/background-size` 0 (src/cljfx/coerce.clj:291); 3. `cljfx.coerce/border` 0 (src/cljfx/coerce.clj:412); 4. `cljfx.coerce/border-stroke-style` 0 (src/cljfx/coerce.clj:331); 5. `cljfx.coerce/border-widths` 0 (src/cljfx/coerce.clj:346); 6. `cljfx.coerce/corner-radii` 0 (src/cljfx/coerce.clj:235); 7. `cljfx.coerce/duration` 0 (src/cljfx/coerce.clj:195); 8. `cljfx.coerce/font` 0 (src/cljfx/coerce.clj:211); 9. `cljfx.coerce/image` 0 (src/cljfx/coerce.clj:84); 10. `cljfx.coerce/insets` 0 (src/cljfx/coerce.clj:253)
- `tests.mutation.tests` per symbol, lowest worst, 163 measured: 1. `cljfx.coerce/background-position` 0 (src/cljfx/coerce.clj:278); 2. `cljfx.coerce/background-size` 0 (src/cljfx/coerce.clj:291); 3. `cljfx.coerce/border` 0 (src/cljfx/coerce.clj:412); 4. `cljfx.coerce/border-stroke-style` 0 (src/cljfx/coerce.clj:331); 5. `cljfx.coerce/border-widths` 0 (src/cljfx/coerce.clj:346); 6. `cljfx.coerce/corner-radii` 0 (src/cljfx/coerce.clj:235); 7. `cljfx.coerce/duration` 0 (src/cljfx/coerce.clj:195); 8. `cljfx.coerce/font` 0 (src/cljfx/coerce.clj:211); 9. `cljfx.coerce/image` 0 (src/cljfx/coerce.clj:84); 10. `cljfx.coerce/insets` 0 (src/cljfx/coerce.clj:253)
- `tests.quality.mock_density` per symbol, highest worst, 54 measured: 1. `cljfx.composite-test/composite-lifecycle-test` 0 (test/cljfx/composite_test.clj:11); 2. `cljfx.composite-test/unordered-composite-lifecycle-preserves-nil-valued-props-test` 0 (test/cljfx/composite_test.clj:112); 3. `cljfx.context-test/after-changing-context-only-affected-subscriptions-are-recalculated` 0 (test/cljfx/context_test.clj:31); 4. `cljfx.context-test/context-allows-subscriptions-to-call-each-other` 0 (test/cljfx/context_test.clj:6); 5. `cljfx.context-test/contexts-are-repl-friendly` 0 (test/cljfx/context_test.clj:170); 6. `cljfx.context-test/creating-derived-contexts-inside-subs-add-dependency-on-context-itself` 0 (test/cljfx/context_test.clj:124); 7. `cljfx.context-test/cross-context-dependencies-are-not-tracked` 0 (test/cljfx/context_test.clj:156); 8. `cljfx.context-test/ctx-asserts-on-extra-subs-after-return` 0 (test/cljfx/context_test.clj:210); 9. `cljfx.context-test/dirty-cache-entry-always-updates-its-key-deps` 0 (test/cljfx/context_test.clj:89); 10. `cljfx.context-test/double-invalidations-don't-trigger-recalculations` 0 (test/cljfx/context_test.clj:138)
- Targets, protect first (10 recommended, top 10): 1. `cljfx.event-handlers-test/test-swapping-else-handlers` both (mutation score None, coverage None; test/cljfx/event_handlers_test.clj); 2. `cljfx.local-state-test/test-lifecycle-semantics` protect (mutation score None, coverage None; test/cljfx/local_state_test.clj); 3. `cljfx.coerce/border-widths` both (mutation score 0.0, coverage 0.0; src/cljfx/coerce.clj); 4. `cljfx.coerce/font` both (mutation score 0.0, coverage 0.0; src/cljfx/coerce.clj); 5. `cljfx.composite/advance-composite-component` both (mutation score 0.0, coverage 1.0; src/cljfx/composite.clj); 6. `cljfx.lifecycle/wrap-let-refs` protect (mutation score 0.0, coverage 0.03125; src/cljfx/lifecycle.clj); 7. `e30-devtools-via-event-filters/devtools-highlight-filter` both (mutation score 0.0, coverage None; examples/e30_devtools_via_event_filters.clj); 8. `cljfx.coerce/duration` protect (mutation score 0.0, coverage 0.0; src/cljfx/coerce.clj); 9. `cljfx.fx.transition/coerce-interpolator` protect (mutation score 0.0, coverage 0.0; src/cljfx/fx/transition.clj); 10. `cljfx.composite/create-composite-component` protect (mutation score 0.0, coverage 1.0; src/cljfx/composite.clj)

## Check

Final survey (status `pass`, exit 0):

| Check | Status | Findings | Score | Time |
|---|---|---|---|---|
| `conc.race` | skipped (unsupported) | 0 | — | 0.0 s |
| `gen.property` | skipped (unsupported) | 0 | — | 0.0 s |
| `meta.oracle-independence` | skipped (needs-range) | 0 | — | 0.0 s |
| `meta.traceability` | skipped (needs-range) | 0 | — | 0.0 s |
| `perf.footprint` | skipped (unsupported) | 0 | — | 0.0 s |
| `spec.contracts` | skipped (unsupported) | 0 | — | 0.0 s |
| `static.analyze` | skipped (unsupported) | 0 | — | 0.0 s |
| `static.api-surface` | skipped (needs-range) | 0 | — | 0.0 s |
| `static.clones` | fail | clone.cluster 1, clone.pair 11 | 14 pairs (n 421) | 0.0 s |
| `static.complexity` | fail | complexity.high 6, function.long 10, nesting.deep 1, params.many 1 | 27 cyclomatic_max (n 624) | 0.0 s |
| `static.crap` | fail | crap.high 19, crap.watch 65 | 240 max (n 263) | 0.0 s |
| `static.dead-code` | fail | dead.export 78, dead.symbol 8 | 8 dead_symbols (n 624) | 0.2 s |
| `static.deps` | fail | dep.cycle 1 | 1 cycles (n 302) | 0.8 s |
| `tests.assertion-density` | fail | density.sparse-test 6 | 4.44444 assertions_per_test (n 54) | 0.0 s |
| `tests.coverage` | error | coverage.uncovered-symbol 107 | 0.496355 line_ratio (n 1646) | 2.1 s |
| `tests.flaky` | pass | 0 | 0 unstable (n 54) | 23.2 s |
| `tests.mutation` | fail | mutant.survived 11, mutant.uncovered 323 | 0.0923913 killed_ratio (n 368) | 0.1 s |
| `tests.quality` | fail | test.logic 3, test.weak-assertion 2 | 4.44444 assertion_density (n 54) | 0.6 s |
| `tests.weakening` | skipped (needs-range) | 0 | — | 0.0 s |

Deep check on the targets' seven files (status `pass`, exit 0, 172 findings (7 fail, 71 warn, 94 info), scope 7 files and 121 symbols, 36 s); its findings are all survey findings (same IDs) and are triaged once below:

| Check | Status | Findings | Score | Time |
|---|---|---|---|---|
| `conc.linearizability` | skipped (unconfigured) | 0 | — | 0.0 s |
| `conc.race` | skipped (unsupported) | 0 | — | 0.0 s |
| `gen.fuzz` | skipped (unsupported) | 0 | — | 0.0 s |
| `gen.metamorphic` | skipped (unsupported) | 0 | — | 0.0 s |
| `gen.property` | skipped (unsupported) | 0 | — | 0.0 s |
| `meta.oracle-independence` | skipped (needs-range) | 0 | — | 0.0 s |
| `meta.traceability` | skipped (needs-range) | 0 | — | 0.0 s |
| `perf.bench` | skipped (unsupported) | 0 | — | 0.0 s |
| `perf.footprint` | skipped (unsupported) | 0 | — | 0.0 s |
| `spec.contracts` | skipped (unsupported) | 0 | — | 0.0 s |
| `static.analyze` | skipped (unsupported) | 0 | — | 0.0 s |
| `static.api-surface` | skipped (needs-range) | 0 | — | 0.0 s |
| `static.clones` | fail | clone.pair 1 | 1 pairs (n 90) | 0.0 s |
| `static.complexity` | fail | complexity.high 4, function.long 2, nesting.deep 1, params.many 1 | 27 cyclomatic_max (n 121) | 0.1 s |
| `static.crap` | fail | crap.high 15, crap.watch 30 | 240 max (n 108) | 0.0 s |
| `static.dead-code` | pass | 0 | 0 dead_symbols (n 121) | 0.3 s |
| `static.deps` | pass | 0 | 0 cycles (n 14) | 0.0 s |
| `tests.assertion-density` | fail | density.sparse-test 2 | 3.2 assertions_per_test (n 5) | 0.0 s |
| `tests.coverage` | fail | coverage.uncovered-symbol 42 | 0.531915 line_ratio (n 1034) | 0.0 s |
| `tests.flaky` | pass | 0 | 0 unstable (n 5) | 27.5 s |
| `tests.mutation` | fail | mutant.survived 7, mutant.uncovered 64 | 0.21978 killed_ratio (n 91) | 0.2 s |
| `tests.quality` | fail | test.logic 1, test.weak-assertion 2 | 3.2 assertion_density (n 5) | 0.0 s |
| `tests.weakening` | skipped (needs-range) | 0 | — | 0.0 s |
| `gen.differential` | skipped (unconfigured) | 0 | — | 0.0 s |

`tests.coverage` is `error` only for the Java root (no report); the Clojure line ratio is 0.496 over 1,646 instrumented lines (`branches: unavailable`: Cloverage's LCOV has no branch records). `tests.mutation` (survey 3, 284.1 s, two slots) scored 0.092: 368 mutants, 34 killed, 11 survived, 1 unviable, and 323 uncovered, 241 of them in files no test loads (`examples/` 232, `dev/` 6, `example-projects/`, `jdk8/`, and `jdk11/` one each) and 82 on `src/` lines no test ran (the `cljfx.coerce` coercions, most of all).

## Triage

Every finding of the final survey (the deep check's are among them), plus every finding of the first survey that a fix removed (verdict `false`, its reason naming the fix) and its `coverage.run-failed`. Verification per ADR-138 §4:

- every `mutant.survived` was replayed: the mutation applied to the clone, the documented `clojure -M:test:runner` run through `scripts/heavy.sh`, the file restored with `git checkout`; 10 left the suite green; the eleventh (`composite.clj:86`) failed it once (`reset-test`, `(= ":a,:b" (.getText instance))`) and stayed green in two more replays;
- every `coverage.uncovered-symbol`, `crap.*`, and `mutant.uncovered` row was recomputed from the run's `target/coverage/lcov.info` (lines after the first, ADR-079): all match; the files the map does not hold were read to see whether a test loads them;
- every `static.complexity` row was recomputed from the source by a script that applies `decisions.scm`'s rules (one `case` counting once in cyclomatic, ADR-072, where the script first counted each constant), and every span and parameter count;
- every `static.dead-code` row was checked for a use of its var in its own file's other forms (all 81 example rows have one), and the build and REPL files by hand;
- by hand: every `static.clones`, `static.deps`, `tests.quality`, and `tests.assertion-density` row (assertion counts recounted; `=>` in comments excluded).

**`coverage`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_ba43f3` | `coverage.run-failed` | `—` | true | Maven ran and left no report: the root `cupel init` found by `pom.xml` (release metadata) has no Java test. | drop the `[lang.java]` root |
| `fnd_ba43f3` | `coverage.run-failed` | `—` | true | Survey 1 only: Cloverage really failed (`Could not initialize class javafx.scene.control.IndexedCell`): it loads every namespace before a test starts the JavaFX toolkit; the coverage key set by hand fixes the run. | set the coverage key |

**`static.clones`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_059748` | `clone.cluster` | `src/cljfx/fx/table_cell.clj:33` | true | Read by hand: the members share one shape, differing in names and literals (two `cell-factory`s are verbatim copies). | extract a helper |
| `fnd_90837f` | `clone.pair` | `examples/e12_interactive_development.clj:202` | true | Read by hand: the members share one shape, differing in names and literals (two `cell-factory`s are verbatim copies). | none (example code) |
| `fnd_9aa470` | `clone.pair` | `examples/e13_re_frame_like_state.clj:111` | true | Read by hand: the members share one shape, differing in names and literals (two `cell-factory`s are verbatim copies). | none (example code) |
| `fnd_082d66` | `clone.pair` | `examples/e13_re_frame_like_state.clj:133` | true | Read by hand: the members share one shape, differing in names and literals (two `cell-factory`s are verbatim copies). | none (example code) |
| `fnd_1cdcae` | `clone.pair` | `examples/e20_markdown_editor.clj:94` | true | Read by hand: the members share one shape, differing in names and literals (two `cell-factory`s are verbatim copies). | none (example code) |
| `fnd_bb3634` | `clone.pair` | `examples/e29_text_formatter.clj:15` | true | Read by hand: the members share one shape, differing in names and literals (two `cell-factory`s are verbatim copies). | none (example code) |
| `fnd_b0b4b3` | `clone.pair` | `examples/e45_git_ui.clj:138` | true | Read by hand: the members share one shape, differing in names and literals (two `cell-factory`s are verbatim copies). | none (example code) |
| `fnd_c3d164` | `clone.pair` | `src/cljfx/composite.clj:143` | true | Read by hand: the members share one shape, differing in names and literals (two `cell-factory`s are verbatim copies). | extract a helper |
| `fnd_a1bc56` | `clone.pair` | `src/cljfx/ext/web_view.clj:15` | true | Read by hand: the members share one shape, differing in names and literals (two `cell-factory`s are verbatim copies). | extract a helper |
| `fnd_847a8c` | `clone.pair` | `src/cljfx/fx/combo_box.clj:17` | true | Read by hand: the members share one shape, differing in names and literals (two `cell-factory`s are verbatim copies). | extract a helper |
| `fnd_6e376d` | `clone.pair` | `src/cljfx/fx/combo_box.clj:31` | true | Read by hand: the members share one shape, differing in names and literals (two `cell-factory`s are verbatim copies). | extract a helper |
| `fnd_9e2894` | `clone.pair` | `src/cljfx/fx/table_view.clj:26` | true | Read by hand: the members share one shape, differing in names and literals (two `cell-factory`s are verbatim copies). | extract a helper |

**`static.complexity`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_db1e20` | `complexity.high` | `dev/user.clj:8` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | none |
| `fnd_fa005d` | `complexity.high` | `examples/e30_devtools_via_event_filters.clj:27` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | none |
| `fnd_932193` | `complexity.high` | `src/cljfx/coerce.clj:211` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | simplify |
| `fnd_5f0124` | `complexity.high` | `src/cljfx/coerce.clj:346` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | simplify |
| `fnd_970150` | `complexity.high` | `src/cljfx/lifecycle.clj:324` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | simplify |
| `fnd_70b473` | `complexity.high` | `test/cljfx/watcher_test.clj:219` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | none |
| `fnd_c559b8` | `function.long` | `dev/user.clj:8` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | none |
| `fnd_d5bc27` | `function.long` | `examples/e27_selection_models.clj:219` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | none |
| `fnd_a930c9` | `function.long` | `examples/e45_git_ui.clj:202` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | none |
| `fnd_8df614` | `function.long` | `src/cljfx/composite.clj:52` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | simplify |
| `fnd_45d5d8` | `function.long` | `src/cljfx/lifecycle.clj:324` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | simplify |
| `fnd_ee6487` | `function.long` | `test/cljfx/composite_test.clj:11` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | none |
| `fnd_343efe` | `function.long` | `test/cljfx/lifecycle_test.clj:215` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | none |
| `fnd_c9e49e` | `function.long` | `test/cljfx/lifecycle_test.clj:311` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | none |
| `fnd_ea2cfd` | `function.long` | `test/cljfx/lifecycle_test.clj:429` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | none |
| `fnd_981f1d` | `function.long` | `test/cljfx/lifecycle_test.clj:497` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | none |
| `fnd_5cde01` | `nesting.deep` | `examples/e30_devtools_via_event_filters.clj:27` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | none |
| `fnd_e5e435` | `params.many` | `src/cljfx/lifecycle.clj:804` | true | Recomputed from the source (cyclomatic per decisions.scm, a `case` once, ADR-072; spans and parameters counted). | simplify |

**`static.crap`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_ec93a2` | `crap.high` | `src/cljfx/coerce.clj:110` | true | CRAP 110.0 recomputed from cyclomatic 10 and the run's lcov coverage 0.000. | test it |
| `fnd_47372f` | `crap.high` | `src/cljfx/coerce.clj:152` | true | CRAP 56.0 recomputed from cyclomatic 7 and the run's lcov coverage 0.000. | test it |
| `fnd_6a5904` | `crap.high` | `src/cljfx/coerce.clj:195` | true | CRAP 182.0 recomputed from cyclomatic 13 and the run's lcov coverage 0.000. | test it |
| `fnd_f1a448` | `crap.high` | `src/cljfx/coerce.clj:211` | true | CRAP 240.0 recomputed from cyclomatic 15 and the run's lcov coverage 0.000. | test it |
| `fnd_a77fe7` | `crap.high` | `src/cljfx/coerce.clj:235` | true | CRAP 90.0 recomputed from cyclomatic 9 and the run's lcov coverage 0.000. | test it |
| `fnd_b7dd14` | `crap.high` | `src/cljfx/coerce.clj:253` | true | CRAP 30.0 recomputed from cyclomatic 5 and the run's lcov coverage 0.000. | test it |
| `fnd_7d4a50` | `crap.high` | `src/cljfx/coerce.clj:278` | true | CRAP 30.0 recomputed from cyclomatic 5 and the run's lcov coverage 0.000. | test it |
| `fnd_1c49fd` | `crap.high` | `src/cljfx/coerce.clj:291` | true | CRAP 30.0 recomputed from cyclomatic 5 and the run's lcov coverage 0.000. | test it |
| `fnd_87f73c` | `crap.high` | `src/cljfx/coerce.clj:331` | true | CRAP 56.0 recomputed from cyclomatic 7 and the run's lcov coverage 0.000. | test it |
| `fnd_fdf23d` | `crap.high` | `src/cljfx/coerce.clj:346` | true | CRAP 210.0 recomputed from cyclomatic 14 and the run's lcov coverage 0.000. | test it |
| `fnd_b3f4e7` | `crap.high` | `src/cljfx/coerce.clj:420` | true | CRAP 132.0 recomputed from cyclomatic 11 and the run's lcov coverage 0.000. | test it |
| `fnd_496ca3` | `crap.high` | `src/cljfx/coerce.clj:446` | true | CRAP 56.0 recomputed from cyclomatic 7 and the run's lcov coverage 0.000. | test it |
| `fnd_12d9dd` | `crap.high` | `src/cljfx/coerce.clj:84` | true | CRAP 30.0 recomputed from cyclomatic 5 and the run's lcov coverage 0.000. | test it |
| `fnd_0f4a41` | `crap.high` | `src/cljfx/fx/date_picker.clj:15` | true | CRAP 56.0 recomputed from cyclomatic 7 and the run's lcov coverage 0.000. | test it |
| `fnd_81bb57` | `crap.high` | `src/cljfx/fx/dialog_pane.clj:11` | true | CRAP 182.0 recomputed from cyclomatic 13 and the run's lcov coverage 0.000. | test it |
| `fnd_d9c90e` | `crap.high` | `src/cljfx/fx/scroll_pane.clj:13` | true | CRAP 56.0 recomputed from cyclomatic 7 and the run's lcov coverage 0.000. | test it |
| `fnd_a5e02f` | `crap.high` | `src/cljfx/fx/table_view.clj:16` | true | CRAP 30.0 recomputed from cyclomatic 5 and the run's lcov coverage 0.000. | test it |
| `fnd_cf2096` | `crap.high` | `src/cljfx/fx/transition.clj:11` | true | CRAP 182.0 recomputed from cyclomatic 13 and the run's lcov coverage 0.000. | test it |
| `fnd_2e137b` | `crap.high` | `src/cljfx/lifecycle.clj:735` | true | CRAP 50.6 recomputed from cyclomatic 7 and the run's lcov coverage 0.038. | test it |
| `fnd_7a3068` | `crap.watch` | `src/cljfx/coerce.clj:120` | true | CRAP 20.0 recomputed from cyclomatic 4 and the run's lcov coverage 0.000. | test it |
| `fnd_1becee` | `crap.watch` | `src/cljfx/coerce.clj:140` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_40206c` | `crap.watch` | `src/cljfx/coerce.clj:162` | true | CRAP 6.0 recomputed from cyclomatic 2 and the run's lcov coverage 0.000. | test it |
| `fnd_c9953c` | `crap.watch` | `src/cljfx/coerce.clj:167` | true | CRAP 20.0 recomputed from cyclomatic 4 and the run's lcov coverage 0.000. | test it |
| `fnd_bda746` | `crap.watch` | `src/cljfx/coerce.clj:183` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_2919e3` | `crap.watch` | `src/cljfx/coerce.clj:189` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_eca59a` | `crap.watch` | `src/cljfx/coerce.clj:264` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_7a716a` | `crap.watch` | `src/cljfx/coerce.clj:304` | true | CRAP 20.0 recomputed from cyclomatic 4 and the run's lcov coverage 0.000. | test it |
| `fnd_7c9c10` | `crap.watch` | `src/cljfx/coerce.clj:315` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_451e36` | `crap.watch` | `src/cljfx/coerce.clj:390` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_5dfec9` | `crap.watch` | `src/cljfx/coerce.clj:400` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_f989ce` | `crap.watch` | `src/cljfx/coerce.clj:412` | true | CRAP 20.0 recomputed from cyclomatic 4 and the run's lcov coverage 0.000. | test it |
| `fnd_92b4d1` | `crap.watch` | `src/cljfx/coerce.clj:45` | true | CRAP 7.6 recomputed from cyclomatic 3 and the run's lcov coverage 0.200. | test it |
| `fnd_9f3373` | `crap.watch` | `src/cljfx/coerce.clj:468` | true | CRAP 6.0 recomputed from cyclomatic 2 and the run's lcov coverage 0.000. | test it |
| `fnd_d6c7c9` | `crap.watch` | `src/cljfx/coerce.clj:495` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_ec9628` | `crap.watch` | `src/cljfx/coerce.clj:508` | true | CRAP 6.0 recomputed from cyclomatic 2 and the run's lcov coverage 0.000. | test it |
| `fnd_ec189f` | `crap.watch` | `src/cljfx/coerce.clj:511` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_bfd867` | `crap.watch` | `src/cljfx/coerce.clj:517` | true | CRAP 6.0 recomputed from cyclomatic 2 and the run's lcov coverage 0.000. | test it |
| `fnd_535c5b` | `crap.watch` | `src/cljfx/coerce.clj:54` | true | CRAP 20.0 recomputed from cyclomatic 4 and the run's lcov coverage 0.000. | test it |
| `fnd_5f24e5` | `crap.watch` | `src/cljfx/composite.clj:173` | true | CRAP 6.0 recomputed from cyclomatic 6 and the run's lcov coverage 1.000. | test it |
| `fnd_83c9a4` | `crap.watch` | `src/cljfx/composite.clj:52` | true | CRAP 7.0 recomputed from cyclomatic 7 and the run's lcov coverage 1.000. | test it |
| `fnd_bccb7c` | `crap.watch` | `src/cljfx/context.clj:161` | true | CRAP 5.7 recomputed from cyclomatic 3 and the run's lcov coverage 0.333. | test it |
| `fnd_31f3bf` | `crap.watch` | `src/cljfx/context.clj:170` | true | CRAP 6.0 recomputed from cyclomatic 2 and the run's lcov coverage 0.000. | test it |
| `fnd_5d4b8c` | `crap.watch` | `src/cljfx/context.clj:43` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_149bbb` | `crap.watch` | `src/cljfx/event_handler.clj:22` | true | CRAP 9.3 recomputed from cyclomatic 3 and the run's lcov coverage 0.111. | test it |
| `fnd_818bcb` | `crap.watch` | `src/cljfx/event_handler.clj:35` | true | CRAP 6.0 recomputed from cyclomatic 2 and the run's lcov coverage 0.000. | test it |
| `fnd_57b818` | `crap.watch` | `src/cljfx/ext/cell_factory.clj:17` | true | CRAP 20.0 recomputed from cyclomatic 4 and the run's lcov coverage 0.000. | test it |
| `fnd_1077a1` | `crap.watch` | `src/cljfx/ext/cell_factory.clj:30` | true | CRAP 6.0 recomputed from cyclomatic 2 and the run's lcov coverage 0.000. | test it |
| `fnd_953b8d` | `crap.watch` | `src/cljfx/ext/web_view.clj:15` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_85d7f8` | `crap.watch` | `src/cljfx/ext/web_view.clj:9` | true | CRAP 20.0 recomputed from cyclomatic 4 and the run's lcov coverage 0.000. | test it |
| `fnd_98ffc6` | `crap.watch` | `src/cljfx/fx/animation.clj:12` | true | CRAP 6.0 recomputed from cyclomatic 2 and the run's lcov coverage 0.000. | test it |
| `fnd_7bd872` | `crap.watch` | `src/cljfx/fx/combo_box.clj:17` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_80a545` | `crap.watch` | `src/cljfx/fx/combo_box.clj:25` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_f2400f` | `crap.watch` | `src/cljfx/fx/combo_box.clj:31` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_439ae2` | `crap.watch` | `src/cljfx/fx/dialog.clj:13` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_7e2626` | `crap.watch` | `src/cljfx/fx/displacement_map.clj:10` | true | CRAP 6.0 recomputed from cyclomatic 2 and the run's lcov coverage 0.000. | test it |
| `fnd_cc6522` | `crap.watch` | `src/cljfx/fx/displacement_map.clj:16` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_f2400f` | `crap.watch` | `src/cljfx/fx/list_spinner_value_factory.clj:13` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_7bd872` | `crap.watch` | `src/cljfx/fx/list_view.clj:18` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_10c96f` | `crap.watch` | `src/cljfx/fx/media_player.clj:11` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_34d2d2` | `crap.watch` | `src/cljfx/fx/node.clj:16` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_953b8d` | `crap.watch` | `src/cljfx/fx/pagination.clj:13` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_444934` | `crap.watch` | `src/cljfx/fx/table_column.clj:14` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_7ebd55` | `crap.watch` | `src/cljfx/fx/table_column.clj:23` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_b6c14f` | `crap.watch` | `src/cljfx/fx/table_view.clj:26` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_59f73e` | `crap.watch` | `src/cljfx/fx/text_field_list_cell.clj:32` | true | CRAP 6.0 recomputed from cyclomatic 2 and the run's lcov coverage 0.000. | test it |
| `fnd_76f7be` | `crap.watch` | `src/cljfx/fx/text_formatter.clj:10` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_ea6085` | `crap.watch` | `src/cljfx/fx/tree_table_column.clj:15` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_804784` | `crap.watch` | `src/cljfx/fx/tree_table_column.clj:29` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_ed599e` | `crap.watch` | `src/cljfx/fx/tree_table_view.clj:16` | true | CRAP 20.0 recomputed from cyclomatic 4 and the run's lcov coverage 0.000. | test it |
| `fnd_b6c14f` | `crap.watch` | `src/cljfx/fx/tree_table_view.clj:23` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_b18f86` | `crap.watch` | `src/cljfx/fx/tree_view.clj:17` | true | CRAP 12.0 recomputed from cyclomatic 3 and the run's lcov coverage 0.000. | test it |
| `fnd_cb027b` | `crap.watch` | `src/cljfx/fx/triangle_mesh.clj:10` | true | CRAP 20.0 recomputed from cyclomatic 4 and the run's lcov coverage 0.000. | test it |
| `fnd_fce15a` | `crap.watch` | `src/cljfx/lifecycle.clj:135` | true | CRAP 5.0 recomputed from cyclomatic 5 and the run's lcov coverage 0.944. | test it |
| `fnd_ab453e` | `crap.watch` | `src/cljfx/lifecycle.clj:243` | true | CRAP 9.2 recomputed from cyclomatic 4 and the run's lcov coverage 0.312. | test it |
| `fnd_8175c4` | `crap.watch` | `src/cljfx/lifecycle.clj:287` | true | CRAP 14.4 recomputed from cyclomatic 5 and the run's lcov coverage 0.278. | test it |
| `fnd_82bb46` | `crap.watch` | `src/cljfx/lifecycle.clj:324` | true | CRAP 27.0 recomputed from cyclomatic 27 and the run's lcov coverage 0.987. | test it |
| `fnd_a950c2` | `crap.watch` | `src/cljfx/lifecycle.clj:477` | true | CRAP 5.0 recomputed from cyclomatic 5 and the run's lcov coverage 1.000. | test it |
| `fnd_56de23` | `crap.watch` | `src/cljfx/lifecycle.clj:623` | true | CRAP 5.0 recomputed from cyclomatic 5 and the run's lcov coverage 1.000. | test it |
| `fnd_7c34f8` | `crap.watch` | `src/cljfx/lifecycle.clj:661` | true | CRAP 27.7 recomputed from cyclomatic 5 and the run's lcov coverage 0.031. | test it |
| `fnd_2c8a11` | `crap.watch` | `src/cljfx/lifecycle.clj:781` | true | CRAP 15.7 recomputed from cyclomatic 4 and the run's lcov coverage 0.100. | test it |
| `fnd_966ba2` | `crap.watch` | `src/cljfx/lifecycle.clj:844` | true | CRAP 5.0 recomputed from cyclomatic 5 and the run's lcov coverage 1.000. | test it |
| `fnd_d99eb3` | `crap.watch` | `src/cljfx/mutator.clj:119` | true | CRAP 7.2 recomputed from cyclomatic 3 and the run's lcov coverage 0.222. | test it |
| `fnd_9ac9b0` | `crap.watch` | `src/cljfx/renderer.clj:49` | true | CRAP 7.0 recomputed from cyclomatic 5 and the run's lcov coverage 0.571. | test it |
| `fnd_95fc2b` | `crap.watch` | `src/cljfx/renderer.clj:67` | true | CRAP 6.0 recomputed from cyclomatic 2 and the run's lcov coverage 0.000. | test it |

**`static.dead-code`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_e95bc4` | `dead.export` | `build.clj:28` | false | `deps.edn`'s `:build {:ns-default build}` makes `clj -T:build <fn>` call it by name, a path the check does not see. | Cupel defect (open item) |
| `fnd_cd0e7f` | `dead.export` | `build.clj:31` | false | `deps.edn`'s `:build {:ns-default build}` makes `clj -T:build <fn>` call it by name, a path the check does not see. | Cupel defect (open item) |
| `fnd_7711c8` | `dead.export` | `build.clj:42` | false | `deps.edn`'s `:build {:ns-default build}` makes `clj -T:build <fn>` call it by name, a path the check does not see. | Cupel defect (open item) |
| `fnd_334cb1` | `dead.export` | `build.clj:65` | false | `deps.edn`'s `:build {:ns-default build}` makes `clj -T:build <fn>` call it by name, a path the check does not see. | Cupel defect (open item) |
| `fnd_6d23b5` | `dead.export` | `examples/e02_fn.clj:4` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_c3f6a6` | `dead.export` | `examples/e03_map_event_handler.clj:18` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_f8a012` | `dead.export` | `examples/e04_state_with_context.clj:11` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_cf9d8c` | `dead.export` | `examples/e04_state_with_context.clj:19` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_5e4d10` | `dead.export` | `examples/e05_fn_fx_like_state.clj:16` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_1e7655` | `dead.export` | `examples/e05_fn_fx_like_state.clj:39` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_df80b1` | `dead.export` | `examples/e05_fn_fx_like_state.clj:8` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_5a3d5b` | `dead.export` | `examples/e08_media.clj:14` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_eaf0b6` | `dead.export` | `examples/e08_media.clj:24` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_9e2e8d` | `dead.export` | `examples/e08_media.clj:42` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_705433` | `dead.export` | `examples/e08_media.clj:70` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_2642be` | `dead.export` | `examples/e08_media.clj:8` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_95041d` | `dead.export` | `examples/e09_todo_app.clj:14` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_4ad4fd` | `dead.export` | `examples/e09_todo_app.clj:25` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_98d216` | `dead.export` | `examples/e09_todo_app.clj:49` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_b38fdb` | `dead.export` | `examples/e10_multiple_windows.clj:5` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_8b9503` | `dead.export` | `examples/e17_dialogs.clj:10` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_080f20` | `dead.export` | `examples/e17_dialogs.clj:24` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_4e5804` | `dead.export` | `examples/e17_dialogs.clj:38` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_da19b7` | `dead.export` | `examples/e17_dialogs.clj:51` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_bfa93a` | `dead.export` | `examples/e17_dialogs.clj:64` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_817c78` | `dead.export` | `examples/e18_pure_event_handling.clj:28` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_d80147` | `dead.export` | `examples/e21_extension_lifecycles.clj:19` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_863881` | `dead.export` | `examples/e21_extension_lifecycles.clj:29` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_528a1d` | `dead.export` | `examples/e21_extension_lifecycles.clj:39` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_57db72` | `dead.export` | `examples/e21_extension_lifecycles.clj:67` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_25ecb3` | `dead.export` | `examples/e25_radio_buttons.clj:7` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_084e38` | `dead.export` | `examples/e27_selection_models.clj:102` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_e10116` | `dead.export` | `examples/e27_selection_models.clj:11` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_b98006` | `dead.export` | `examples/e27_selection_models.clj:135` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_bff23a` | `dead.export` | `examples/e27_selection_models.clj:159` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_7dec09` | `dead.export` | `examples/e27_selection_models.clj:192` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_60f32b` | `dead.export` | `examples/e27_selection_models.clj:199` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_1bbc66` | `dead.export` | `examples/e27_selection_models.clj:219` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_7b8700` | `dead.export` | `examples/e27_selection_models.clj:290` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_004c99` | `dead.export` | `examples/e27_selection_models.clj:293` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_cfbbe9` | `dead.export` | `examples/e27_selection_models.clj:31` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_55da8b` | `dead.export` | `examples/e27_selection_models.clj:68` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_dbf384` | `dead.export` | `examples/e28_canvas.clj:9` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_258fe8` | `dead.export` | `examples/e29_text_formatter.clj:15` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_258fe8` | `dead.export` | `examples/e29_text_formatter.clj:24` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_4eb345` | `dead.export` | `examples/e29_text_formatter.clj:8` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_ab0945` | `dead.export` | `examples/e31_indefinite_transitions.clj:207` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_544a4f` | `dead.export` | `examples/e31_indefinite_transitions.clj:235` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_28d261` | `dead.export` | `examples/e31_indefinite_transitions.clj:45` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_7a4d23` | `dead.export` | `examples/e31_indefinite_transitions.clj:50` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_f835a7` | `dead.export` | `examples/e31_indefinite_transitions.clj:61` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_99e21e` | `dead.export` | `examples/e31_indefinite_transitions.clj:80` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_861e1c` | `dead.export` | `examples/e34_web_browser.clj:29` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_e00e96` | `dead.export` | `examples/e34_web_browser.clj:42` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_85e277` | `dead.export` | `examples/e34_web_browser.clj:49` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_efc5ad` | `dead.export` | `examples/e34_web_browser.clj:58` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_4eff16` | `dead.export` | `examples/e34_web_browser.clj:65` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_d2249e` | `dead.export` | `examples/e34_web_browser.clj:70` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_be2f41` | `dead.export` | `examples/e35_popup.clj:31` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_b036a3` | `dead.export` | `examples/e36_pseudo_classes.clj:24` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_bf5f04` | `dead.export` | `examples/e36_pseudo_classes.clj:34` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_3dff63` | `dead.export` | `examples/e39_web_view_extensions.clj:15` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_f2db56` | `dead.export` | `examples/e40_dynamic_combo_box.clj:12` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_3da5b3` | `dead.export` | `examples/e40_dynamic_combo_box.clj:15` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_3debc8` | `dead.export` | `examples/e40_dynamic_combo_box.clj:9` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_695acd` | `dead.export` | `examples/e44_nested_form_view.clj:12` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_349f59` | `dead.export` | `examples/e44_nested_form_view.clj:27` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_5bc1d4` | `dead.export` | `examples/e44_nested_form_view.clj:33` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_4d938a` | `dead.export` | `examples/e44_nested_form_view.clj:40` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_c35a87` | `dead.export` | `examples/e44_nested_form_view.clj:61` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_171310` | `dead.export` | `examples/e44_nested_form_view.clj:70` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_89405a` | `dead.export` | `examples/e44_nested_form_view.clj:92` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_7b999e` | `dead.export` | `examples/e45_git_ui.clj:151` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_bda8e3` | `dead.export` | `examples/e45_git_ui.clj:165` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_ee727f` | `dead.export` | `examples/e45_git_ui.clj:178` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_56eb67` | `dead.export` | `examples/e45_git_ui.clj:61` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_f6eb84` | `dead.export` | `examples/e47_check_box_tree.clj:7` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_570c7a` | `dead.export` | `src/cljfx/fx/combo_box.clj:17` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_75df15` | `dead.export` | `src/cljfx/fx/combo_box.clj:25` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_570c7a` | `dead.export` | `src/cljfx/fx/list_view.clj:18` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_6b4fb7` | `dead.export` | `src/cljfx/fx/table_column.clj:14` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_b3481f` | `dead.export` | `src/cljfx/fx/table_column.clj:23` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_aa48ca` | `dead.export` | `src/cljfx/fx/tree_table_column.clj:29` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_b484b1` | `dead.export` | `src/cljfx/fx/tree_view.clj:17` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_606e54` | `dead.export` | `src/cljfx/fx/triangle_mesh.clj:10` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_255d02` | `dead.symbol` | `examples/e07_extra_props.clj:95` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_1a1942` | `dead.symbol` | `examples/e16_cell_factories.clj:40` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_c82804` | `dead.symbol` | `examples/e26_tooltips.clj:13` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_5640a4` | `dead.symbol` | `examples/e41_ext_watcher_concurrency.clj:6` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_826079` | `dead.symbol` | `examples/e42_local_state.clj:4` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_89f97f` | `dead.symbol` | `examples/e42_local_state.clj:9` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_a69087` | `dead.symbol` | `examples/e43_nested_local_state.clj:17` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_db5bfd` | `dead.symbol` | `examples/e43_nested_local_state.clj:4` | false | The example's top-level forms (`fx/mount-renderer`, `def renderer`, `fx/on-fx-thread`) use it when the file is loaded, which is how examples run; the module pass sees no import and no root. | Cupel defect (open item) |
| `fnd_8dc531` | `dead.symbol` | `src/cljfx/fx/date_picker.clj:15` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_1f2a78` | `dead.symbol` | `src/cljfx/fx/displacement_map.clj:10` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_408a30` | `dead.symbol` | `src/cljfx/fx/displacement_map.clj:16` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_eef2a7` | `dead.symbol` | `src/cljfx/fx/list_spinner_value_factory.clj:13` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_a94fd9` | `dead.symbol` | `src/cljfx/fx/media_player.clj:11` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_bd9c64` | `dead.symbol` | `src/cljfx/fx/pagination.clj:13` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_234d64` | `dead.symbol` | `src/cljfx/fx/scene.clj:13` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_87245a` | `dead.symbol` | `src/cljfx/fx/scroll_pane.clj:13` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_487037` | `dead.symbol` | `src/cljfx/fx/table_view.clj:16` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_c27ef7` | `dead.symbol` | `src/cljfx/fx/table_view.clj:26` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_a1f7dc` | `dead.symbol` | `src/cljfx/fx/table_view.clj:32` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_131ad1` | `dead.symbol` | `src/cljfx/fx/text_formatter.clj:10` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_f8e223` | `dead.symbol` | `src/cljfx/fx/text_formatter.clj:23` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_d53850` | `dead.symbol` | `src/cljfx/fx/tree_table_view.clj:16` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_c27ef7` | `dead.symbol` | `src/cljfx/fx/tree_table_view.clj:23` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_a1f7dc` | `dead.symbol` | `src/cljfx/fx/tree_table_view.clj:29` | false | Survey 1 only: `cljfx.fx` loads the namespace by `requiring-resolve` through `lazy-load`, which the module pass did not see (fixed in f6b9d3c). | fixed |
| `fnd_4b354f` | `dead.export` | `dev/user.clj:8` | disputed | Nothing calls it (correct); it is a REPL utility of the `user` namespace the `:dev` alias loads. | none |

**`static.deps`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_8e8e63` | `dep.cycle` | `src/cljfx/ext/cell_factory.clj:6` | disputed | The cycle is in the source (`cljfx.ext.cell-factory` requires `cljfx.fx`, whose registry lazily loads `cljfx.fx.combo-box`, which requires `cell-factory`), but the lazy `requiring-resolve` is what lets it load: the idiom breaks it. | none |

**`tests.assertion-density`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_30ec75` | `density.sparse-file` | `test/cljfx/context_test.clj:6` | false | Survey 1 only: the file's testit assertions were not counted (fixed in 3003be2). | fixed |
| `fnd_8e3b62` | `density.sparse-file` | `test/cljfx/lifecycle_test.clj:14` | false | Survey 1 only: the file's testit assertions were not counted (fixed in 3003be2). | fixed |
| `fnd_e9c0b5` | `density.sparse-test` | `test/cljfx/event_handlers_test.clj:20` | true | 1 assertion(s) in 52 lines, recounted; 31% of the lines inside assertions. | assert more |
| `fnd_5ab219` | `density.sparse-test` | `test/cljfx/lifecycle_test.clj:429` | true | 3 assertion(s) in 66 lines, recounted; 30% of the lines inside assertions. | assert more |
| `fnd_8698f1` | `density.sparse-test` | `test/cljfx/lifecycle_test.clj:935` | true | 1 assertion(s) in 39 lines, recounted; 18% of the lines inside assertions. | assert more |
| `fnd_d08fbf` | `density.sparse-test` | `test/cljfx/local_state_test.clj:8` | true | 1 assertion(s) in 60 lines, recounted; 40% of the lines inside assertions. | assert more |
| `fnd_3f5e05` | `density.sparse-test` | `test/cljfx/props_test.clj:32` | true | 2 assertion(s) in 59 lines, recounted; 17% of the lines inside assertions. | assert more |
| `fnd_ddbade` | `density.sparse-test` | `test/cljfx/watcher_test.clj:10` | true | 1 assertion(s) in 27 lines, recounted; 4% of the lines inside assertions. | assert more |

**`tests.coverage`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_9da3d9` | `coverage.uncovered-symbol` | `src/cljfx/api.clj:247` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_2edbd8` | `coverage.uncovered-symbol` | `src/cljfx/api.clj:366` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_014aab` | `coverage.uncovered-symbol` | `src/cljfx/api.clj:388` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_760666` | `coverage.uncovered-symbol` | `src/cljfx/api.clj:446` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_329764` | `coverage.uncovered-symbol` | `src/cljfx/api.clj:498` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_db29ce` | `coverage.uncovered-symbol` | `src/cljfx/api.clj:504` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_ce7ec4` | `coverage.uncovered-symbol` | `src/cljfx/api.clj:509` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_e821a8` | `coverage.uncovered-symbol` | `src/cljfx/api.clj:518` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_b0b4f5` | `coverage.uncovered-symbol` | `src/cljfx/api.clj:528` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_0a00ab` | `coverage.uncovered-symbol` | `src/cljfx/api.clj:533` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_b75eb0` | `coverage.uncovered-symbol` | `src/cljfx/api.clj:550` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_e87962` | `coverage.uncovered-symbol` | `src/cljfx/api.clj:620` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_a4f20d` | `coverage.uncovered-symbol` | `src/cljfx/api.clj:636` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_147d1b` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:102` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_a7697e` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:110` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_dd46c1` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:120` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_c2c17f` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:140` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_8f1c08` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:152` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_fad2ed` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:162` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_9de9b5` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:167` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_b3741e` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:174` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_80b10b` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:183` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_da2526` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:189` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_e6f097` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:195` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_0b3229` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:211` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_d2a73f` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:235` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_3431a5` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:253` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_d81095` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:264` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_6f480e` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:278` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_cb6c36` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:291` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_47f0a0` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:304` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_5cac70` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:315` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_df3b98` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:331` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_028f44` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:346` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_0ac7a9` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:36` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_302752` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:390` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_9506f3` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:400` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_d014d0` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:412` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_0fa860` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:42` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_6d6e82` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:420` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_45916b` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:446` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_ae3407` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:468` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_ef138f` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:473` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_ad9661` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:495` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_e09287` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:508` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_52b8d2` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:511` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_3294d1` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:517` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_5ebe09` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:54` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_8d3e71` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:61` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_4a12be` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:65` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_cc67d4` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:74` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_ef2799` | `coverage.uncovered-symbol` | `src/cljfx/coerce.clj:84` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_492aa7` | `coverage.uncovered-symbol` | `src/cljfx/context.clj:170` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_4cb0ac` | `coverage.uncovered-symbol` | `src/cljfx/context.clj:43` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_37cb8d` | `coverage.uncovered-symbol` | `src/cljfx/defaults.clj:14` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_f08b71` | `coverage.uncovered-symbol` | `src/cljfx/defaults.clj:37` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_b8213b` | `coverage.uncovered-symbol` | `src/cljfx/event_handler.clj:19` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_089d72` | `coverage.uncovered-symbol` | `src/cljfx/event_handler.clj:35` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_9770b9` | `coverage.uncovered-symbol` | `src/cljfx/event_handler.clj:5` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_021ed1` | `coverage.uncovered-symbol` | `src/cljfx/ext/cell_factory.clj:14` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_dde82c` | `coverage.uncovered-symbol` | `src/cljfx/ext/cell_factory.clj:17` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_183fd3` | `coverage.uncovered-symbol` | `src/cljfx/ext/cell_factory.clj:25` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_7ca585` | `coverage.uncovered-symbol` | `src/cljfx/ext/cell_factory.clj:30` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_9855fc` | `coverage.uncovered-symbol` | `src/cljfx/ext/multiple_selection_model.clj:50` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_966443` | `coverage.uncovered-symbol` | `src/cljfx/ext/web_view.clj:15` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_39f255` | `coverage.uncovered-symbol` | `src/cljfx/ext/web_view.clj:9` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_c40235` | `coverage.uncovered-symbol` | `src/cljfx/fx/animation.clj:12` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_acb6c1` | `coverage.uncovered-symbol` | `src/cljfx/fx/combo_box.clj:17` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_4912c5` | `coverage.uncovered-symbol` | `src/cljfx/fx/combo_box.clj:25` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_8b25f2` | `coverage.uncovered-symbol` | `src/cljfx/fx/combo_box.clj:31` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_5b086b` | `coverage.uncovered-symbol` | `src/cljfx/fx/date_picker.clj:15` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_5ad028` | `coverage.uncovered-symbol` | `src/cljfx/fx/dialog.clj:13` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_e5e8ef` | `coverage.uncovered-symbol` | `src/cljfx/fx/dialog_pane.clj:11` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_36e15d` | `coverage.uncovered-symbol` | `src/cljfx/fx/displacement_map.clj:10` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_40cbf0` | `coverage.uncovered-symbol` | `src/cljfx/fx/displacement_map.clj:16` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_8b25f2` | `coverage.uncovered-symbol` | `src/cljfx/fx/list_spinner_value_factory.clj:13` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_acb6c1` | `coverage.uncovered-symbol` | `src/cljfx/fx/list_view.clj:18` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_36a81b` | `coverage.uncovered-symbol` | `src/cljfx/fx/list_view.clj:26` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_d8a81d` | `coverage.uncovered-symbol` | `src/cljfx/fx/media_player.clj:11` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_100473` | `coverage.uncovered-symbol` | `src/cljfx/fx/node.clj:16` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_966443` | `coverage.uncovered-symbol` | `src/cljfx/fx/pagination.clj:13` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_6b6e4b` | `coverage.uncovered-symbol` | `src/cljfx/fx/scene.clj:13` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_251dc9` | `coverage.uncovered-symbol` | `src/cljfx/fx/scroll_pane.clj:13` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_f06ac5` | `coverage.uncovered-symbol` | `src/cljfx/fx/table_cell.clj:33` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_6d5780` | `coverage.uncovered-symbol` | `src/cljfx/fx/table_column.clj:14` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_d3d5e3` | `coverage.uncovered-symbol` | `src/cljfx/fx/table_column.clj:23` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_d690ee` | `coverage.uncovered-symbol` | `src/cljfx/fx/table_view.clj:16` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_320e23` | `coverage.uncovered-symbol` | `src/cljfx/fx/table_view.clj:26` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_36a81b` | `coverage.uncovered-symbol` | `src/cljfx/fx/table_view.clj:32` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_73a0bb` | `coverage.uncovered-symbol` | `src/cljfx/fx/text_field_list_cell.clj:32` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_fb9af9` | `coverage.uncovered-symbol` | `src/cljfx/fx/text_formatter.clj:10` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_6a087a` | `coverage.uncovered-symbol` | `src/cljfx/fx/transition.clj:11` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_f06ac5` | `coverage.uncovered-symbol` | `src/cljfx/fx/tree_cell.clj:35` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_f06ac5` | `coverage.uncovered-symbol` | `src/cljfx/fx/tree_table_cell.clj:33` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_51a80e` | `coverage.uncovered-symbol` | `src/cljfx/fx/tree_table_column.clj:15` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_f10dcc` | `coverage.uncovered-symbol` | `src/cljfx/fx/tree_table_column.clj:29` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_0776d4` | `coverage.uncovered-symbol` | `src/cljfx/fx/tree_table_view.clj:16` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_320e23` | `coverage.uncovered-symbol` | `src/cljfx/fx/tree_table_view.clj:23` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_36a81b` | `coverage.uncovered-symbol` | `src/cljfx/fx/tree_table_view.clj:29` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_6cfc75` | `coverage.uncovered-symbol` | `src/cljfx/fx/tree_view.clj:17` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_36a81b` | `coverage.uncovered-symbol` | `src/cljfx/fx/tree_view.clj:25` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_30c91c` | `coverage.uncovered-symbol` | `src/cljfx/fx/triangle_mesh.clj:10` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_c8dd04` | `coverage.uncovered-symbol` | `src/cljfx/lifecycle.clj:450` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_8b14e1` | `coverage.uncovered-symbol` | `src/cljfx/lifecycle.clj:775` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_86e800` | `coverage.uncovered-symbol` | `src/cljfx/prop.clj:23` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_eca4cc` | `coverage.uncovered-symbol` | `src/cljfx/renderer.clj:67` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |
| `fnd_e79c62` | `coverage.uncovered-symbol` | `src/cljfx/renderer.clj:92` | true | Recomputed from the run's lcov (lines after the first, ADR-079): no line ran. | test it |

**`tests.mutation`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_3dac8f` | `mutant.uncovered` | `jdk11/cljfx/jdk/platform.clj:12` | false | `jdk11` is on `deps.edn`'s `:paths` and `initialize` runs in every test session, but the Cloverage command `cupel init` wrote instruments only `--src-ns-path src`, so the file is absent from the map, not unloaded. | Cupel defect (open item) |
| `fnd_262acb` | `mutant.survived` | `src/cljfx/composite.clj:128` | true | Replayed: the mutation applied to the clone leaves `clojure -M:test:runner` green (54 tests). | pin the case |
| `fnd_c42738` | `mutant.survived` | `src/cljfx/composite.clj:43` | true | Replayed: the mutation applied to the clone leaves `clojure -M:test:runner` green (54 tests). | pin the case |
| `fnd_8fe0c6` | `mutant.survived` | `src/cljfx/composite.clj:86` | disputed | Replayed three times: `reset-test` killed it once and the suite stayed green twice; a timing-dependent test on the JavaFX thread decides. | stabilize `reset-test` |
| `fnd_727807` | `mutant.survived` | `src/cljfx/context.clj:147` | true | Replayed: the mutation applied to the clone leaves `clojure -M:test:runner` green (54 tests). | pin the case |
| `fnd_57a806` | `mutant.survived` | `src/cljfx/lifecycle.clj:150` | true | Replayed: the mutation applied to the clone leaves `clojure -M:test:runner` green (54 tests). | pin the case |
| `fnd_147759` | `mutant.survived` | `src/cljfx/lifecycle.clj:155` | true | Replayed: the mutation applied to the clone leaves `clojure -M:test:runner` green (54 tests). | pin the case |
| `fnd_719df3` | `mutant.survived` | `src/cljfx/lifecycle.clj:821` | true | Replayed: the mutation applied to the clone leaves `clojure -M:test:runner` green (54 tests). | pin the case |
| `fnd_be62b5` | `mutant.survived` | `src/cljfx/lifecycle.clj:835` | true | Replayed: the mutation applied to the clone leaves `clojure -M:test:runner` green (54 tests). | pin the case |
| `fnd_9db11e` | `mutant.survived` | `src/cljfx/prop.clj:30` | true | Replayed: the mutation applied to the clone leaves `clojure -M:test:runner` green (54 tests). | pin the case |
| `fnd_92306b` | `mutant.survived` | `src/cljfx/renderer.clj:34` | true | Replayed: the mutation applied to the clone leaves `clojure -M:test:runner` green (54 tests). | pin the case |
| `fnd_f125be` | `mutant.survived` | `src/cljfx/renderer.clj:55` | true | Replayed: the mutation applied to the clone leaves `clojure -M:test:runner` green (54 tests). | pin the case |
| `fnd_8edd6a` | `mutant.uncovered` | `dev/user.clj:20` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_50a4ef` | `mutant.uncovered` | `dev/user.clj:22` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_f25899` | `mutant.uncovered` | `dev/user.clj:22` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_c4b532` | `mutant.uncovered` | `dev/user.clj:61` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_ef013a` | `mutant.uncovered` | `dev/user.clj:84` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_70b73e` | `mutant.uncovered` | `dev/user.clj:86` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_4f0276` | `mutant.uncovered` | `example-projects/splash/src/splash.clj:11` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_66b648` | `mutant.uncovered` | `examples/e03_map_event_handler.clj:19` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_66a44a` | `mutant.uncovered` | `examples/e03_map_event_handler.clj:19` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_333151` | `mutant.uncovered` | `examples/e03_map_event_handler.clj:20` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_3f6db8` | `mutant.uncovered` | `examples/e04_state_with_context.clj:23` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_3be5a1` | `mutant.uncovered` | `examples/e05_fn_fx_like_state.clj:18` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_1c0a7a` | `mutant.uncovered` | `examples/e07_extra_props.clj:100` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_044624` | `mutant.uncovered` | `examples/e07_extra_props.clj:99` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_95d9b0` | `mutant.uncovered` | `examples/e08_media.clj:16` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_498a04` | `mutant.uncovered` | `examples/e08_media.clj:17` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_3eb067` | `mutant.uncovered` | `examples/e08_media.clj:44` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_0f9f29` | `mutant.uncovered` | `examples/e08_media.clj:48` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_fd6bbc` | `mutant.uncovered` | `examples/e08_media.clj:60` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_5b4725` | `mutant.uncovered` | `examples/e09_todo_app.clj:27` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_01b2cf` | `mutant.uncovered` | `examples/e09_todo_app.clj:34` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_75d9fc` | `mutant.uncovered` | `examples/e09_todo_app.clj:53` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_87f1d8` | `mutant.uncovered` | `examples/e09_todo_app.clj:59` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_45a9a6` | `mutant.uncovered` | `examples/e10_multiple_windows.clj:7` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_50ab43` | `mutant.uncovered` | `examples/e10_multiple_windows.clj:8` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_08b69b` | `mutant.uncovered` | `examples/e11_mouse_dragging.clj:28` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_f13aae` | `mutant.uncovered` | `examples/e12_interactive_development.clj:103` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_e46de1` | `mutant.uncovered` | `examples/e12_interactive_development.clj:104` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_708d03` | `mutant.uncovered` | `examples/e12_interactive_development.clj:128` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_66c2f6` | `mutant.uncovered` | `examples/e12_interactive_development.clj:139` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_28b55e` | `mutant.uncovered` | `examples/e12_interactive_development.clj:144` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_ac1ac7` | `mutant.uncovered` | `examples/e12_interactive_development.clj:145` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_32b285` | `mutant.uncovered` | `examples/e12_interactive_development.clj:155` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_dca9aa` | `mutant.uncovered` | `examples/e12_interactive_development.clj:155` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_a98a3c` | `mutant.uncovered` | `examples/e12_interactive_development.clj:155` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_0bfd5a` | `mutant.uncovered` | `examples/e12_interactive_development.clj:155` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_6ab627` | `mutant.uncovered` | `examples/e12_interactive_development.clj:156` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_fc7b93` | `mutant.uncovered` | `examples/e12_interactive_development.clj:158` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_603760` | `mutant.uncovered` | `examples/e12_interactive_development.clj:158` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_1037cc` | `mutant.uncovered` | `examples/e12_interactive_development.clj:169` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_6ff73f` | `mutant.uncovered` | `examples/e12_interactive_development.clj:200` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_a20659` | `mutant.uncovered` | `examples/e12_interactive_development.clj:204` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_45fb97` | `mutant.uncovered` | `examples/e12_interactive_development.clj:215` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_b64cc0` | `mutant.uncovered` | `examples/e12_interactive_development.clj:221` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_6b7e27` | `mutant.uncovered` | `examples/e12_interactive_development.clj:222` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_a20659` | `mutant.uncovered` | `examples/e12_interactive_development.clj:237` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_45fb97` | `mutant.uncovered` | `examples/e12_interactive_development.clj:248` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_64fca7` | `mutant.uncovered` | `examples/e12_interactive_development.clj:254` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_9dc419` | `mutant.uncovered` | `examples/e12_interactive_development.clj:255` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_2cce7b` | `mutant.uncovered` | `examples/e12_interactive_development.clj:26` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_b32ac5` | `mutant.uncovered` | `examples/e12_interactive_development.clj:58` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_27c01b` | `mutant.uncovered` | `examples/e12_interactive_development.clj:62` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_577d42` | `mutant.uncovered` | `examples/e12_interactive_development.clj:66` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_511157` | `mutant.uncovered` | `examples/e12_interactive_development.clj:67` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_c869b1` | `mutant.uncovered` | `examples/e12_interactive_development.clj:89` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_0084c7` | `mutant.uncovered` | `examples/e12_interactive_development.clj:93` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_f2572f` | `mutant.uncovered` | `examples/e12_interactive_development.clj:98` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_322c20` | `mutant.uncovered` | `examples/e13_re_frame_like_state.clj:102` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_369cb3` | `mutant.uncovered` | `examples/e13_re_frame_like_state.clj:106` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_528eda` | `mutant.uncovered` | `examples/e13_re_frame_like_state.clj:106` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_d31d85` | `mutant.uncovered` | `examples/e13_re_frame_like_state.clj:185` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_1cc190` | `mutant.uncovered` | `examples/e15_task_tracker.clj:150` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_b67cc6` | `mutant.uncovered` | `examples/e15_task_tracker.clj:164` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_51dfc3` | `mutant.uncovered` | `examples/e15_task_tracker.clj:169` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_bfe18c` | `mutant.uncovered` | `examples/e17_dialogs.clj:12` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_f4846d` | `mutant.uncovered` | `examples/e17_dialogs.clj:26` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_84e5e8` | `mutant.uncovered` | `examples/e17_dialogs.clj:41` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_dab636` | `mutant.uncovered` | `examples/e17_dialogs.clj:53` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_799df4` | `mutant.uncovered` | `examples/e17_dialogs.clj:66` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_6a89b4` | `mutant.uncovered` | `examples/e18_pure_event_handling.clj:32` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_fdf6e8` | `mutant.uncovered` | `examples/e18_pure_event_handling/events.clj:18` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_b1627e` | `mutant.uncovered` | `examples/e18_pure_event_handling/events.clj:18` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_32de74` | `mutant.uncovered` | `examples/e18_pure_event_handling/events.clj:19` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_cbb964` | `mutant.uncovered` | `examples/e18_pure_event_handling/events.clj:45` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_50ea24` | `mutant.uncovered` | `examples/e18_pure_event_handling/events.clj:46` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_fe1b18` | `mutant.uncovered` | `examples/e18_pure_event_handling/views.clj:100` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_590d79` | `mutant.uncovered` | `examples/e18_pure_event_handling/views.clj:21` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_23a48e` | `mutant.uncovered` | `examples/e18_pure_event_handling/views.clj:34` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_b70116` | `mutant.uncovered` | `examples/e18_pure_event_handling/views.clj:56` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_2b2045` | `mutant.uncovered` | `examples/e18_pure_event_handling/views.clj:58` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_5f4dc9` | `mutant.uncovered` | `examples/e18_pure_event_handling/views.clj:63` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_cd494f` | `mutant.uncovered` | `examples/e18_pure_event_handling/views.clj:64` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_1197c3` | `mutant.uncovered` | `examples/e18_pure_event_handling/views.clj:71` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_d23d65` | `mutant.uncovered` | `examples/e18_pure_event_handling/views.clj:73` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_eca705` | `mutant.uncovered` | `examples/e18_pure_event_handling/views.clj:93` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_6c7051` | `mutant.uncovered` | `examples/e19_instant_repl.clj:20` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_201a77` | `mutant.uncovered` | `examples/e19_instant_repl.clj:36` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_6ca3a5` | `mutant.uncovered` | `examples/e19_instant_repl.clj:41` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_1afe1f` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:101` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_173b9d` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:112` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_1afe1f` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:114` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_bac44c` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:130` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_de6502` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:130` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_15d73c` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:130` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_e56d30` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:131` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_540138` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:139` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_7f9e29` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:139` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_92e810` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:139` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_3293dd` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:140` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_5913c6` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:142` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_346498` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:147` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_3af585` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:154` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_6a581d` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:154` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_809843` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:154` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_b624b9` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:155` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_f1b90e` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:157` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_4ed032` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:183` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_6e0472` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:194` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_3175b2` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:209` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_d25f62` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:227` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_0e2984` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:241` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_5cf62b` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:245` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_32ec2a` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:83` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_5dc6c4` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:89` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_173b9d` | `mutant.uncovered` | `examples/e20_markdown_editor.clj:99` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_17f642` | `mutant.uncovered` | `examples/e21_extension_lifecycles.clj:12` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_697654` | `mutant.uncovered` | `examples/e21_extension_lifecycles.clj:49` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_f1e9b0` | `mutant.uncovered` | `examples/e21_extension_lifecycles.clj:56` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_48a5bc` | `mutant.uncovered` | `examples/e22_button_with_confirmation_dialog.clj:24` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_2018fc` | `mutant.uncovered` | `examples/e22_button_with_confirmation_dialog.clj:30` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_c2ff34` | `mutant.uncovered` | `examples/e22_button_with_confirmation_dialog.clj:33` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_20628f` | `mutant.uncovered` | `examples/e22_button_with_confirmation_dialog.clj:40` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_eb582b` | `mutant.uncovered` | `examples/e22_button_with_confirmation_dialog.clj:54` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_d6f5ca` | `mutant.uncovered` | `examples/e23_accelerators.clj:10` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_ba37a1` | `mutant.uncovered` | `examples/e25_radio_buttons.clj:17` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_01c8d7` | `mutant.uncovered` | `examples/e27_selection_models.clj:175` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_e8e63b` | `mutant.uncovered` | `examples/e27_selection_models.clj:201` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_d24d72` | `mutant.uncovered` | `examples/e27_selection_models.clj:202` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_c2f1c6` | `mutant.uncovered` | `examples/e27_selection_models.clj:208` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_dd8e31` | `mutant.uncovered` | `examples/e27_selection_models.clj:210` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_a59d91` | `mutant.uncovered` | `examples/e27_selection_models.clj:213` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_152770` | `mutant.uncovered` | `examples/e27_selection_models.clj:213` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_7debf9` | `mutant.uncovered` | `examples/e27_selection_models.clj:214` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_c32628` | `mutant.uncovered` | `examples/e27_selection_models.clj:222` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_8ac2c5` | `mutant.uncovered` | `examples/e27_selection_models.clj:226` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_46d0e0` | `mutant.uncovered` | `examples/e27_selection_models.clj:227` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_b57b76` | `mutant.uncovered` | `examples/e27_selection_models.clj:81` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_193d43` | `mutant.uncovered` | `examples/e27_selection_models.clj:90` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_620e4b` | `mutant.uncovered` | `examples/e27_selection_models.clj:91` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_04dbb3` | `mutant.uncovered` | `examples/e28_canvas.clj:15` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_953e73` | `mutant.uncovered` | `examples/e28_canvas.clj:15` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_7b41d2` | `mutant.uncovered` | `examples/e28_canvas.clj:17` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_b8aa3e` | `mutant.uncovered` | `examples/e28_canvas.clj:17` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_bb8220` | `mutant.uncovered` | `examples/e28_canvas.clj:19` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_8229be` | `mutant.uncovered` | `examples/e28_canvas.clj:19` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_62c257` | `mutant.uncovered` | `examples/e28_canvas.clj:19` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_5ca9fc` | `mutant.uncovered` | `examples/e30_devtools_via_event_filters.clj:45` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_c68cd5` | `mutant.uncovered` | `examples/e30_devtools_via_event_filters.clj:47` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_5d0c5e` | `mutant.uncovered` | `examples/e30_devtools_via_event_filters.clj:47` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_a4a29d` | `mutant.uncovered` | `examples/e30_devtools_via_event_filters.clj:49` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_6ed279` | `mutant.uncovered` | `examples/e30_devtools_via_event_filters.clj:65` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_885c05` | `mutant.uncovered` | `examples/e30_devtools_via_event_filters.clj:76` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_1fae4b` | `mutant.uncovered` | `examples/e31_indefinite_transitions.clj:209` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_334cd9` | `mutant.uncovered` | `examples/e31_indefinite_transitions.clj:210` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_48a1d4` | `mutant.uncovered` | `examples/e31_indefinite_transitions.clj:63` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_0d7e3f` | `mutant.uncovered` | `examples/e31_indefinite_transitions.clj:64` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_67c0e8` | `mutant.uncovered` | `examples/e31_indefinite_transitions.clj:67` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_7b66e0` | `mutant.uncovered` | `examples/e31_indefinite_transitions.clj:69` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_6a896e` | `mutant.uncovered` | `examples/e31_indefinite_transitions.clj:75` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_168f2b` | `mutant.uncovered` | `examples/e31_indefinite_transitions.clj:82` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_c5d30a` | `mutant.uncovered` | `examples/e31_indefinite_transitions.clj:87` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_bc38f0` | `mutant.uncovered` | `examples/e31_indefinite_transitions.clj:88` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_73a382` | `mutant.uncovered` | `examples/e31_indefinite_transitions.clj:91` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_e279ba` | `mutant.uncovered` | `examples/e31_indefinite_transitions.clj:92` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_0ddad8` | `mutant.uncovered` | `examples/e32_editable_table_cell_with_datascript.clj:116` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_d1a04c` | `mutant.uncovered` | `examples/e32_editable_table_cell_with_datascript.clj:135` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_d21a4e` | `mutant.uncovered` | `examples/e32_editable_table_cell_with_datascript.clj:143` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_88bbeb` | `mutant.uncovered` | `examples/e32_editable_table_cell_with_datascript.clj:45` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_6d0c61` | `mutant.uncovered` | `examples/e32_editable_table_cell_with_datascript.clj:61` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_027bcd` | `mutant.uncovered` | `examples/e32_editable_table_cell_with_datascript.clj:97` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_24ff01` | `mutant.uncovered` | `examples/e33_file_chooser.clj:22` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_f833ae` | `mutant.uncovered` | `examples/e33_file_chooser.clj:39` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_49cb91` | `mutant.uncovered` | `examples/e34_web_browser.clj:60` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_fc5e42` | `mutant.uncovered` | `examples/e35_popup.clj:11` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_0cb11e` | `mutant.uncovered` | `examples/e35_popup.clj:14` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_9a83ff` | `mutant.uncovered` | `examples/e35_popup.clj:33` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_6af3b1` | `mutant.uncovered` | `examples/e35_popup.clj:45` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_db5662` | `mutant.uncovered` | `examples/e35_popup.clj:46` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_ee7ab4` | `mutant.uncovered` | `examples/e36_pseudo_classes.clj:36` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_97b1b2` | `mutant.uncovered` | `examples/e38_lazy_loaded_tree_view.clj:17` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_aa4317` | `mutant.uncovered` | `examples/e38_lazy_loaded_tree_view.clj:29` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_e5c054` | `mutant.uncovered` | `examples/e38_lazy_loaded_tree_view.clj:35` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_3dcfbe` | `mutant.uncovered` | `examples/e39_web_view_extensions.clj:17` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_c8b402` | `mutant.uncovered` | `examples/e40_dynamic_combo_box.clj:17` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_108c39` | `mutant.uncovered` | `examples/e44_nested_form_view.clj:15` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_8a6e53` | `mutant.uncovered` | `examples/e44_nested_form_view.clj:18` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_2fe2aa` | `mutant.uncovered` | `examples/e44_nested_form_view.clj:22` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_e11a66` | `mutant.uncovered` | `examples/e44_nested_form_view.clj:23` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_4f2b5f` | `mutant.uncovered` | `examples/e44_nested_form_view.clj:50` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_d1a50f` | `mutant.uncovered` | `examples/e44_nested_form_view.clj:54` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_adf298` | `mutant.uncovered` | `examples/e44_nested_form_view.clj:82` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_7ac89c` | `mutant.uncovered` | `examples/e44_nested_form_view.clj:88` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_e1570c` | `mutant.uncovered` | `examples/e45_git_ui.clj:135` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_947691` | `mutant.uncovered` | `examples/e45_git_ui.clj:157` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_f35559` | `mutant.uncovered` | `examples/e45_git_ui.clj:183` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_078358` | `mutant.uncovered` | `examples/e45_git_ui.clj:195` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_d9838e` | `mutant.uncovered` | `examples/e45_git_ui.clj:237` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_def541` | `mutant.uncovered` | `examples/e45_git_ui.clj:243` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_e00bea` | `mutant.uncovered` | `examples/e45_git_ui.clj:266` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_34be7c` | `mutant.uncovered` | `examples/e45_git_ui.clj:267` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_4efef9` | `mutant.uncovered` | `examples/e45_git_ui.clj:272` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_db884a` | `mutant.uncovered` | `examples/e45_git_ui.clj:273` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_afcf3d` | `mutant.uncovered` | `examples/e45_git_ui.clj:277` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_6a21e4` | `mutant.uncovered` | `examples/e45_git_ui.clj:278` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_d186ae` | `mutant.uncovered` | `examples/e45_git_ui.clj:283` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_17aa12` | `mutant.uncovered` | `examples/e45_git_ui.clj:284` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_e8e1fa` | `mutant.uncovered` | `examples/e45_git_ui.clj:288` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_b50df5` | `mutant.uncovered` | `examples/e45_git_ui.clj:294` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_e04810` | `mutant.uncovered` | `examples/e45_git_ui.clj:298` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_c324f4` | `mutant.uncovered` | `examples/e45_git_ui.clj:304` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_59a5a1` | `mutant.uncovered` | `examples/e45_git_ui.clj:305` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_061d29` | `mutant.uncovered` | `examples/e45_git_ui.clj:306` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_860770` | `mutant.uncovered` | `examples/e45_git_ui.clj:324` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_9c0706` | `mutant.uncovered` | `examples/e45_git_ui.clj:334` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_b7b5dd` | `mutant.uncovered` | `examples/e45_git_ui.clj:340` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_7de2ff` | `mutant.uncovered` | `examples/e45_git_ui.clj:341` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_f49780` | `mutant.uncovered` | `examples/e45_git_ui.clj:341` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_91a784` | `mutant.uncovered` | `examples/e45_git_ui.clj:352` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_f2bb56` | `mutant.uncovered` | `examples/e45_git_ui.clj:385` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_7e4012` | `mutant.uncovered` | `examples/e45_git_ui.clj:58` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_960beb` | `mutant.uncovered` | `examples/e45_git_ui.clj:59` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_7e6053` | `mutant.uncovered` | `examples/e45_git_ui.clj:74` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_d85e82` | `mutant.uncovered` | `examples/e45_git_ui.clj:81` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_9aca08` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:10` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_7ab6cd` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:11` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_cfb09f` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:21` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_b159d9` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:22` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_660c55` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:25` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_f8f0f9` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:26` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_2b95a6` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:29` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_0c3875` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:30` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_8a701c` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:52` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_193c09` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:53` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_f614f9` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:58` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_f245e9` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:59` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_3eec89` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:63` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_73dd61` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:64` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_35cfd1` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:8` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_49628c` | `mutant.uncovered` | `examples/e47_check_box_tree.clj:9` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_33c403` | `mutant.uncovered` | `jdk8/cljfx/jdk/platform.clj:7` | true | No test loads the file (absent from the run's lcov; not on the test classpath, or only by an alias the tests do not use). | none |
| `fnd_2b37a9` | `mutant.uncovered` | `src/cljfx/coerce.clj:108` | true | Line 108 has 0 hits in the run's lcov. | test it |
| `fnd_188e94` | `mutant.uncovered` | `src/cljfx/coerce.clj:115` | true | Line 115 has 0 hits in the run's lcov. | test it |
| `fnd_011eca` | `mutant.uncovered` | `src/cljfx/coerce.clj:115` | true | Line 115 has 0 hits in the run's lcov. | test it |
| `fnd_bc920e` | `mutant.uncovered` | `src/cljfx/coerce.clj:116` | true | Line 116 has 0 hits in the run's lcov. | test it |
| `fnd_e198ab` | `mutant.uncovered` | `src/cljfx/coerce.clj:116` | true | Line 116 has 0 hits in the run's lcov. | test it |
| `fnd_27117d` | `mutant.uncovered` | `src/cljfx/coerce.clj:117` | true | Line 117 has 0 hits in the run's lcov. | test it |
| `fnd_b41806` | `mutant.uncovered` | `src/cljfx/coerce.clj:117` | true | Line 117 has 0 hits in the run's lcov. | test it |
| `fnd_58133a` | `mutant.uncovered` | `src/cljfx/coerce.clj:156` | true | Line 156 has 0 hits in the run's lcov. | test it |
| `fnd_7f7335` | `mutant.uncovered` | `src/cljfx/coerce.clj:157` | true | Line 157 has 0 hits in the run's lcov. | test it |
| `fnd_3400de` | `mutant.uncovered` | `src/cljfx/coerce.clj:158` | true | Line 158 has 0 hits in the run's lcov. | test it |
| `fnd_1e66a8` | `mutant.uncovered` | `src/cljfx/coerce.clj:159` | true | Line 159 has 0 hits in the run's lcov. | test it |
| `fnd_13c4c9` | `mutant.uncovered` | `src/cljfx/coerce.clj:198` | true | Line 198 has 0 hits in the run's lcov. | test it |
| `fnd_66e734` | `mutant.uncovered` | `src/cljfx/coerce.clj:199` | true | Line 199 has 0 hits in the run's lcov. | test it |
| `fnd_83c1a8` | `mutant.uncovered` | `src/cljfx/coerce.clj:200` | true | Line 200 has 0 hits in the run's lcov. | test it |
| `fnd_2b6a73` | `mutant.uncovered` | `src/cljfx/coerce.clj:201` | true | Line 201 has 0 hits in the run's lcov. | test it |
| `fnd_af05ee` | `mutant.uncovered` | `src/cljfx/coerce.clj:214` | true | Line 214 has 0 hits in the run's lcov. | test it |
| `fnd_11a517` | `mutant.uncovered` | `src/cljfx/coerce.clj:219` | true | Line 219 has 0 hits in the run's lcov. | test it |
| `fnd_e2ff1a` | `mutant.uncovered` | `src/cljfx/coerce.clj:222` | true | Line 222 has 0 hits in the run's lcov. | test it |
| `fnd_8b4c0a` | `mutant.uncovered` | `src/cljfx/coerce.clj:225` | true | Line 225 has 0 hits in the run's lcov. | test it |
| `fnd_5bdd15` | `mutant.uncovered` | `src/cljfx/coerce.clj:228` | true | Line 228 has 0 hits in the run's lcov. | test it |
| `fnd_94f253` | `mutant.uncovered` | `src/cljfx/coerce.clj:238` | true | Line 238 has 0 hits in the run's lcov. | test it |
| `fnd_06a5fa` | `mutant.uncovered` | `src/cljfx/coerce.clj:246` | true | Line 246 has 0 hits in the run's lcov. | test it |
| `fnd_669cc1` | `mutant.uncovered` | `src/cljfx/coerce.clj:247` | true | Line 247 has 0 hits in the run's lcov. | test it |
| `fnd_32fa24` | `mutant.uncovered` | `src/cljfx/coerce.clj:248` | true | Line 248 has 0 hits in the run's lcov. | test it |
| `fnd_e27d9e` | `mutant.uncovered` | `src/cljfx/coerce.clj:249` | true | Line 249 has 0 hits in the run's lcov. | test it |
| `fnd_4825eb` | `mutant.uncovered` | `src/cljfx/coerce.clj:256` | true | Line 256 has 0 hits in the run's lcov. | test it |
| `fnd_21e715` | `mutant.uncovered` | `src/cljfx/coerce.clj:258` | true | Line 258 has 0 hits in the run's lcov. | test it |
| `fnd_f9b04a` | `mutant.uncovered` | `src/cljfx/coerce.clj:259` | true | Line 259 has 0 hits in the run's lcov. | test it |
| `fnd_b99ac7` | `mutant.uncovered` | `src/cljfx/coerce.clj:260` | true | Line 260 has 0 hits in the run's lcov. | test it |
| `fnd_c598df` | `mutant.uncovered` | `src/cljfx/coerce.clj:261` | true | Line 261 has 0 hits in the run's lcov. | test it |
| `fnd_5d28a9` | `mutant.uncovered` | `src/cljfx/coerce.clj:281` | true | Line 281 has 0 hits in the run's lcov. | test it |
| `fnd_a8a4a1` | `mutant.uncovered` | `src/cljfx/coerce.clj:282` | true | Line 282 has 0 hits in the run's lcov. | test it |
| `fnd_3207f2` | `mutant.uncovered` | `src/cljfx/coerce.clj:294` | true | Line 294 has 0 hits in the run's lcov. | test it |
| `fnd_d2c2dd` | `mutant.uncovered` | `src/cljfx/coerce.clj:295` | true | Line 295 has 0 hits in the run's lcov. | test it |
| `fnd_c1f269` | `mutant.uncovered` | `src/cljfx/coerce.clj:334` | true | Line 334 has 0 hits in the run's lcov. | test it |
| `fnd_3cb0b6` | `mutant.uncovered` | `src/cljfx/coerce.clj:335` | true | Line 335 has 0 hits in the run's lcov. | test it |
| `fnd_348c04` | `mutant.uncovered` | `src/cljfx/coerce.clj:336` | true | Line 336 has 0 hits in the run's lcov. | test it |
| `fnd_e56503` | `mutant.uncovered` | `src/cljfx/coerce.clj:337` | true | Line 337 has 0 hits in the run's lcov. | test it |
| `fnd_308a82` | `mutant.uncovered` | `src/cljfx/coerce.clj:342` | true | Line 342 has 0 hits in the run's lcov. | test it |
| `fnd_f9e43a` | `mutant.uncovered` | `src/cljfx/coerce.clj:349` | true | Line 349 has 0 hits in the run's lcov. | test it |
| `fnd_9a24f2` | `mutant.uncovered` | `src/cljfx/coerce.clj:350` | true | Line 350 has 0 hits in the run's lcov. | test it |
| `fnd_8ed179` | `mutant.uncovered` | `src/cljfx/coerce.clj:351` | true | Line 351 has 0 hits in the run's lcov. | test it |
| `fnd_80e005` | `mutant.uncovered` | `src/cljfx/coerce.clj:352` | true | Line 352 has 0 hits in the run's lcov. | test it |
| `fnd_66af3f` | `mutant.uncovered` | `src/cljfx/coerce.clj:363` | true | Line 363 has 0 hits in the run's lcov. | test it |
| `fnd_9040d0` | `mutant.uncovered` | `src/cljfx/coerce.clj:415` | true | Line 415 has 0 hits in the run's lcov. | test it |
| `fnd_5a35bb` | `mutant.uncovered` | `src/cljfx/coerce.clj:90` | true | Line 90 has 0 hits in the run's lcov. | test it |
| `fnd_5830c0` | `mutant.uncovered` | `src/cljfx/coerce.clj:91` | true | Line 91 has 0 hits in the run's lcov. | test it |
| `fnd_bd68c7` | `mutant.uncovered` | `src/cljfx/coerce.clj:96` | true | Line 96 has 0 hits in the run's lcov. | test it |
| `fnd_f58b6b` | `mutant.uncovered` | `src/cljfx/coerce.clj:97` | true | Line 97 has 0 hits in the run's lcov. | test it |
| `fnd_401535` | `mutant.uncovered` | `src/cljfx/context.clj:173` | true | Line 173 has 0 hits in the run's lcov. | test it |
| `fnd_4157e3` | `mutant.uncovered` | `src/cljfx/ext/cell_factory.clj:19` | true | Line 19 has 0 hits in the run's lcov. | test it |
| `fnd_d2f9a4` | `mutant.uncovered` | `src/cljfx/ext/cell_factory.clj:19` | true | Line 19 has 0 hits in the run's lcov. | test it |
| `fnd_1dea2d` | `mutant.uncovered` | `src/cljfx/ext/cell_factory.clj:31` | true | Line 31 has 0 hits in the run's lcov. | test it |
| `fnd_45a124` | `mutant.uncovered` | `src/cljfx/ext/web_view.clj:12` | true | Line 12 has 0 hits in the run's lcov. | test it |
| `fnd_832114` | `mutant.uncovered` | `src/cljfx/fx/displacement_map.clj:11` | true | Line 11 has 0 hits in the run's lcov. | test it |
| `fnd_999c13` | `mutant.uncovered` | `src/cljfx/fx/displacement_map.clj:11` | true | Line 11 has 0 hits in the run's lcov. | test it |
| `fnd_b997a4` | `mutant.uncovered` | `src/cljfx/fx/scroll_pane.clj:18` | true | Line 18 has 0 hits in the run's lcov. | test it |
| `fnd_a415bb` | `mutant.uncovered` | `src/cljfx/fx/scroll_pane.clj:18` | true | Line 18 has 0 hits in the run's lcov. | test it |
| `fnd_371695` | `mutant.uncovered` | `src/cljfx/fx/scroll_pane.clj:21` | true | Line 21 has 0 hits in the run's lcov. | test it |
| `fnd_2484b6` | `mutant.uncovered` | `src/cljfx/fx/scroll_pane.clj:21` | true | Line 21 has 0 hits in the run's lcov. | test it |
| `fnd_853320` | `mutant.uncovered` | `src/cljfx/fx/scroll_pane.clj:25` | true | Line 25 has 0 hits in the run's lcov. | test it |
| `fnd_133a44` | `mutant.uncovered` | `src/cljfx/fx/scroll_pane.clj:25` | true | Line 25 has 0 hits in the run's lcov. | test it |
| `fnd_71181b` | `mutant.uncovered` | `src/cljfx/fx/table_view.clj:19` | true | Line 19 has 0 hits in the run's lcov. | test it |
| `fnd_e71e9a` | `mutant.uncovered` | `src/cljfx/fx/table_view.clj:20` | true | Line 20 has 0 hits in the run's lcov. | test it |
| `fnd_28f6c8` | `mutant.uncovered` | `src/cljfx/fx/table_view.clj:29` | true | Line 29 has 0 hits in the run's lcov. | test it |
| `fnd_bf211d` | `mutant.uncovered` | `src/cljfx/fx/transition.clj:14` | true | Line 14 has 0 hits in the run's lcov. | test it |
| `fnd_36b67b` | `mutant.uncovered` | `src/cljfx/fx/tree_table_view.clj:19` | true | Line 19 has 0 hits in the run's lcov. | test it |
| `fnd_f2333c` | `mutant.uncovered` | `src/cljfx/fx/tree_table_view.clj:20` | true | Line 20 has 0 hits in the run's lcov. | test it |
| `fnd_28f6c8` | `mutant.uncovered` | `src/cljfx/fx/tree_table_view.clj:26` | true | Line 26 has 0 hits in the run's lcov. | test it |
| `fnd_d32177` | `mutant.uncovered` | `src/cljfx/fx/triangle_mesh.clj:13` | true | Line 13 has 0 hits in the run's lcov. | test it |
| `fnd_061540` | `mutant.uncovered` | `src/cljfx/fx/triangle_mesh.clj:14` | true | Line 14 has 0 hits in the run's lcov. | test it |
| `fnd_9a4e6c` | `mutant.uncovered` | `src/cljfx/lifecycle.clj:255` | true | Line 255 has 0 hits in the run's lcov. | test it |
| `fnd_1db85a` | `mutant.uncovered` | `src/cljfx/lifecycle.clj:255` | true | Line 255 has 0 hits in the run's lcov. | test it |
| `fnd_4af96c` | `mutant.uncovered` | `src/cljfx/lifecycle.clj:256` | true | Line 256 has 0 hits in the run's lcov. | test it |
| `fnd_6e06fd` | `mutant.uncovered` | `src/cljfx/lifecycle.clj:301` | true | Line 301 has 0 hits in the run's lcov. | test it |
| `fnd_a6172b` | `mutant.uncovered` | `src/cljfx/lifecycle.clj:301` | true | Line 301 has 0 hits in the run's lcov. | test it |
| `fnd_12f2ea` | `mutant.uncovered` | `src/cljfx/lifecycle.clj:302` | true | Line 302 has 0 hits in the run's lcov. | test it |
| `fnd_500cbd` | `mutant.uncovered` | `src/cljfx/lifecycle.clj:679` | true | Line 679 has 0 hits in the run's lcov. | test it |
| `fnd_25d41c` | `mutant.uncovered` | `src/cljfx/lifecycle.clj:754` | true | Line 754 has 0 hits in the run's lcov. | test it |
| `fnd_aac847` | `mutant.uncovered` | `src/cljfx/mutator.clj:67` | true | Line 67 has 0 hits in the run's lcov. | test it |
| `fnd_33146a` | `mutant.uncovered` | `src/cljfx/mutator.clj:80` | true | Line 80 has 0 hits in the run's lcov. | test it |
| `fnd_b7b36f` | `mutant.uncovered` | `src/cljfx/mutator.clj:90` | true | Line 90 has 0 hits in the run's lcov. | test it |

**`tests.quality`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_c55697` | `test.logic` | `test/cljfx/lifecycle_test.clj:672` | false | Survey 1 only: its branches guard testit assertions, which were not read (fixed in 3003be2). | fixed |
| `fnd_a967ef` | `test.logic` | `test/cljfx/lifecycle_test.clj:744` | false | Survey 1 only: its branches guard testit assertions, which were not read (fixed in 3003be2). | fixed |
| `fnd_f51381` | `test.no-assertion` | `test/cljfx/composite_test.clj:11` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_f0de37` | `test.no-assertion` | `test/cljfx/composite_test.clj:112` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_dd78d9` | `test.no-assertion` | `test/cljfx/context_test.clj:12` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_5b1f1e` | `test.no-assertion` | `test/cljfx/context_test.clj:124` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_64a315` | `test.no-assertion` | `test/cljfx/context_test.clj:138` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_f34de9` | `test.no-assertion` | `test/cljfx/context_test.clj:156` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_6901e2` | `test.no-assertion` | `test/cljfx/context_test.clj:170` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_66fd2f` | `test.no-assertion` | `test/cljfx/context_test.clj:189` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_071725` | `test.no-assertion` | `test/cljfx/context_test.clj:210` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_8f993a` | `test.no-assertion` | `test/cljfx/context_test.clj:31` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_bedbd0` | `test.no-assertion` | `test/cljfx/context_test.clj:57` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_134848` | `test.no-assertion` | `test/cljfx/context_test.clj:6` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_6856c8` | `test.no-assertion` | `test/cljfx/context_test.clj:89` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_58686b` | `test.no-assertion` | `test/cljfx/exception_test.clj:6` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_eb6ed3` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:14` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_8fa121` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:140` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_a48970` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:178` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_ef3954` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:215` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_2eb488` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:311` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_57c84c` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:39` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_6ac754` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:391` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_0210d6` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:429` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_163f63` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:497` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_b06eb2` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:603` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_38c55a` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:67` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_c61163` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:672` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_09d151` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:714` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_037698` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:744` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_af11c1` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:778` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_3364b3` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:812` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_9023ac` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:865` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_9189d5` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:892` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_2ee851` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:917` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_4e3f13` | `test.no-assertion` | `test/cljfx/lifecycle_test.clj:95` | false | Survey 1 only: its testit `fact`/`facts` arrows were not read as assertions (fixed in 3003be2). | fixed |
| `fnd_bb9d36` | `test.logic` | `test/cljfx/event_handlers_test.clj:73` | true | Read by hand: a try/catch, a loop, or branches in the test. | simplify |
| `fnd_609332` | `test.logic` | `test/cljfx/watcher_test.clj:10` | true | Read by hand: a try/catch, a loop, or branches in the test. | simplify |
| `fnd_28d43d` | `test.logic` | `test/cljfx/watcher_test.clj:219` | true | Read by hand: a try/catch, a loop, or branches in the test. | simplify |
| `fnd_21e0b5` | `test.weak-assertion` | `test/cljfx/event_handlers_test.clj:73` | true | Read by hand: its only assertion is a truthiness (`(is false …)` in a catch, `(is (some? …))`). | pin the value |
| `fnd_12c422` | `test.weak-assertion` | `test/cljfx/local_state_test.clj:8` | true | Read by hand: its only assertion is a truthiness (`(is false …)` in a catch, `(is (some? …))`). | pin the value |


## False-positive rate per check

`false / (true + false)`, disputed rows apart.

| Check | True | False | Disputed | Rate |
|---|---|---|---|---|
| `coverage` | 2 | 0 | 0 | 0.00 |
| `static.clones` | 12 | 0 | 0 | 0.00 |
| `static.complexity` | 18 | 0 | 0 | 0.00 |
| `static.crap` | 84 | 0 | 0 | 0.00 |
| `static.dead-code` | 0 | 109 | 1 | 1.00 |
| `static.deps` | 0 | 0 | 1 | — |
| `tests.assertion-density` | 6 | 2 | 0 | 0.25 |
| `tests.coverage` | 107 | 0 | 0 | 0.00 |
| `tests.mutation` | 332 | 1 | 1 | 0.00 |
| `tests.quality` | 5 | 36 | 0 | 0.88 |

Every other check that ran (`tests.flaky`) reported nothing: no rate. The `coverage` row is the producers' own diagnostic (`coverage.run-failed`). The false rows of `tests.quality`, `tests.assertion-density`, and 24 of `static.dead-code` are from the first survey and fixed; `static.dead-code`'s remaining 85 false rows (examples and the build script) and `tests.mutation`'s one are open items.

## Check bugs fixed

- **metosin/testit's `fact` and `facts` assert** (`tests.quality`, `tests.assertion-density`): cljfx asserts through testit (`(fact (grab-history) => [...])`, `(facts a => b c => d)`), whose arrows are each an `is` of clojure.test, and 34 of its 54 tests read as asserting nothing. Clojure's `tests.scm` captures each arrow inside a `fact` or `facts` form as an assertion, classed by the arrow: `=>` and `=eventually=>` equality (strong), `=not=>`, `=in=>`, `=eventually-in=>` equality (weak), `=throws=>` throws (strong). Fixture: `crates/cupel-fixtures/repos/testit-facts/clojure/`; tests `clojure_tests::testit_facts_assert_with_each_arrow` and `crates/cupel-fixtures/tests/testit_facts.rs`. Commit 3003be2. It removed 34 `test.no-assertion`, 2 `density.sparse-file`, and 2 `test.logic` (branches that guard testit assertions, ADR-069), and added 1 `density.sparse-test` (true).
- **A symbol qualified by a project namespace is an edge to it** (`static.dead-code`, `static.deps`): cljfx loads each of its 197 `cljfx.fx.*` namespaces on first use, `(lazy-load cljfx.fx.combo-box/lifecycle)` expanding to `(requiring-resolve 'cljfx.fx.combo-box/lifecycle)`, so nothing `:require`s them, and the module pass reported 8 exports and 16 private helpers of them unreachable. Clojure's `imports.scm` captures a symbol qualified by a dotted namespace where it is read as code (a list, vector, map, set, `#(…)`, `@…`, or `#'…` holds it) or quoted as the argument of `requiring-resolve` or `resolve`; the resolver keeps it as an edge only when the namespace is another file of the project and the file has no edge to it yet (an alias such as `fx.combo-box-base/props`, a library's namespace, and the file's own make none). A quoted symbol elsewhere is data: the first version of the fix made cljfx's `'cljfx.api/ext-watcher` labels edges, and a 218-module cycle (fixed before the final survey). Fixture: `crates/cupel-fixtures/repos/lazy-load/clojure/`; tests `clojure_modules::a_qualified_symbol_naming_a_namespace_of_the_project_depends_on_it` and `crates/cupel-fixtures/tests/lazy_load.rs`. Commits f6b9d3c and 32c7687. It removed 8 `dead.export` and 16 `dead.symbol` and added 1 `dep.cycle` (disputed: the lazy load is what breaks it).

Rerun after the fixes: the removed rows are gone and every other finding of survey 3 (the first with coverage) is still reported, by check, kind, file, and line, with the one cycle added.

Not fixed, counted false above (open items):

- **An example or script namespace's load-time forms are its entry points** (`static.dead-code`, 81 rows): cljfx's examples are loaded at a REPL, and their top-level forms (`(fx/mount-renderer *state renderer)`, `(def renderer (fx/create-renderer … root …))`, `(fx/on-fx-thread (fx/create-component …))`) use their functions; ADR-138 §9 (group 2, Python) and §10 (ECMAScript, this group) root what module-level code calls. In Clojure every namespace has load-time forms (`def`s of values), so rooting what they reference would root every orphan namespace and end the module pass's use; telling a script namespace from an orphan library one needs a convention (a directory, `-main`, an alias) and is a design decision.
- **`clj -T:build` calls `build.clj`'s functions by name** (`static.dead-code`, 4 rows): `deps.edn`'s `:build {:ns-default build}` names the namespace whose public functions the CLI invokes; reading tool aliases for entry points is the same kind of convention.
- **Cloverage instruments only the roots its command names** (`tests.mutation`, 1 row): the command `cupel init` writes has `--src-ns-path src`, so `jdk11/cljfx/jdk/platform.clj`, on `deps.edn`'s `:paths` and run by every test session, is absent from the map and its mutant is `uncovered`; ADR-138 §8 reads an absent file as one no test loads, which holds for V8 and coverage.py but not for Cloverage. `init` could name every `:paths` entry.
- **Cupel's Clojure runner cannot run a project whose `:test` alias has `:main-opts`** (selection; by inspection, not run): the Clojure CLI appends the arguments after `-M:test` to the alias's `:main-opts`, so Cupel's script and report path would reach `-m cognitect.test-runner` instead of `clojure.main`; that is how most permissively licensed `deps.edn` libraries configure their tests.
- **Cloverage's run is OS-sensitive for a JavaFX library**: the coverage command had to be set by hand twice (above); `init` cannot know a project's headless alias.

## What a maintainer would do first

The coercions in `src/cljfx/coerce.clj` (`font`, `border-widths`, `duration`, and the rest: CRAP up to 240, no test runs them) are the risk: a table test per coercion, input to JavaFX value, would cover most of the 82 uncovered mutants in `src/`. Then the ten surviving mutants in `cljfx.composite`, `cljfx.context`, `cljfx.lifecycle`, `cljfx.prop`, and `cljfx.renderer` (an `and` that can be `or` in `prop.clj:30` and `renderer.clj:55`, a `<` that can be `<=` in `composite.clj:128`), and `reset-test`'s timing dependence. The `examples/` dead-code rows and the clones among the examples need no action.
