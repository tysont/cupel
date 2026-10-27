# External dogfood: gvergnaud/ts-pattern

Group 3 of P9.9 ([ADR-138](../adr/CUPEL_DECISION_138.md)): the TypeScript repository. Cupel's release build (`target/release/cupel`, ADR-138 §2) of branch `p9-dogfood-ts-clojure`: the first survey at `7f696f6`, the second survey and the deep check at `6a582a5`, the final survey at `e12d282`; every run went through `scripts/heavy.sh`.

## Repository

- **URL:** https://github.com/gvergnaud/ts-pattern, commit `f11be73` (2026-10-14), default branch `main`, version 5.9.2.
- **Lines:** 19,373 lines of TypeScript in 85 files: 4,172 in `src/` (20 files, 13 of them the type-level `src/types/`), 13,708 in `tests/` (52), 1,135 in `benchmarks/` (8), 358 in `examples/` (5); plus `scripts/completions/run.mjs` and `jest.config.cjs` (JavaScript).
- **License:** MIT.
- **Suite:** `npx jest` (the documented `npm test`; jest 30 with the `ts-jest` preset, which type-checks every test file) passes in 19.1 s cold (21.9 s wall): 467 tests in 50 files, Node 22.23.3 from `make dev-setup`. Dependencies came from `npm ci --ignore-scripts --no-audit --no-fund --prefix <clone>` (the lockfile's 655 packages, no install script run). Selection: the most depended-on pattern-matching library for TypeScript, a jest suite with no services; `remeda/remeda` was ruled out by its `.nvmrc` (Node 26, above the pin), `immerjs/immer` and `jquense/yup` by `yarn.lock` (yarn is not in the dev environment).

## Commands

From Cupel's worktree, with `C=target/release/cupel` and `R=<scratchpad>/p9-group3/repos/ts-pattern`, each line one `scripts/heavy.sh` invocation:

```
$C init --write --cwd $R                                                        # kept, one key added (below)
$C survey --depth standard --budget 30m --no-install --format json --cwd $R    # 2347 s, exit 5 (partial: the budget ended tests.mutation), at 7f696f6
$C survey --depth standard --budget 30m --no-install --format json --cwd $R    # 2263 s, exit 0, at 6a582a5
$C survey --depth standard --budget 30m --no-install --format json --cwd $R    # 51 s, exit 0, at e12d282 (mutant outcomes reused)
$C check --depth deep --no-install --format json --cwd $R \
   --paths tests/branded-nominal-types.test.ts tests/types.test.ts \
           tests/exhaustive-fallback.test.ts tests/exhaustive-match.test.ts      # 11 s, exit 0, at 6a582a5
```

`cupel init` wrote `[lang.typescript] test = "npx jest"` and `coverage = "npx jest --coverage --coverageReporters=lcov"`. Set by hand: `mutation = { jobs = 2 }`, the brief's two slots (ADR-148 bounds their memory). The first survey ran out of its budget inside `tests.mutation` (1,755.7 s, `partial`, no finding: a partial production reports none of its mutants) and left `static.analyze` out (`skipped`, reason `budget`); the second ran every check, `tests.mutation` in 1,241.1 s over 280 mutants, whose outcomes the third reused (jest has no per-test map, ADR-147 §5, so each runs the whole suite; nothing was reused, since the first survey stored no outcome). The survey's ten targets are all test functions in four test files (the ranking defect group 1 recorded: a test function has no mutation score or coverage), so the deep check, run on those files as the method says, mutates nothing and reports a subset of the survey's findings. The final survey envelope (0.9 MB) stays in the agent's scratchpad; the deep check's (97 KB) is [`gvergnaud-ts-pattern/check-deep.json`](gvergnaud-ts-pattern/check-deep.json).

## Survey

Whole tree at `e12d282`, status `pass`, exit 0, 389 findings (21 fail, 198 warn, 170 info), scope 88 files and 741 symbols, 51 s. The rankings, verbatim (`rank. target value (location)`):

- `static.clones.cluster_size` per group, highest worst, 46 measured: 1. `tests.numbers.Numbers.chainable.f and 14 others` 15 (tests/numbers.test.ts:61); 2. `tests.bigints.BigInts.f and 6 others` 7 (tests/bigints.test.ts:6); 3. `benchmarks.always-last-digit.testExhaustive and 3 others` 4 (benchmarks/always-last-digit.ts:6); 4. `tests.nesting.Nesting.objects.it should work on 2 level and 3 others` 4 (tests/nesting.test.ts:56); 5. `tests.variadic-tuples.variadic tuples ([a, ...b[]]).runtime.match.[any ...any] pattern should match any non-empty array and 3 others` 4 (tests/variadic-tuples.test.ts:7); 6. `tests.chainable.chainable methods.string compositions.P.string.startsWith(..).endsWith(..) and 2 others` 3 (tests/chainable.test.ts:82); 7. `tests.chainable.chainable methods.string compositions.f and 2 others` 3 (tests/chainable.test.ts:7); 8. `tests.chainable.chainable methods.string compositions.f and 2 others` 3 (tests/chainable.test.ts:24); 9. `tests.nesting.Nesting.array.it should work on 2 levels and 2 others` 3 (tests/nesting.test.ts:124); 10. `tests.output-type.output type.exhaustive.should return a single type if they are all compatible and 2 others` 3 (tests/output-type.test.ts:7)
- `static.complexity.cyclomatic` per symbol, highest worst, 741 measured: 1. `benchmarks.nested-objects.testIfElse` 28 (benchmarks/nested-objects.ts:12); 2. `internals.helpers.matchPattern` 23 (src/internals/helpers.ts:32); 3. `patterns.record` 12 (src/patterns.ts:445); 4. `benchmarks.always-last-digit.testIfElse` 11 (benchmarks/always-last-digit.ts:36); 5. `benchmarks.always-last-digit.testTernary` 11 (benchmarks/always-last-digit.ts:89); 6. `benchmarks.random-digit.testIfElse` 11 (benchmarks/random-digit.ts:36); 7. `benchmarks.random-digit.testTernary` 11 (benchmarks/random-digit.ts:89); 8. `match.MatchExpression.with` 10 (src/match.ts:50); 9. `scripts.completions.run.tsgo` 10 (scripts/completions/run.mjs:169); 10. `patterns.map` 7 (src/patterns.ts:362)
- `static.complexity.cognitive` per symbol, highest worst, 741 measured: 1. `internals.helpers.matchPattern` 62 (src/internals/helpers.ts:32); 2. `benchmarks.always-last-digit.testTernary` 55 (benchmarks/always-last-digit.ts:89); 3. `benchmarks.random-digit.testTernary` 55 (benchmarks/random-digit.ts:89); 4. `benchmarks.nested-objects.testIfElse` 30 (benchmarks/nested-objects.ts:12); 5. `benchmarks.always-last-digit.testIfElse` 20 (benchmarks/always-last-digit.ts:36); 6. `benchmarks.random-digit.testIfElse` 20 (benchmarks/random-digit.ts:36); 7. `scripts.completions.run.tsgo` 18 (scripts/completions/run.mjs:169); 8. `patterns.record` 13 (src/patterns.ts:445); 9. `benchmarks.always-last-digit.testSwitch` 11 (benchmarks/always-last-digit.ts:62); 10. `benchmarks.random-digit.testSwitch` 11 (benchmarks/random-digit.ts:62)
- `static.crap.crap` per symbol, highest worst, 81 measured: 1. `internals.helpers.matchPattern` 23.0104 (src/internals/helpers.ts:32); 2. `patterns.record` 12.0247 (src/patterns.ts:445); 3. `match.MatchExpression.with` 10 (src/match.ts:50); 4. `patterns.map` 7.53772 (src/patterns.ts:362); 5. `patterns.map.selector` 6 (src/patterns.ts:381); 6. `patterns.select` 6 (src/patterns.ts:696); 7. `patterns.set.selector` 6 (src/patterns.ts:317); 8. `patterns.set` 5.24599 (src/patterns.ts:299); 9. `patterns.array` 5 (src/patterns.ts:246); 10. `internals.helpers.getSelectionKeys` 4 (src/internals/helpers.ts:120)
- `static.deps.distance` per module, highest worst, 18 measured: 1. `benchmarks/compile-time/shared.ts` 1 (benchmarks/compile-time/shared.ts:1); 2. `src/errors.ts` 1 (src/errors.ts:1); 3. `src/internals/symbols.ts` 1 (src/internals/symbols.ts:1); 4. `tests/types-catalog/definition.ts` 1 (tests/types-catalog/definition.ts:1); 5. `src/types/helpers.ts` 0.981 (src/types/helpers.ts:1); 6. `src/types/IsMatching.ts` 0.75 (src/types/IsMatching.ts:1); 7. `src/types/BuildMany.ts` 0.667 (src/types/BuildMany.ts:1); 8. `src/types/ExtractPreciseValue.ts` 0.667 (src/types/ExtractPreciseValue.ts:1); 9. `src/types/PrimitivePattern.ts` 0.667 (src/types/PrimitivePattern.ts:1); 10. `src/types/Pattern.ts` 0.628 (src/types/Pattern.ts:1)
- `static.deps.coupling` per module, highest worst, 88 measured: 1. `src/types/helpers.ts` 58 (src/types/helpers.ts:1); 2. `src/index.ts` 52 (src/index.ts:1); 3. `src/types/Pattern.ts` 22 (src/types/Pattern.ts:1); 4. `tests/types-catalog/utils.ts` 19 (tests/types-catalog/utils.ts:1); 5. `src/patterns.ts` 10 (src/patterns.ts:1); 6. `src/types/FindSelected.ts` 8 (src/types/FindSelected.ts:1); 7. `src/types/InvertPattern.ts` 8 (src/types/InvertPattern.ts:1); 8. `src/internals/symbols.ts` 7 (src/internals/symbols.ts:1); 9. `src/types/Match.ts` 7 (src/types/Match.ts:1); 10. `src/internals/helpers.ts` 6 (src/internals/helpers.ts:1)
- `static.deps.cycle_size` per group, highest worst, 1 measured: 1. `src/internals/helpers.ts → src/types/Pattern.ts → src/patterns.ts → src/internals/helpers.ts` 9 (src/internals/helpers.ts:9)
- `tests.coverage.uncovered_lines` per symbol, highest worst, 81 measured: 1. `patterns.map` 4 (src/patterns.ts:362); 2. `patterns.set` 3 (src/patterns.ts:299); 3. `errors.NonExhaustiveError.constructor` 1 (src/errors.ts:6); 4. `internals.helpers.matchPattern` 1 (src/internals/helpers.ts:32); 5. `is-matching.isMatching` 1 (src/is-matching.ts:53); 6. `match.MatchExpression.narrow` 1 (src/match.ts:128); 7. `patterns.arrayChainable` 1 (src/patterns.ts:165); 8. `patterns.map.selector` 1 (src/patterns.ts:381); 9. `patterns.mapEvery` 1 (src/patterns.ts:410); 10. `patterns.not` 1 (src/patterns.ts:611)
- `tests.coverage.line_coverage` per symbol, lowest worst, 81 measured: 1. `match.MatchExpression.narrow` 0 (src/match.ts:128); 2. `patterns.map.selector` 0 (src/patterns.ts:381); 3. `patterns.set.selector` 0 (src/patterns.ts:317); 4. `patterns.arrayChainable` 0.666667 (src/patterns.ts:165); 5. `errors.NonExhaustiveError.constructor` 0.75 (src/errors.ts:6); 6. `patterns.mapEvery` 0.75 (src/patterns.ts:410); 7. `patterns.not` 0.75 (src/patterns.ts:611); 8. `patterns.map` 0.777778 (src/patterns.ts:362); 9. `patterns.set` 0.785714 (src/patterns.ts:299); 10. `is-matching.isMatching` 0.875 (src/is-matching.ts:53)
- `tests.mutation.survivors` per symbol, highest worst, 72 measured: 1. `match.MatchExpression.with` 4 (src/match.ts:50); 2. `patterns.map` 3 (src/patterns.ts:362); 3. `patterns.negative` 2 (src/patterns.ts:1047); 4. `patterns.negativeBigInt` 2 (src/patterns.ts:1174); 5. `patterns.positive` 2 (src/patterns.ts:1035); 6. `patterns.positiveBigInt` 2 (src/patterns.ts:1162); 7. `patterns.record` 2 (src/patterns.ts:445); 8. `patterns.chainable` 1 (src/patterns.ts:136); 9. `patterns.isNonNullable` 1 (src/patterns.ts:762); 10. `patterns.lt` 1 (src/patterns.ts:955)
- `tests.mutation.mutation_score` per symbol, lowest worst, 72 measured: 1. `benchmarks.always-last-digit.testExhaustive` 0 (benchmarks/always-last-digit.ts:6); 2. `benchmarks.always-last-digit.testIfElse` 0 (benchmarks/always-last-digit.ts:36); 3. `benchmarks.always-last-digit.testOtherwise` 0 (benchmarks/always-last-digit.ts:21); 4. `benchmarks.always-last-digit.testSwitch` 0 (benchmarks/always-last-digit.ts:62); 5. `benchmarks.always-last-digit.testTernary` 0 (benchmarks/always-last-digit.ts:89); 6. `benchmarks.compile-time.discriminated-union-exhaustive.redact` 0 (benchmarks/compile-time/discriminated-union-exhaustive.ts:9); 7. `benchmarks.compile-time.discriminated-union-non-exhaustive.redact` 0 (benchmarks/compile-time/discriminated-union-non-exhaustive.ts:9); 8. `benchmarks.compile-time.large-union-exhaustive.handle` 0 (benchmarks/compile-time/large-union-exhaustive.ts:8); 9. `benchmarks.compile-time.large-union-non-exhaustive.handle` 0 (benchmarks/compile-time/large-union-non-exhaustive.ts:9); 10. `benchmarks.nested-objects.rand` 0 (benchmarks/nested-objects.ts:54)
- `tests.mutation.tests` per symbol, lowest worst, 72 measured: 1. `benchmarks.always-last-digit.testExhaustive` 0 (benchmarks/always-last-digit.ts:6); 2. `benchmarks.always-last-digit.testIfElse` 0 (benchmarks/always-last-digit.ts:36); 3. `benchmarks.always-last-digit.testOtherwise` 0 (benchmarks/always-last-digit.ts:21); 4. `benchmarks.always-last-digit.testSwitch` 0 (benchmarks/always-last-digit.ts:62); 5. `benchmarks.always-last-digit.testTernary` 0 (benchmarks/always-last-digit.ts:89); 6. `benchmarks.compile-time.discriminated-union-exhaustive.redact` 0 (benchmarks/compile-time/discriminated-union-exhaustive.ts:9); 7. `benchmarks.compile-time.discriminated-union-non-exhaustive.redact` 0 (benchmarks/compile-time/discriminated-union-non-exhaustive.ts:9); 8. `benchmarks.compile-time.large-union-exhaustive.handle` 0 (benchmarks/compile-time/large-union-exhaustive.ts:8); 9. `benchmarks.compile-time.large-union-non-exhaustive.handle` 0 (benchmarks/compile-time/large-union-non-exhaustive.ts:9); 10. `benchmarks.nested-objects.rand` 0 (benchmarks/nested-objects.ts:54)
- `tests.quality.mock_density` per symbol, highest worst, 406 measured: 1. `tests.branded-nominal-types.Branded strings.issue #167` 0 (tests/branded-nominal-types.test.ts:27); 2. `tests.branded-nominal-types.Branded strings.issue #178` 0 (tests/branded-nominal-types.test.ts:40); 3. `tests.branded-nominal-types.Branded strings.should treat branded strings as default string, and not as objects` 0 (tests/branded-nominal-types.test.ts:11); 4. `tests.build-many.BuildMany.should correctly update the content of a readonly tuple` 0 (tests/build-many.test.ts:6); 5. `tests.chainable.chainable methods.and.should infer the intersection of narrowed patterns` 0 (tests/chainable.test.ts:217); 6. `tests.chainable.chainable methods.or.should infer the union of narrowed patterns` 0 (tests/chainable.test.ts:239); 7. `tests.chainable.chainable methods.string compositions.P.string.startsWith(..).endsWith(..)` 0 (tests/chainable.test.ts:82); 8. `tests.deep-exclude.DeepExclude.Excluding nested unions.should correctly exclude` 0 (tests/deep-exclude.test.ts:488); 9. `tests.deep-exclude.DeepExclude.List.should work with empty list patterns` 0 (tests/deep-exclude.test.ts:283); 10. `tests.deep-exclude.DeepExclude.Literals` 0 (tests/deep-exclude.test.ts:30)
- Targets, protect first (10 recommended, top 10): 1. `tests.branded-nominal-types.Branded strings.issue #167` both (mutation score None, coverage None; tests/branded-nominal-types.test.ts); 2. `tests.types.type narrowing inheritence.on a discriminated union type, once a case is handled it should be excluded from the input type.shouldn't exclude in case of primitive type` protect (mutation score None, coverage None; tests/types.test.ts); 3. `tests.exhaustive-fallback.Exhaustive fallback function.should return a value assignable .returnType<T>()` protect (mutation score None, coverage None; tests/exhaustive-fallback.test.ts); 4. `tests.exhaustive-fallback.Exhaustive fallback function.should return a value assignable to the explicit output type` protect (mutation score None, coverage None; tests/exhaustive-fallback.test.ts); 5. `tests.exhaustive-fallback.Exhaustive fallback function.should throw otherwise` protect (mutation score None, coverage None; tests/exhaustive-fallback.test.ts); 6. `tests.exhaustive-match.exhaustive().Exhaustive match and `not` patterns.should work with a single not pattern` protect (mutation score None, coverage None; tests/exhaustive-match.test.ts); 7. `tests.exhaustive-match.exhaustive().Exhaustive match and `not` patterns.should work with not patterns and lists` protect (mutation score None, coverage None; tests/exhaustive-match.test.ts); 8. `tests.exhaustive-match.exhaustive().issue #271: P.array should exhaustively match readonly arrays` protect (mutation score None, coverage None; tests/exhaustive-match.test.ts); 9. `tests.exhaustive-match.exhaustive().real world example` protect (mutation score None, coverage None; tests/exhaustive-match.test.ts); 10. `tests.exhaustive-match.exhaustive().reducer example` protect (mutation score None, coverage None; tests/exhaustive-match.test.ts)

## Check

Final survey (status `pass`, exit 0, 389 findings, 51 s):

| Check | Status | Findings | Score | Time |
|---|---|---|---|---|
| `conc.race` | skipped (unsupported) | 0 | — | 0.0 s |
| `gen.property` | pass | property.missing 19 | 0 falsified (n 0) | 0.0 s |
| `meta.oracle-independence` | skipped (needs-range) | 0 | — | 0.0 s |
| `meta.traceability` | skipped (needs-range) | 0 | — | 0.0 s |
| `perf.footprint` | pass | 0 | 650 dependencies (n 650) | 0.5 s |
| `spec.contracts` | skipped (unsupported) | 0 | — | 0.0 s |
| `static.analyze` | pass | 0 | 0 diagnostics (n 1) | 0.9 s |
| `static.api-surface` | skipped (needs-range) | 0 | — | 0.0 s |
| `static.clones` | fail | clone.cluster 14, clone.pair 32 | 167 pairs (n 684) | 0.0 s |
| `static.complexity` | fail | complexity.high 8, function.long 38, nesting.deep 2 | 28 cyclomatic_max (n 741) | 0.0 s |
| `static.crap` | pass | crap.watch 9 | 23.0104 max (n 81) | 0.0 s |
| `static.dead-code` | fail | dead.export 4, dead.symbol 6 | 6 dead_symbols (n 741) | 0.4 s |
| `static.deps` | fail | dep.cycle 1 | 1 cycles (n 88) | 0.1 s |
| `tests.assertion-density` | fail | density.sparse-file 4, density.sparse-test 26 | 3.07389 assertions_per_test (n 406) | 0.0 s |
| `tests.coverage` | fail | coverage.uncovered-branch 10, coverage.uncovered-symbol 3 | 0.940594 line_ratio (n 303) | 0.0 s |
| `tests.flaky` | pass | 0 | 0 unstable (n 456) | 10.2 s |
| `tests.mutation` | fail | mutant.survived 21, mutant.uncovered 128 | 0.467857 killed_ratio (n 280) | 0.5 s |
| `tests.quality` | fail | test.duplicate-setup 3, test.logic 4, test.no-assertion 19, test.weak-assertion 38 | 3.07389 assertion_density (n 406) | 0.7 s |
| `tests.weakening` | skipped (needs-range) | 0 | — | 0.0 s |

Deep check on the targets' four files (status `pass`, exit 0, 43 findings (0 fail, 43 warn, 0 info), scope 4 files and 85 symbols, 11 s); its 43 findings are survey findings (same IDs) and are triaged once below; it ran before the last fix, which concerns no file of its scope:

| Check | Status | Findings | Score | Time |
|---|---|---|---|---|
| `conc.linearizability` | skipped (unconfigured) | 0 | — | 0.0 s |
| `conc.race` | skipped (unsupported) | 0 | — | 0.0 s |
| `gen.fuzz` | skipped (unsupported) | 0 | — | 0.0 s |
| `gen.metamorphic` | skipped (unsupported) | 0 | — | 0.0 s |
| `gen.property` | pass | 0 | 0 falsified (n 0) | 0.2 s |
| `meta.oracle-independence` | skipped (needs-range) | 0 | — | 0.0 s |
| `meta.traceability` | skipped (needs-range) | 0 | — | 0.0 s |
| `perf.bench` | skipped (unconfigured) | 0 | — | 0.0 s |
| `perf.footprint` | pass | 0 | 650 dependencies (n 650) | 0.4 s |
| `spec.contracts` | skipped (unsupported) | 0 | — | 0.0 s |
| `static.analyze` | pass | 0 | 0 diagnostics (n 1) | 0.8 s |
| `static.api-surface` | skipped (needs-range) | 0 | — | 0.0 s |
| `static.clones` | fail | clone.pair 4 | 4 pairs (n 81) | 0.0 s |
| `static.complexity` | fail | function.long 3 | 3 cyclomatic_max (n 85) | 0.0 s |
| `static.crap` | pass | 0 | 0 max (n 0) | 0.0 s |
| `static.dead-code` | pass | 0 | 0 dead_symbols (n 85) | 0.1 s |
| `static.deps` | pass | 0 | 0 cycles (n 7) | 0.0 s |
| `tests.assertion-density` | fail | density.sparse-file 2, density.sparse-test 1 | 2.80357 assertions_per_test (n 56) | 0.0 s |
| `tests.coverage` | pass | 0 | 0 line_ratio (n 0) | 0.0 s |
| `tests.flaky` | pass | 0 | 0 unstable (n 54) | 6.0 s |
| `tests.mutation` | pass | 0 | 1 killed_ratio (n 0) | 0.1 s |
| `tests.quality` | fail | test.logic 3, test.no-assertion 10, test.weak-assertion 20 | 2.80357 assertion_density (n 56) | 0.0 s |
| `tests.weakening` | skipped (needs-range) | 0 | — | 0.0 s |
| `gen.differential` | skipped (unconfigured) | 0 | — | 0.0 s |

The first survey, before the fixes (status `partial`, exit 5, 475 findings): `tests.quality` reported `test.no-assertion` 217, `test.weak-assertion` 4, `test.duplicate-setup` 9, `test.logic` 5; `tests.assertion-density` 64; `static.clones` 51; `static.complexity` 49; the rest as in the final survey. `tests.mutation` scored 0.468 in the second and final surveys: 131 killed, 21 survived, 128 uncovered (118 of them in `benchmarks/` and `examples/`, which no test loads).

## Triage

Every finding of the final survey (the deep check's are among them), plus every finding of the first two surveys that a fix removed (verdict `false`, its reason naming the fix). Verification per ADR-138 §4:

- every `mutant.survived` was replayed: the mutation applied to the clone, `npx jest --silent --maxWorkers=4` run through `scripts/heavy.sh`, the file restored with `git checkout`; all 21 left the suite green (467 passed);
- every `coverage.*`, `crap.watch`, and `mutant.uncovered` row was recomputed from the run's `coverage/lcov.info` with the symbol-lines rule of ADR-079 (the lines after the first): all match;
- every `function.long` span and every `density.sparse-test` assertion count (`expect(`, `Expect<`, `@ts-expect-error`) was recomputed from the source: all match; a sparse test is `disputed` when at least half of its lines lie inside its assertions (bracket-matched from each `Expect<` or `expect(`), since its length is then the expected type it pins;
- every `complexity.high` cyclomatic count was recounted from the decision tokens (two differ from the recount by a named closure's own decisions, which Cupel gives the closure, as ADR-030 says);
- by hand: every `tests.quality` row of the kinds `test.logic`, `test.duplicate-setup`, and `test.no-assertion`, every `@ts-expect-error` weak assertion, every `static.dead-code`, `static.deps`, and `gen.property` row, both `nesting.deep` rows, and ten `static.clones` rows (the `bigints` and `wildcards` clusters, `flatMap` in `src/internals/helpers.ts` and its copy in `tests/exhaustive-match.test.ts`, the `numbers`, `return-type`, `nesting`, `output-type`, `invert-pattern` and benchmark pairs).

**`gen.property`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_3b781c` | `property.missing` | `benchmarks/compile-time/discriminated-union-exhaustive.ts:9` | disputed | It has parameters and no property test (correct), but it is benchmark or example code, not the library's. | none |
| `fnd_7cc28e` | `property.missing` | `benchmarks/compile-time/discriminated-union-non-exhaustive.ts:9` | disputed | It has parameters and no property test (correct), but it is benchmark or example code, not the library's. | none |
| `fnd_00bd4a` | `property.missing` | `benchmarks/compile-time/large-union-exhaustive.ts:8` | disputed | It has parameters and no property test (correct), but it is benchmark or example code, not the library's. | none |
| `fnd_1a1f63` | `property.missing` | `benchmarks/compile-time/large-union-non-exhaustive.ts:9` | disputed | It has parameters and no property test (correct), but it is benchmark or example code, not the library's. | none |
| `fnd_8da06c` | `property.missing` | `examples/gif-fetcher/src/searchGif.ts:6` | disputed | It has parameters and no property test (correct), but it is benchmark or example code, not the library's. | none |
| `fnd_8fe75c` | `property.missing` | `src/is-matching.ts:53` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |
| `fnd_20453b` | `property.missing` | `src/match.ts:32` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |
| `fnd_9bff09` | `property.missing` | `src/patterns.ts:1251` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |
| `fnd_6af4e2` | `property.missing` | `src/patterns.ts:1274` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |
| `fnd_9c203e` | `property.missing` | `src/patterns.ts:187` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |
| `fnd_32d6ea` | `property.missing` | `src/patterns.ts:246` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |
| `fnd_f5ccfe` | `property.missing` | `src/patterns.ts:299` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |
| `fnd_669213` | `property.missing` | `src/patterns.ts:362` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |
| `fnd_fe8707` | `property.missing` | `src/patterns.ts:445` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |
| `fnd_8a9194` | `property.missing` | `src/patterns.ts:536` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |
| `fnd_df61aa` | `property.missing` | `src/patterns.ts:572` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |
| `fnd_8eb393` | `property.missing` | `src/patterns.ts:611` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |
| `fnd_62e64a` | `property.missing` | `src/patterns.ts:646` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |
| `fnd_a97462` | `property.missing` | `src/patterns.ts:696` | true | A public function with parameters; the repository has no property test (no fast-check). | add a property |

**`static.clones`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_9a43d0` | `clone.cluster` | `tests/deep-exclude.test.ts:14` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_05f699` | `clone.cluster` | `tests/deep-exclude.test.ts:245` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_6e12ee` | `clone.cluster` | `tests/deep-exclude.test.ts:260` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_1f8a1d` | `clone.cluster` | `tests/exhaustive-match.test.ts:1019` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_9a9875` | `clone.cluster` | `tests/extract-precise-value.test.ts:213` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_69717f` | `clone.pair` | `tests/chainable.test.ts:218` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_595d5e` | `clone.pair` | `tests/deep-exclude.test.ts:283` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_09b78c` | `clone.pair` | `tests/exhaustive-match.test.ts:1040` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_be323b` | `clone.pair` | `tests/intersection-and-union.test.ts:108` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_f04118` | `clone.pair` | `tests/output-type.test.ts:7` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_3e02c3` | `clone.pair` | `tests/record.test.ts:157` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_5fc916` | `clone.cluster` | `benchmarks/always-last-digit.ts:6` | true | Members share one normalized shape (jaccard 0.89–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | none (benchmark copies) |
| `fnd_4ceb3b` | `clone.cluster` | `tests/bigints.test.ts:6` | true | Members share one normalized shape (jaccard 0.85–0.90), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_21fb0e` | `clone.cluster` | `tests/chainable.test.ts:24` | true | Members share one normalized shape (jaccard 0.87–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_df52e0` | `clone.cluster` | `tests/chainable.test.ts:7` | true | Members share one normalized shape (jaccard 0.90–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_c6e94e` | `clone.cluster` | `tests/chainable.test.ts:82` | true | Members share one normalized shape (jaccard 0.82–0.90), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_5109bf` | `clone.cluster` | `tests/nesting.test.ts:124` | true | Members share one normalized shape (jaccard 0.83–0.94), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_cb7d6e` | `clone.cluster` | `tests/nesting.test.ts:56` | true | Members share one normalized shape (jaccard 0.86–0.89), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_1707cd` | `clone.cluster` | `tests/numbers.test.ts:61` | true | Members share one normalized shape (jaccard 0.82–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_040be8` | `clone.cluster` | `tests/output-type.test.ts:7` | true | Members share one normalized shape (jaccard 0.82–0.90), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_e870cc` | `clone.cluster` | `tests/record.test.ts:119` | true | Members share one normalized shape (jaccard 0.82–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_49cc9c` | `clone.cluster` | `tests/record.test.ts:177` | true | Members share one normalized shape (jaccard 0.89–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_5449f4` | `clone.cluster` | `tests/variadic-tuples.test.ts:7` | true | Members share one normalized shape (jaccard 0.83–0.94), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_f35623` | `clone.cluster` | `tests/wildcards.test.ts:161` | true | Members share one normalized shape (jaccard 1.00–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_0c740a` | `clone.cluster` | `tests/wildcards.test.ts:8` | true | Members share one normalized shape (jaccard 0.88–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_8e6718` | `clone.pair` | `benchmarks/always-last-digit.ts:36` | true | Members share one normalized shape (jaccard 1.00–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | none (benchmark copies) |
| `fnd_2dd3ff` | `clone.pair` | `benchmarks/always-last-digit.ts:62` | true | Members share one normalized shape (jaccard 1.00–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | none (benchmark copies) |
| `fnd_aa8031` | `clone.pair` | `benchmarks/always-last-digit.ts:89` | true | Members share one normalized shape (jaccard 1.00–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | none (benchmark copies) |
| `fnd_30dcee` | `clone.pair` | `benchmarks/compile-time/discriminated-union-exhaustive.ts:9` | true | Members share one normalized shape (jaccard 0.93–0.93), differing only in names, literals, or a few nodes; ten such rows read by hand. | none (benchmark copies) |
| `fnd_c70a07` | `clone.pair` | `benchmarks/compile-time/large-union-exhaustive.ts:8` | true | Members share one normalized shape (jaccard 0.99–0.99), differing only in names, literals, or a few nodes; ten such rows read by hand. | none (benchmark copies) |
| `fnd_92ca05` | `clone.pair` | `src/internals/helpers.ts:132` | true | Members share one normalized shape (jaccard 1.00–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_213d72` | `clone.pair` | `tests/chainable.test.ts:37` | true | Members share one normalized shape (jaccard 1.00–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_fea682` | `clone.pair` | `tests/exhaustive-match.test.ts:1016` | true | Members share one normalized shape (jaccard 0.82–0.82), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_d48ffe` | `clone.pair` | `tests/exhaustive-match.test.ts:1019` | true | Members share one normalized shape (jaccard 0.86–0.86), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_40455b` | `clone.pair` | `tests/exhaustive-match.test.ts:14` | true | Members share one normalized shape (jaccard 0.98–0.98), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_55d24b` | `clone.pair` | `tests/exhaustive-match.test.ts:868` | true | Members share one normalized shape (jaccard 0.84–0.84), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_c0c835` | `clone.pair` | `tests/helpers.test.ts:15` | true | Members share one normalized shape (jaccard 0.93–0.93), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_0c0e24` | `clone.pair` | `tests/helpers.test.ts:25` | true | Members share one normalized shape (jaccard 0.88–0.88), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_540deb` | `clone.pair` | `tests/invert-pattern.test.ts:257` | true | Members share one normalized shape (jaccard 1.00–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_6bfe02` | `clone.pair` | `tests/invert-pattern.test.ts:386` | true | Members share one normalized shape (jaccard 1.00–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_f173ac` | `clone.pair` | `tests/multiple-patterns.test.ts:6` | true | Members share one normalized shape (jaccard 0.97–0.97), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_9ffa95` | `clone.pair` | `tests/multiple-patterns.test.ts:7` | true | Members share one normalized shape (jaccard 0.95–0.95), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_9a7f0e` | `clone.pair` | `tests/numbers.test.ts:22` | true | Members share one normalized shape (jaccard 0.88–0.88), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_0915dc` | `clone.pair` | `tests/output-type.test.ts:17` | true | Members share one normalized shape (jaccard 0.87–0.87), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_ca4e8f` | `clone.pair` | `tests/output-type.test.ts:26` | true | Members share one normalized shape (jaccard 0.84–0.84), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_c66225` | `clone.pair` | `tests/output-type.test.ts:36` | true | Members share one normalized shape (jaccard 0.89–0.89), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_c66f36` | `clone.pair` | `tests/output-type.test.ts:46` | true | Members share one normalized shape (jaccard 0.90–0.90), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_89de69` | `clone.pair` | `tests/output-type.test.ts:47` | true | Members share one normalized shape (jaccard 0.86–0.86), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_b70a46` | `clone.pair` | `tests/record.test.ts:344` | true | Members share one normalized shape (jaccard 0.91–0.91), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_8a48fe` | `clone.pair` | `tests/return-type.test.ts:12` | true | Members share one normalized shape (jaccard 1.00–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_063220` | `clone.pair` | `tests/return-type.test.ts:5` | true | Members share one normalized shape (jaccard 1.00–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_00ccf4` | `clone.pair` | `tests/select.test.ts:228` | true | Members share one normalized shape (jaccard 0.91–0.91), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_e017fe` | `clone.pair` | `tests/type-error.test.ts:56` | true | Members share one normalized shape (jaccard 0.82–0.82), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_3ae7c3` | `clone.pair` | `tests/variadic-tuples.test.ts:148` | true | Members share one normalized shape (jaccard 0.90–0.90), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_e8af61` | `clone.pair` | `tests/variadic-tuples.test.ts:149` | true | Members share one normalized shape (jaccard 0.84–0.84), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_0af4ef` | `clone.pair` | `tests/variadic-tuples.test.ts:34` | true | Members share one normalized shape (jaccard 1.00–1.00), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |
| `fnd_771a27` | `clone.pair` | `tests/variadic-tuples.test.ts:71` | true | Members share one normalized shape (jaccard 0.84–0.84), differing only in names, literals, or a few nodes; ten such rows read by hand. | extract a helper or table |

**`static.complexity`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_71fe75` | `complexity.high` | `tests/types.test.ts:105` | false | Survey 1 only: sixteen `return`s ending inline callbacks counted as early exits, cognitive 16 (fixed in 90d3669). | fixed |
| `fnd_d860ea` | `complexity.high` | `benchmarks/always-last-digit.ts:36` | true | Cyclomatic 11 and cognitive 20: decisions recounted from the source (a named closure's own excluded). | none |
| `fnd_2f4514` | `complexity.high` | `benchmarks/always-last-digit.ts:89` | true | Cyclomatic 11 and cognitive 55: decisions recounted from the source (a named closure's own excluded). | none |
| `fnd_647f30` | `complexity.high` | `benchmarks/nested-objects.ts:12` | true | Cyclomatic 28 and cognitive 30: decisions recounted from the source (a named closure's own excluded). | none |
| `fnd_d860ea` | `complexity.high` | `benchmarks/random-digit.ts:36` | true | Cyclomatic 11 and cognitive 20: decisions recounted from the source (a named closure's own excluded). | none |
| `fnd_2f4514` | `complexity.high` | `benchmarks/random-digit.ts:89` | true | Cyclomatic 11 and cognitive 55: decisions recounted from the source (a named closure's own excluded). | none |
| `fnd_73a966` | `complexity.high` | `scripts/completions/run.mjs:169` | true | Cyclomatic 10 and cognitive 18: decisions recounted from the source (a named closure's own excluded). | none |
| `fnd_361716` | `complexity.high` | `src/internals/helpers.ts:32` | true | Cyclomatic 23 and cognitive 62: decisions recounted from the source (a named closure's own excluded). | simplify |
| `fnd_db9b80` | `complexity.high` | `src/patterns.ts:445` | true | Cyclomatic 12 and cognitive 13: decisions recounted from the source (a named closure's own excluded). | simplify |
| `fnd_ee2814` | `function.long` | `benchmarks/compile-time/large-union-exhaustive.ts:8` | true | The span is 204 lines, recomputed from the source. | split |
| `fnd_847418` | `function.long` | `benchmarks/compile-time/large-union-non-exhaustive.ts:9` | true | The span is 206 lines, recomputed from the source. | split |
| `fnd_fba15d` | `function.long` | `examples/gif-fetcher/src/App.tsx:75` | true | The span is 82 lines, recomputed from the source. | split |
| `fnd_317a53` | `function.long` | `scripts/completions/run.mjs:107` | true | The span is 61 lines, recomputed from the source. | split |
| `fnd_c2bdc3` | `function.long` | `scripts/completions/run.mjs:169` | true | The span is 94 lines, recomputed from the source. | split |
| `fnd_cb9900` | `function.long` | `src/internals/helpers.ts:32` | true | The span is 86 lines, recomputed from the source. | split |
| `fnd_aeebdf` | `function.long` | `tests/deep-exclude.test.ts:395` | true | The span is 66 lines, recomputed from the source. | none (type-level cases) |
| `fnd_b7c4de` | `function.long` | `tests/deep-exclude.test.ts:63` | true | The span is 68 lines, recomputed from the source. | none (type-level cases) |
| `fnd_6c112c` | `function.long` | `tests/distribute-unions.test.ts:12` | true | The span is 271 lines, recomputed from the source. | none (type-level cases) |
| `fnd_e183e3` | `function.long` | `tests/distribute-unions.test.ts:284` | true | The span is 144 lines, recomputed from the source. | none (type-level cases) |
| `fnd_d70efd` | `function.long` | `tests/distribute-unions.test.ts:429` | true | The span is 88 lines, recomputed from the source. | none (type-level cases) |
| `fnd_6f383b` | `function.long` | `tests/distribute-unions.test.ts:566` | true | The span is 147 lines, recomputed from the source. | none (type-level cases) |
| `fnd_5ad87b` | `function.long` | `tests/distribute-unions.test.ts:867` | true | The span is 68 lines, recomputed from the source. | none (type-level cases) |
| `fnd_1a4096` | `function.long` | `tests/exhaustive-match.test.ts:148` | true | The span is 109 lines, recomputed from the source. | none (type-level cases) |
| `fnd_0aa4fe` | `function.long` | `tests/extract-precise-value.test.ts:8` | true | The span is 65 lines, recomputed from the source. | none (type-level cases) |
| `fnd_0d1ccc` | `function.long` | `tests/find-selected.test.ts:342` | true | The span is 67 lines, recomputed from the source. | none (type-level cases) |
| `fnd_cea09c` | `function.long` | `tests/intersection-and-union.test.ts:468` | true | The span is 62 lines, recomputed from the source. | none (type-level cases) |
| `fnd_b2186f` | `function.long` | `tests/intersection-and-union.test.ts:592` | true | The span is 78 lines, recomputed from the source. | none (type-level cases) |
| `fnd_5cd8db` | `function.long` | `tests/intersection-and-union.test.ts:94` | true | The span is 70 lines, recomputed from the source. | none (type-level cases) |
| `fnd_0678b1` | `function.long` | `tests/large-exhaustive.test.ts:155` | true | The span is 65 lines, recomputed from the source. | none (type-level cases) |
| `fnd_94acf7` | `function.long` | `tests/large-exhaustive.test.ts:326` | true | The span is 164 lines, recomputed from the source. | none (type-level cases) |
| `fnd_16b99e` | `function.long` | `tests/large-exhaustive.test.ts:38` | true | The span is 83 lines, recomputed from the source. | none (type-level cases) |
| `fnd_023fee` | `function.long` | `tests/multiple-patterns.test.ts:125` | true | The span is 90 lines, recomputed from the source. | none (type-level cases) |
| `fnd_daeb03` | `function.long` | `tests/optional.test.ts:131` | true | The span is 62 lines, recomputed from the source. | none (type-level cases) |
| `fnd_03b9f8` | `function.long` | `tests/optional.test.ts:47` | true | The span is 61 lines, recomputed from the source. | none (type-level cases) |
| `fnd_ffe44d` | `function.long` | `tests/pattern.test.ts:38` | true | The span is 77 lines, recomputed from the source. | none (type-level cases) |
| `fnd_6cb1cc` | `function.long` | `tests/real-world.test.ts:134` | true | The span is 76 lines, recomputed from the source. | none (type-level cases) |
| `fnd_551fc4` | `function.long` | `tests/real-world.test.ts:6` | true | The span is 127 lines, recomputed from the source. | none (type-level cases) |
| `fnd_8a58a6` | `function.long` | `tests/record.test.ts:400` | true | The span is 80 lines, recomputed from the source. | none (type-level cases) |
| `fnd_8e49c0` | `function.long` | `tests/select.test.ts:97` | true | The span is 67 lines, recomputed from the source. | none (type-level cases) |
| `fnd_4b4aee` | `function.long` | `tests/tuples.test.ts:103` | true | The span is 71 lines, recomputed from the source. | none (type-level cases) |
| `fnd_c7b547` | `function.long` | `tests/type-is-matching.test.ts:64` | true | The span is 146 lines, recomputed from the source. | none (type-level cases) |
| `fnd_ab3b6d` | `function.long` | `tests/types.test.ts:105` | true | The span is 123 lines, recomputed from the source. | none (type-level cases) |
| `fnd_180ada` | `function.long` | `tests/types.test.ts:23` | true | The span is 81 lines, recomputed from the source. | none (type-level cases) |
| `fnd_b7e96c` | `function.long` | `tests/variadic-tuples.test.ts:243` | true | The span is 127 lines, recomputed from the source. | none (type-level cases) |
| `fnd_d02e59` | `function.long` | `tests/variadic-tuples.test.ts:371` | true | The span is 129 lines, recomputed from the source. | none (type-level cases) |
| `fnd_e55676` | `function.long` | `tests/when.test.ts:193` | true | The span is 93 lines, recomputed from the source. | none (type-level cases) |
| `fnd_2309aa` | `function.long` | `tests/when.test.ts:196` | true | The span is 89 lines, recomputed from the source. | none (type-level cases) |
| `fnd_0cad59` | `nesting.deep` | `benchmarks/always-last-digit.ts:89` | true | A chain of ten nested ternaries, read by hand. | none (benchmark of the ternary form) |
| `fnd_0cad59` | `nesting.deep` | `benchmarks/random-digit.ts:89` | true | A chain of ten nested ternaries, read by hand. | none (benchmark of the ternary form) |

**`static.crap`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_1a961b` | `crap.watch` | `src/internals/helpers.ts:32` | true | CRAP 23.01 recomputed from cyclomatic 23 and the run's lcov coverage 0.973. | test the uncovered lines |
| `fnd_10fad2` | `crap.watch` | `src/match.ts:50` | true | CRAP 10.00 recomputed from cyclomatic 10 and the run's lcov coverage 1.000. | test the uncovered lines |
| `fnd_c6d67e` | `crap.watch` | `src/patterns.ts:246` | true | CRAP 5.00 recomputed from cyclomatic 5 and the run's lcov coverage 1.000. | test the uncovered lines |
| `fnd_9343e5` | `crap.watch` | `src/patterns.ts:299` | true | CRAP 5.25 recomputed from cyclomatic 5 and the run's lcov coverage 0.786. | test the uncovered lines |
| `fnd_1a037d` | `crap.watch` | `src/patterns.ts:317` | true | CRAP 6.00 recomputed from cyclomatic 2 and the run's lcov coverage 0.000. | test the uncovered lines |
| `fnd_9632d7` | `crap.watch` | `src/patterns.ts:362` | true | CRAP 7.54 recomputed from cyclomatic 7 and the run's lcov coverage 0.778. | test the uncovered lines |
| `fnd_1a037d` | `crap.watch` | `src/patterns.ts:381` | true | CRAP 6.00 recomputed from cyclomatic 2 and the run's lcov coverage 0.000. | test the uncovered lines |
| `fnd_728a79` | `crap.watch` | `src/patterns.ts:445` | true | CRAP 12.02 recomputed from cyclomatic 12 and the run's lcov coverage 0.944. | test the uncovered lines |
| `fnd_482b41` | `crap.watch` | `src/patterns.ts:696` | true | CRAP 6.00 recomputed from cyclomatic 6 and the run's lcov coverage 1.000. | test the uncovered lines |

**`static.dead-code`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_423d9d` | `dead.symbol` | `benchmarks/always-last-digit.ts:21` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_b67ea0` | `dead.symbol` | `benchmarks/always-last-digit.ts:36` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_18ac43` | `dead.symbol` | `benchmarks/always-last-digit.ts:6` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_ee1bf0` | `dead.symbol` | `benchmarks/always-last-digit.ts:62` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_cc25f2` | `dead.symbol` | `benchmarks/always-last-digit.ts:89` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_6a7ff6` | `dead.symbol` | `benchmarks/nested-objects.ts:12` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_a695df` | `dead.symbol` | `benchmarks/nested-objects.ts:4` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_b63396` | `dead.symbol` | `benchmarks/nested-objects.ts:54` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_593727` | `dead.symbol` | `benchmarks/random-digit.ts:113` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_423d9d` | `dead.symbol` | `benchmarks/random-digit.ts:21` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_b67ea0` | `dead.symbol` | `benchmarks/random-digit.ts:36` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_18ac43` | `dead.symbol` | `benchmarks/random-digit.ts:6` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_ee1bf0` | `dead.symbol` | `benchmarks/random-digit.ts:62` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_cc25f2` | `dead.symbol` | `benchmarks/random-digit.ts:89` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_3f8db7` | `dead.symbol` | `scripts/completions/run.mjs:107` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_b11267` | `dead.symbol` | `scripts/completions/run.mjs:169` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_494503` | `dead.symbol` | `scripts/completions/run.mjs:264` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_a0391d` | `dead.symbol` | `scripts/completions/run.mjs:272` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_b93ce0` | `dead.symbol` | `scripts/completions/run.mjs:35` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_d64698` | `dead.symbol` | `scripts/completions/run.mjs:43` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_aa5fde` | `dead.symbol` | `scripts/completions/run.mjs:71` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_0c8ca8` | `dead.symbol` | `scripts/completions/run.mjs:74` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_097873` | `dead.symbol` | `scripts/completions/run.mjs:83` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_8eacd4` | `dead.symbol` | `scripts/completions/run.mjs:88` | false | Surveys 1 and 2 only: the script's top-level code calls it, directly or in a callback it hands to `suite`, and the file is run directly (fixed in e12d282). | fixed |
| `fnd_05bd0e` | `dead.export` | `benchmarks/compile-time/discriminated-union-exhaustive.ts:9` | disputed | Nothing imports it (correct); the file is a compile-time benchmark `tsc` checks, never imported. | none |
| `fnd_ec6856` | `dead.export` | `benchmarks/compile-time/discriminated-union-non-exhaustive.ts:9` | disputed | Nothing imports it (correct); the file is a compile-time benchmark `tsc` checks, never imported. | none |
| `fnd_9fa68a` | `dead.export` | `benchmarks/compile-time/large-union-exhaustive.ts:8` | disputed | Nothing imports it (correct); the file is a compile-time benchmark `tsc` checks, never imported. | none |
| `fnd_32c969` | `dead.export` | `benchmarks/compile-time/large-union-non-exhaustive.ts:9` | disputed | Nothing imports it (correct); the file is a compile-time benchmark `tsc` checks, never imported. | none |
| `fnd_2c50ba` | `dead.symbol` | `examples/one-file-demo/one-file-demo.ts:146` | true | Never referenced anywhere, its own file included. | delete or use it |
| `fnd_9b8f57` | `dead.symbol` | `examples/one-file-demo/one-file-demo.ts:25` | true | Never referenced anywhere, its own file included. | delete or use it |
| `fnd_049855` | `dead.symbol` | `examples/one-file-demo/one-file-demo.ts:60` | true | Never referenced anywhere, its own file included. | delete or use it |
| `fnd_e1ceaf` | `dead.symbol` | `examples/one-file-demo/one-file-demo.ts:79` | true | Never referenced anywhere, its own file included. | delete or use it |
| `fnd_709cbf` | `dead.symbol` | `examples/one-file-demo/one-file-demo.ts:91` | true | Never referenced anywhere, its own file included. | delete or use it |
| `fnd_f1082e` | `dead.symbol` | `tests/narrow.test.ts:27` | true | Never referenced anywhere, its own file included. | delete or use it |

**`static.deps`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_a7e74a` | `dep.cycle` | `src/internals/helpers.ts:9` | disputed | The cycle is in the source, but `src/types/Pattern.ts` imports `matcher` from `../patterns` only for types, which TypeScript elides: no cycle at run time. | none |

**`tests.assertion-density`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_ff6551` | `density.sparse-file` | `tests/deep-exclude.test.ts:14` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_d51510` | `density.sparse-file` | `tests/distribute-unions.test.ts:12` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e033d0` | `density.sparse-file` | `tests/extract-precise-value.test.ts:8` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f5d90d` | `density.sparse-file` | `tests/find-selected.test.ts:21` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_2cb6e7` | `density.sparse-file` | `tests/generics.test.ts:18` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_2409eb` | `density.sparse-file` | `tests/helpers.test.ts:15` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_1a3db8` | `density.sparse-file` | `tests/invert-pattern.test.ts:10` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_764b81` | `density.sparse-file` | `tests/output-type.test.ts:7` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_74ee26` | `density.sparse-file` | `tests/pattern.test.ts:8` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_75f083` | `density.sparse-file` | `tests/readonly.test.ts:6` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_1237f1` | `density.sparse-file` | `tests/return-type.test.ts:4` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_8513ee` | `density.sparse-file` | `tests/type-error.test.ts:8` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_25e1c7` | `density.sparse-file` | `tests/type-is-matching.test.ts:7` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_0b05cb` | `density.sparse-file` | `tests/types.test.ts:8` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_d7ec81` | `density.sparse-test` | `tests/exhaustive-match.test.ts:550` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_95359f` | `density.sparse-test` | `tests/exhaustive-match.test.ts:580` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_485a7d` | `density.sparse-test` | `tests/exhaustive-match.test.ts:857` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c12728` | `density.sparse-test` | `tests/exhaustive-match.test.ts:932` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f004ac` | `density.sparse-test` | `tests/intersection-and-union.test.ts:222` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_2005fe` | `density.sparse-test` | `tests/intersection-and-union.test.ts:468` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_293afc` | `density.sparse-test` | `tests/intersection-and-union.test.ts:531` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_6b10e8` | `density.sparse-test` | `tests/intersection-and-union.test.ts:592` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_0bf3e4` | `density.sparse-test` | `tests/is-matching.test.ts:31` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_ac4bda` | `density.sparse-test` | `tests/is-matching.test.ts:5` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_3b4a8a` | `density.sparse-test` | `tests/lists.test.ts:6` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_b833f2` | `density.sparse-test` | `tests/matcher-protocol.test.ts:52` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_11a17d` | `density.sparse-test` | `tests/matcher-protocol.test.ts:95` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_04528a` | `density.sparse-test` | `tests/multiple-patterns.test.ts:125` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_d171b0` | `density.sparse-test` | `tests/multiple-patterns.test.ts:46` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e8a5c1` | `density.sparse-test` | `tests/multiple-patterns.test.ts:6` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_794a37` | `density.sparse-test` | `tests/nesting.test.ts:6` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_247e71` | `density.sparse-test` | `tests/objects.test.ts:92` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_2b31a9` | `density.sparse-test` | `tests/optional-props.test.ts:5` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_3042ff` | `density.sparse-test` | `tests/optional-props.test.ts:72` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_15db59` | `density.sparse-test` | `tests/optional.test.ts:109` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_15db59` | `density.sparse-test` | `tests/optional.test.ts:131` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_aa00e5` | `density.sparse-test` | `tests/optional.test.ts:47` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_ce0526` | `density.sparse-test` | `tests/primitive-values.test.ts:5` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_259e17` | `density.sparse-test` | `tests/record.test.ts:230` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_ae1bec` | `density.sparse-test` | `tests/record.test.ts:481` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_a6f67b` | `density.sparse-test` | `tests/record.test.ts:73` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_63aa8c` | `density.sparse-test` | `tests/select.test.ts:165` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f7255c` | `density.sparse-test` | `tests/select.test.ts:201` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_099370` | `density.sparse-test` | `tests/select.test.ts:63` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_0f4767` | `density.sparse-test` | `tests/tuples.test.ts:103` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c076f5` | `density.sparse-test` | `tests/tuples.test.ts:21` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_fe2695` | `density.sparse-test` | `tests/unions.test.ts:25` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_a3b1b9` | `density.sparse-test` | `tests/unions.test.ts:73` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_fd518d` | `density.sparse-test` | `tests/variadic-tuples.test.ts:530` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f79cb7` | `density.sparse-test` | `tests/when.test.ts:104` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_b670d5` | `density.sparse-test` | `tests/when.test.ts:147` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_63bf5d` | `density.sparse-test` | `tests/when.test.ts:326` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c31292` | `density.sparse-test` | `tests/when.test.ts:369` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_830bb3` | `density.sparse-test` | `tests/when.test.ts:43` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_eb430f` | `density.sparse-file` | `tests/branded-nominal-types.test.ts:11` | true | 2 assertion(s) over 3 tests, recounted from its cases. | assert in every case |
| `fnd_9de658` | `density.sparse-file` | `tests/exhaustive-fallback.test.ts:5` | true | 5 assertion(s) over 4 tests, recounted from its cases. | assert in every case |
| `fnd_b378fa` | `density.sparse-file` | `tests/lists.test.ts:6` | true | 6 assertion(s) over 4 tests, recounted from its cases. | assert in every case |
| `fnd_56a479` | `density.sparse-file` | `tests/nesting.test.ts:6` | true | 13 assertion(s) over 11 tests, recounted from its cases. | assert in every case |
| `fnd_4f54ba` | `density.sparse-test` | `tests/deep-exclude.test.ts:350` | disputed | 2 assertion(s) in 44 lines (recounted), but 91% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_cc5597` | `density.sparse-test` | `tests/deep-exclude.test.ts:576` | disputed | 1 assertion(s) in 22 lines (recounted), but 91% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_f2d473` | `density.sparse-test` | `tests/distribute-unions.test.ts:12` | disputed | 5 assertion(s) in 271 lines (recounted), but 83% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_ffa5a2` | `density.sparse-test` | `tests/distribute-unions.test.ts:284` | disputed | 4 assertion(s) in 144 lines (recounted), but 97% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_03a00e` | `density.sparse-test` | `tests/distribute-unions.test.ts:518` | disputed | 1 assertion(s) in 45 lines (recounted), but 91% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_50f989` | `density.sparse-test` | `tests/distribute-unions.test.ts:566` | disputed | 2 assertion(s) in 147 lines (recounted), but 60% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_794055` | `density.sparse-test` | `tests/distribute-unions.test.ts:867` | disputed | 2 assertion(s) in 68 lines (recounted), but 91% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_e78e77` | `density.sparse-test` | `tests/exhaustive-match.test.ts:106` | true | 1 assertion(s) in 22 lines, recounted; 0% of the lines are inside assertions. | assert more of the output |
| `fnd_6ca2d4` | `density.sparse-test` | `tests/extract-precise-value.test.ts:347` | disputed | 1 assertion(s) in 35 lines (recounted), but 63% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_0a94d3` | `density.sparse-test` | `tests/extract-precise-value.test.ts:96` | disputed | 2 assertion(s) in 44 lines (recounted), but 91% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_2e939a` | `density.sparse-test` | `tests/find-selected.test.ts:478` | true | 1 assertion(s) in 40 lines, recounted; 2% of the lines are inside assertions. | assert more of the output |
| `fnd_4bfaeb` | `density.sparse-test` | `tests/instance-of.test.ts:48` | true | 1 assertion(s) in 24 lines, recounted; 25% of the lines are inside assertions. | assert more of the output |
| `fnd_cea4cd` | `density.sparse-test` | `tests/large-exhaustive.test.ts:12` | disputed | 1 assertion(s) in 25 lines (recounted), but 92% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_37b3b0` | `density.sparse-test` | `tests/large-exhaustive.test.ts:129` | disputed | 1 assertion(s) in 25 lines (recounted), but 92% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_0d2e88` | `density.sparse-test` | `tests/large-exhaustive.test.ts:326` | disputed | 3 assertion(s) in 164 lines (recounted), but 66% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_9d312f` | `density.sparse-test` | `tests/large-exhaustive.test.ts:38` | disputed | 4 assertion(s) in 83 lines (recounted), but 98% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_120989` | `density.sparse-test` | `tests/nesting.test.ts:99` | disputed | 1 assertion(s) in 22 lines (recounted), but 91% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_0f0ff4` | `density.sparse-test` | `tests/not.test.ts:153` | true | 1 assertion(s) in 30 lines, recounted; 30% of the lines are inside assertions. | assert more of the output |
| `fnd_368b49` | `density.sparse-test` | `tests/not.test.ts:199` | true | 1 assertion(s) in 35 lines, recounted; 3% of the lines are inside assertions. | assert more of the output |
| `fnd_fe5898` | `density.sparse-test` | `tests/not.test.ts:265` | true | 1 assertion(s) in 24 lines, recounted; 0% of the lines are inside assertions. | assert more of the output |
| `fnd_a732d5` | `density.sparse-test` | `tests/real-world.test.ts:134` | disputed | 3 assertion(s) in 76 lines (recounted), but 91% of the lines are inside the assertions: each spells out its expected type. | none |
| `fnd_c17fa8` | `density.sparse-test` | `tests/record.test.ts:525` | true | 2 assertion(s) in 58 lines, recounted; 0% of the lines are inside assertions. | assert more of the output |
| `fnd_9179a7` | `density.sparse-test` | `tests/select.test.ts:403` | true | 1 assertion(s) in 30 lines, recounted; 3% of the lines are inside assertions. | assert more of the output |
| `fnd_e32b78` | `density.sparse-test` | `tests/unions.test.ts:132` | true | 1 assertion(s) in 30 lines, recounted; 3% of the lines are inside assertions. | assert more of the output |
| `fnd_ad7a93` | `density.sparse-test` | `tests/when.test.ts:193` | true | 4 assertion(s) in 93 lines, recounted; 34% of the lines are inside assertions. | assert more of the output |
| `fnd_f79cb7` | `density.sparse-test` | `tests/when.test.ts:6` | true | 1 assertion(s) in 21 lines, recounted; 38% of the lines are inside assertions. | assert more of the output |

**`tests.coverage`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_25209e` | `coverage.uncovered-branch` | `src/internals/helpers.ts:120` | true | Recomputed from the run's lcov (lines after the first, ADR-079; BRDA hits). | add a test for the path |
| `fnd_eed2b9` | `coverage.uncovered-branch` | `src/internals/helpers.ts:32` | true | Recomputed from the run's lcov (lines after the first, ADR-079; BRDA hits). | add a test for the path |
| `fnd_19ef31` | `coverage.uncovered-branch` | `src/is-matching.ts:53` | true | Recomputed from the run's lcov (lines after the first, ADR-079; BRDA hits). | add a test for the path |
| `fnd_b99a1b` | `coverage.uncovered-branch` | `src/patterns.ts:148` | true | Recomputed from the run's lcov (lines after the first, ADR-079; BRDA hits). | add a test for the path |
| `fnd_c00e4a` | `coverage.uncovered-branch` | `src/patterns.ts:165` | true | Recomputed from the run's lcov (lines after the first, ADR-079; BRDA hits). | add a test for the path |
| `fnd_f76207` | `coverage.uncovered-branch` | `src/patterns.ts:299` | true | Recomputed from the run's lcov (lines after the first, ADR-079; BRDA hits). | add a test for the path |
| `fnd_ac35c1` | `coverage.uncovered-branch` | `src/patterns.ts:362` | true | Recomputed from the run's lcov (lines after the first, ADR-079; BRDA hits). | add a test for the path |
| `fnd_cdcaec` | `coverage.uncovered-branch` | `src/patterns.ts:410` | true | Recomputed from the run's lcov (lines after the first, ADR-079; BRDA hits). | add a test for the path |
| `fnd_b0d913` | `coverage.uncovered-branch` | `src/patterns.ts:445` | true | Recomputed from the run's lcov (lines after the first, ADR-079; BRDA hits). | add a test for the path |
| `fnd_6668e0` | `coverage.uncovered-branch` | `src/patterns.ts:696` | true | Recomputed from the run's lcov (lines after the first, ADR-079; BRDA hits). | add a test for the path |
| `fnd_08c4c7` | `coverage.uncovered-symbol` | `src/match.ts:128` | true | Recomputed from the run's lcov (lines after the first, ADR-079; BRDA hits). | add a test for the path |
| `fnd_486be5` | `coverage.uncovered-symbol` | `src/patterns.ts:317` | true | Recomputed from the run's lcov (lines after the first, ADR-079; BRDA hits). | add a test for the path |
| `fnd_486be5` | `coverage.uncovered-symbol` | `src/patterns.ts:381` | true | Recomputed from the run's lcov (lines after the first, ADR-079; BRDA hits). | add a test for the path |

**`tests.mutation`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_e7b737` | `mutant.survived` | `src/match.ts:62` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_372175` | `mutant.survived` | `src/match.ts:64` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_c80995` | `mutant.survived` | `src/match.ts:64` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_d1c348` | `mutant.survived` | `src/match.ts:64` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_4bde13` | `mutant.survived` | `src/patterns.ts:1036` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_45ee59` | `mutant.survived` | `src/patterns.ts:1036` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_7067b8` | `mutant.survived` | `src/patterns.ts:1048` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_facc3c` | `mutant.survived` | `src/patterns.ts:1048` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_71f8fa` | `mutant.survived` | `src/patterns.ts:1163` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_7a8649` | `mutant.survived` | `src/patterns.ts:1163` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_38dd2e` | `mutant.survived` | `src/patterns.ts:1175` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_f47f3f` | `mutant.survived` | `src/patterns.ts:1175` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_0125d3` | `mutant.survived` | `src/patterns.ts:144` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_102464` | `mutant.survived` | `src/patterns.ts:160` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_e9e505` | `mutant.survived` | `src/patterns.ts:377` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_a259c3` | `mutant.survived` | `src/patterns.ts:377` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_b53281` | `mutant.survived` | `src/patterns.ts:396` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_f70e65` | `mutant.survived` | `src/patterns.ts:497` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_ef2ad2` | `mutant.survived` | `src/patterns.ts:497` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_3cced6` | `mutant.survived` | `src/patterns.ts:763` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_c64f8f` | `mutant.survived` | `src/patterns.ts:958` | true | Replayed: the mutation applied to the clone leaves `jest` green (467 tests). | add a test that pins the boundary |
| `fnd_41587d` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:100` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_795533` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:102` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_5be990` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:104` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_763e9a` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:106` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_0544f5` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:108` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_b96165` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:23` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_ea39d8` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:24` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_1bf617` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:37` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_f7afb4` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:37` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_898a3d` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:39` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_5af459` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:39` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_c1ae3a` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:41` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_1762e3` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:43` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_297c7a` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:45` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_725338` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:47` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_4dc6a7` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:49` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_15c7a2` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:51` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_c3f52a` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:53` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_a27e7d` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:55` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_d5834a` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:64` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_7b25d7` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:66` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_487ad0` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:8` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_dc48ed` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:9` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_c7ef4e` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:90` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_4a5458` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:90` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_4c5d51` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:92` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_3a39f8` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:92` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_1dbbfe` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:94` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_306884` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:96` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_f05ff5` | `mutant.uncovered` | `benchmarks/always-last-digit.ts:98` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_790736` | `mutant.uncovered` | `benchmarks/compile-time/discriminated-union-exhaustive.ts:11` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_9ebe96` | `mutant.uncovered` | `benchmarks/compile-time/discriminated-union-exhaustive.ts:15` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_bd67a5` | `mutant.uncovered` | `benchmarks/compile-time/discriminated-union-non-exhaustive.ts:12` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_49ffa7` | `mutant.uncovered` | `benchmarks/compile-time/discriminated-union-non-exhaustive.ts:16` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_e74bdc` | `mutant.uncovered` | `benchmarks/compile-time/large-union-exhaustive.ts:10` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_7fcbb2` | `mutant.uncovered` | `benchmarks/compile-time/large-union-exhaustive.ts:11` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_fa5eb3` | `mutant.uncovered` | `benchmarks/compile-time/large-union-non-exhaustive.ts:12` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_0b99b7` | `mutant.uncovered` | `benchmarks/compile-time/large-union-non-exhaustive.ts:13` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_9e8545` | `mutant.uncovered` | `benchmarks/nested-objects.ts:14` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_f6fd74` | `mutant.uncovered` | `benchmarks/nested-objects.ts:15` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_506b47` | `mutant.uncovered` | `benchmarks/nested-objects.ts:15` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_dddc4a` | `mutant.uncovered` | `benchmarks/nested-objects.ts:16` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_436881` | `mutant.uncovered` | `benchmarks/nested-objects.ts:17` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_65d946` | `mutant.uncovered` | `benchmarks/nested-objects.ts:17` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_1266bf` | `mutant.uncovered` | `benchmarks/nested-objects.ts:18` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_98c6b2` | `mutant.uncovered` | `benchmarks/nested-objects.ts:19` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_6600d8` | `mutant.uncovered` | `benchmarks/nested-objects.ts:20` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_78fedc` | `mutant.uncovered` | `benchmarks/nested-objects.ts:20` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_aafcc1` | `mutant.uncovered` | `benchmarks/nested-objects.ts:21` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_4ab211` | `mutant.uncovered` | `benchmarks/nested-objects.ts:22` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_f5d7aa` | `mutant.uncovered` | `benchmarks/nested-objects.ts:22` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_1ba489` | `mutant.uncovered` | `benchmarks/nested-objects.ts:23` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_0c288e` | `mutant.uncovered` | `benchmarks/nested-objects.ts:24` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_e4e0ed` | `mutant.uncovered` | `benchmarks/nested-objects.ts:28` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_33ab0d` | `mutant.uncovered` | `benchmarks/nested-objects.ts:29` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_1f301f` | `mutant.uncovered` | `benchmarks/nested-objects.ts:29` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_9a1488` | `mutant.uncovered` | `benchmarks/nested-objects.ts:30` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_69c839` | `mutant.uncovered` | `benchmarks/nested-objects.ts:31` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_5a97bd` | `mutant.uncovered` | `benchmarks/nested-objects.ts:31` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_32784e` | `mutant.uncovered` | `benchmarks/nested-objects.ts:32` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_48efed` | `mutant.uncovered` | `benchmarks/nested-objects.ts:33` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_6c1b57` | `mutant.uncovered` | `benchmarks/nested-objects.ts:34` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_664573` | `mutant.uncovered` | `benchmarks/nested-objects.ts:34` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_69f168` | `mutant.uncovered` | `benchmarks/nested-objects.ts:34` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_d7d79b` | `mutant.uncovered` | `benchmarks/nested-objects.ts:34` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_161f20` | `mutant.uncovered` | `benchmarks/nested-objects.ts:35` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_38d55d` | `mutant.uncovered` | `benchmarks/nested-objects.ts:35` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_fada03` | `mutant.uncovered` | `benchmarks/nested-objects.ts:39` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_7e638d` | `mutant.uncovered` | `benchmarks/nested-objects.ts:40` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_b1b7b4` | `mutant.uncovered` | `benchmarks/nested-objects.ts:40` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_882655` | `mutant.uncovered` | `benchmarks/nested-objects.ts:41` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_aaeb32` | `mutant.uncovered` | `benchmarks/nested-objects.ts:42` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_7607c6` | `mutant.uncovered` | `benchmarks/nested-objects.ts:42` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_ad097a` | `mutant.uncovered` | `benchmarks/nested-objects.ts:43` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_d3995c` | `mutant.uncovered` | `benchmarks/nested-objects.ts:44` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_d45613` | `mutant.uncovered` | `benchmarks/nested-objects.ts:44` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_20f71f` | `mutant.uncovered` | `benchmarks/nested-objects.ts:45` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_365d3b` | `mutant.uncovered` | `benchmarks/nested-objects.ts:46` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_c59936` | `mutant.uncovered` | `benchmarks/nested-objects.ts:56` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_4111a9` | `mutant.uncovered` | `benchmarks/nested-objects.ts:57` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_aa81c6` | `mutant.uncovered` | `benchmarks/nested-objects.ts:59` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_a9de7f` | `mutant.uncovered` | `benchmarks/nested-objects.ts:59` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_6f6eb8` | `mutant.uncovered` | `benchmarks/nested-objects.ts:64` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_1af0b9` | `mutant.uncovered` | `benchmarks/nested-objects.ts:64` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_1fd9fc` | `mutant.uncovered` | `benchmarks/nested-objects.ts:64` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_105a01` | `mutant.uncovered` | `benchmarks/nested-objects.ts:7` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_41587d` | `mutant.uncovered` | `benchmarks/random-digit.ts:100` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_795533` | `mutant.uncovered` | `benchmarks/random-digit.ts:102` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_5be990` | `mutant.uncovered` | `benchmarks/random-digit.ts:104` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_763e9a` | `mutant.uncovered` | `benchmarks/random-digit.ts:106` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_0544f5` | `mutant.uncovered` | `benchmarks/random-digit.ts:108` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_b12ccf` | `mutant.uncovered` | `benchmarks/random-digit.ts:113` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_b96165` | `mutant.uncovered` | `benchmarks/random-digit.ts:23` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_ea39d8` | `mutant.uncovered` | `benchmarks/random-digit.ts:24` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_1bf617` | `mutant.uncovered` | `benchmarks/random-digit.ts:37` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_f7afb4` | `mutant.uncovered` | `benchmarks/random-digit.ts:37` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_898a3d` | `mutant.uncovered` | `benchmarks/random-digit.ts:39` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_5af459` | `mutant.uncovered` | `benchmarks/random-digit.ts:39` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_c1ae3a` | `mutant.uncovered` | `benchmarks/random-digit.ts:41` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_1762e3` | `mutant.uncovered` | `benchmarks/random-digit.ts:43` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_297c7a` | `mutant.uncovered` | `benchmarks/random-digit.ts:45` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_725338` | `mutant.uncovered` | `benchmarks/random-digit.ts:47` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_4dc6a7` | `mutant.uncovered` | `benchmarks/random-digit.ts:49` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_15c7a2` | `mutant.uncovered` | `benchmarks/random-digit.ts:51` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_c3f52a` | `mutant.uncovered` | `benchmarks/random-digit.ts:53` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_a27e7d` | `mutant.uncovered` | `benchmarks/random-digit.ts:55` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_d5834a` | `mutant.uncovered` | `benchmarks/random-digit.ts:64` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_7b25d7` | `mutant.uncovered` | `benchmarks/random-digit.ts:66` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_487ad0` | `mutant.uncovered` | `benchmarks/random-digit.ts:8` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_dc48ed` | `mutant.uncovered` | `benchmarks/random-digit.ts:9` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_c7ef4e` | `mutant.uncovered` | `benchmarks/random-digit.ts:90` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_4a5458` | `mutant.uncovered` | `benchmarks/random-digit.ts:90` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_4c5d51` | `mutant.uncovered` | `benchmarks/random-digit.ts:92` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_3a39f8` | `mutant.uncovered` | `benchmarks/random-digit.ts:92` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_1dbbfe` | `mutant.uncovered` | `benchmarks/random-digit.ts:94` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_306884` | `mutant.uncovered` | `benchmarks/random-digit.ts:96` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_f05ff5` | `mutant.uncovered` | `benchmarks/random-digit.ts:98` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_9a2fbf` | `mutant.uncovered` | `examples/one-file-demo/one-file-demo.ts:94` | true | No test loads the file (absent from the run's lcov). | none (not library code) |
| `fnd_df32a0` | `mutant.uncovered` | `src/patterns.ts:312` | true | Line 312 has 0 hits in the run's lcov. | add a test for the path |
| `fnd_546469` | `mutant.uncovered` | `src/patterns.ts:318` | true | Line 318 has 0 hits in the run's lcov. | add a test for the path |
| `fnd_1c04eb` | `mutant.uncovered` | `src/patterns.ts:330` | true | Line 330 has 0 hits in the run's lcov. | add a test for the path |
| `fnd_6b2a88` | `mutant.uncovered` | `src/patterns.ts:330` | true | Line 330 has 0 hits in the run's lcov. | add a test for the path |
| `fnd_fc5e4a` | `mutant.uncovered` | `src/patterns.ts:330` | true | Line 330 has 0 hits in the run's lcov. | add a test for the path |
| `fnd_4a8d0f` | `mutant.uncovered` | `src/patterns.ts:378` | true | Line 378 has 0 hits in the run's lcov. | add a test for the path |
| `fnd_546469` | `mutant.uncovered` | `src/patterns.ts:382` | true | Line 382 has 0 hits in the run's lcov. | add a test for the path |
| `fnd_96fe3a` | `mutant.uncovered` | `src/patterns.ts:402` | true | Line 402 has 0 hits in the run's lcov. | add a test for the path |
| `fnd_c19fa4` | `mutant.uncovered` | `src/patterns.ts:402` | true | Line 402 has 0 hits in the run's lcov. | add a test for the path |
| `fnd_30a325` | `mutant.uncovered` | `src/patterns.ts:416` | true | Line 416 has 0 hits in the run's lcov. | add a test for the path |

**`tests.quality`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_d86a77` | `test.duplicate-setup` | `tests/deep-exclude.test.ts:63` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_a55550` | `test.duplicate-setup` | `tests/distribute-unions.test.ts:12` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_bff9e1` | `test.duplicate-setup` | `tests/exhaustive-match.test.ts:14` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_6b010c` | `test.duplicate-setup` | `tests/extract-precise-value.test.ts:213` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_4654a6` | `test.duplicate-setup` | `tests/find-selected.test.ts:78` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_db08d8` | `test.duplicate-setup` | `tests/invert-pattern.test.ts:376` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_cf418b` | `test.duplicate-setup` | `tests/type-is-matching.test.ts:26` | false | Survey 1 only: type arguments erased, so tests checking different types normalized alike (fixed in 6a582a5). | fixed |
| `fnd_adb44e` | `test.logic` | `tests/is-matching.test.ts:105` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f6d1f8` | `test.no-assertion` | `tests/build-many.test.ts:6` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e1b818` | `test.no-assertion` | `tests/deep-exclude.test.ts:132` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7d506a` | `test.no-assertion` | `tests/deep-exclude.test.ts:14` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7e8851` | `test.no-assertion` | `tests/deep-exclude.test.ts:149` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c375c8` | `test.no-assertion` | `tests/deep-exclude.test.ts:181` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_34592d` | `test.no-assertion` | `tests/deep-exclude.test.ts:189` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_9d0044` | `test.no-assertion` | `tests/deep-exclude.test.ts:210` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e87687` | `test.no-assertion` | `tests/deep-exclude.test.ts:245` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_af7cc5` | `test.no-assertion` | `tests/deep-exclude.test.ts:260` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_5f78c3` | `test.no-assertion` | `tests/deep-exclude.test.ts:283` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_d87aca` | `test.no-assertion` | `tests/deep-exclude.test.ts:30` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f6d1f8` | `test.no-assertion` | `tests/deep-exclude.test.ts:350` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7e8851` | `test.no-assertion` | `tests/deep-exclude.test.ts:395` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f6d1f8` | `test.no-assertion` | `tests/deep-exclude.test.ts:45` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f6d1f8` | `test.no-assertion` | `tests/deep-exclude.test.ts:463` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_34592d` | `test.no-assertion` | `tests/deep-exclude.test.ts:488` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f6d1f8` | `test.no-assertion` | `tests/deep-exclude.test.ts:52` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c89196` | `test.no-assertion` | `tests/deep-exclude.test.ts:562` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_fce8cd` | `test.no-assertion` | `tests/deep-exclude.test.ts:576` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c85796` | `test.no-assertion` | `tests/deep-exclude.test.ts:63` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c89196` | `test.no-assertion` | `tests/distribute-unions.test.ts:1025` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_a769b6` | `test.no-assertion` | `tests/distribute-unions.test.ts:12` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_34592d` | `test.no-assertion` | `tests/distribute-unions.test.ts:284` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_0c4d39` | `test.no-assertion` | `tests/distribute-unions.test.ts:429` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c89196` | `test.no-assertion` | `tests/distribute-unions.test.ts:518` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c375c8` | `test.no-assertion` | `tests/distribute-unions.test.ts:566` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c375c8` | `test.no-assertion` | `tests/distribute-unions.test.ts:755` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c375c8` | `test.no-assertion` | `tests/distribute-unions.test.ts:763` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_34592d` | `test.no-assertion` | `tests/distribute-unions.test.ts:796` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_02fa34` | `test.no-assertion` | `tests/distribute-unions.test.ts:844` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_435bd4` | `test.no-assertion` | `tests/distribute-unions.test.ts:867` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7e8851` | `test.no-assertion` | `tests/distribute-unions.test.ts:936` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c375c8` | `test.no-assertion` | `tests/distribute-unions.test.ts:992` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_9849b6` | `test.no-assertion` | `tests/exhaustive-fallback.test.ts:29` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_644164` | `test.no-assertion` | `tests/exhaustive-fallback.test.ts:41` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_b9d5dd` | `test.no-assertion` | `tests/exhaustive-match.test.ts:106` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_d96627` | `test.no-assertion` | `tests/exhaustive-match.test.ts:129` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_bb2e03` | `test.no-assertion` | `tests/exhaustive-match.test.ts:14` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_507701` | `test.no-assertion` | `tests/exhaustive-match.test.ts:148` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_631dbd` | `test.no-assertion` | `tests/exhaustive-match.test.ts:258` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e32766` | `test.no-assertion` | `tests/exhaustive-match.test.ts:287` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_294185` | `test.no-assertion` | `tests/exhaustive-match.test.ts:330` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f12bb6` | `test.no-assertion` | `tests/exhaustive-match.test.ts:356` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_54e7ac` | `test.no-assertion` | `tests/exhaustive-match.test.ts:375` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_d5a5a9` | `test.no-assertion` | `tests/exhaustive-match.test.ts:430` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_b99f2e` | `test.no-assertion` | `tests/exhaustive-match.test.ts:60` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e77d38` | `test.no-assertion` | `tests/exhaustive-match.test.ts:603` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_0238b5` | `test.no-assertion` | `tests/exhaustive-match.test.ts:618` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_4bbe27` | `test.no-assertion` | `tests/exhaustive-match.test.ts:652` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_a1ec11` | `test.no-assertion` | `tests/exhaustive-match.test.ts:667` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_673651` | `test.no-assertion` | `tests/exhaustive-match.test.ts:772` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_0286ae` | `test.no-assertion` | `tests/exhaustive-match.test.ts:842` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_23ab9f` | `test.no-assertion` | `tests/exhaustive-match.test.ts:897` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c375c8` | `test.no-assertion` | `tests/extract-precise-value.test.ts:141` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_bdc5e8` | `test.no-assertion` | `tests/extract-precise-value.test.ts:173` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_6a7751` | `test.no-assertion` | `tests/extract-precise-value.test.ts:213` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_3319c3` | `test.no-assertion` | `tests/extract-precise-value.test.ts:228` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_aae0bb` | `test.no-assertion` | `tests/extract-precise-value.test.ts:257` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_af7cc5` | `test.no-assertion` | `tests/extract-precise-value.test.ts:300` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_1f4b0a` | `test.no-assertion` | `tests/extract-precise-value.test.ts:319` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_6a7751` | `test.no-assertion` | `tests/extract-precise-value.test.ts:327` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_786f1c` | `test.no-assertion` | `tests/extract-precise-value.test.ts:347` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_41eb1a` | `test.no-assertion` | `tests/extract-precise-value.test.ts:385` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_8a84f0` | `test.no-assertion` | `tests/extract-precise-value.test.ts:395` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e87687` | `test.no-assertion` | `tests/extract-precise-value.test.ts:435` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_af7cc5` | `test.no-assertion` | `tests/extract-precise-value.test.ts:455` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_af7cc5` | `test.no-assertion` | `tests/extract-precise-value.test.ts:475` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_af7cc5` | `test.no-assertion` | `tests/extract-precise-value.test.ts:488` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_af7cc5` | `test.no-assertion` | `tests/extract-precise-value.test.ts:507` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_8de0a8` | `test.no-assertion` | `tests/extract-precise-value.test.ts:530` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c89196` | `test.no-assertion` | `tests/extract-precise-value.test.ts:74` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_5f78c3` | `test.no-assertion` | `tests/extract-precise-value.test.ts:8` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c89196` | `test.no-assertion` | `tests/extract-precise-value.test.ts:85` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f6d1f8` | `test.no-assertion` | `tests/extract-precise-value.test.ts:96` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_af7cc5` | `test.no-assertion` | `tests/find-selected.test.ts:129` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e87687` | `test.no-assertion` | `tests/find-selected.test.ts:148` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e87687` | `test.no-assertion` | `tests/find-selected.test.ts:180` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_634ac5` | `test.no-assertion` | `tests/find-selected.test.ts:21` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_6a7751` | `test.no-assertion` | `tests/find-selected.test.ts:212` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_6a7751` | `test.no-assertion` | `tests/find-selected.test.ts:226` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c375c8` | `test.no-assertion` | `tests/find-selected.test.ts:241` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f6d1f8` | `test.no-assertion` | `tests/find-selected.test.ts:273` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_af7cc5` | `test.no-assertion` | `tests/find-selected.test.ts:301` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c89196` | `test.no-assertion` | `tests/find-selected.test.ts:326` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7d506a` | `test.no-assertion` | `tests/find-selected.test.ts:342` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_ec277a` | `test.no-assertion` | `tests/find-selected.test.ts:443` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_ef3ecd` | `test.no-assertion` | `tests/find-selected.test.ts:478` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_af7cc5` | `test.no-assertion` | `tests/find-selected.test.ts:78` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e87687` | `test.no-assertion` | `tests/find-selected.test.ts:98` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_5fe67d` | `test.no-assertion` | `tests/generics.test.ts:109` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_137eeb` | `test.no-assertion` | `tests/generics.test.ts:18` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_af5295` | `test.no-assertion` | `tests/generics.test.ts:37` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e77b52` | `test.no-assertion` | `tests/generics.test.ts:49` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_1ce35b` | `test.no-assertion` | `tests/generics.test.ts:72` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7e8851` | `test.no-assertion` | `tests/helpers.test.ts:15` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7e8851` | `test.no-assertion` | `tests/helpers.test.ts:25` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7e8851` | `test.no-assertion` | `tests/helpers.test.ts:37` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7e8851` | `test.no-assertion` | `tests/helpers.test.ts:47` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_5f5738` | `test.no-assertion` | `tests/helpers.test.ts:77` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_ee5fcb` | `test.no-assertion` | `tests/infer.test.ts:26` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_371a4c` | `test.no-assertion` | `tests/infer.test.ts:6` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_387cb3` | `test.no-assertion` | `tests/intersection-and-union.test.ts:265` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_aa3e2a` | `test.no-assertion` | `tests/intersection-and-union.test.ts:38` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_3686b6` | `test.no-assertion` | `tests/intersection-and-union.test.ts:713` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_4bf7cb` | `test.no-assertion` | `tests/intersection-and-union.test.ts:728` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c111bd` | `test.no-assertion` | `tests/intersection-and-union.test.ts:94` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_fb85b1` | `test.no-assertion` | `tests/invert-pattern.test.ts:10` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_b5320b` | `test.no-assertion` | `tests/invert-pattern.test.ts:100` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_1f84f0` | `test.no-assertion` | `tests/invert-pattern.test.ts:128` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_cdaf1b` | `test.no-assertion` | `tests/invert-pattern.test.ts:157` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f6d1f8` | `test.no-assertion` | `tests/invert-pattern.test.ts:201` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_1f4b0a` | `test.no-assertion` | `tests/invert-pattern.test.ts:228` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_1f4b0a` | `test.no-assertion` | `tests/invert-pattern.test.ts:237` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_1f4b0a` | `test.no-assertion` | `tests/invert-pattern.test.ts:245` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_4f11bb` | `test.no-assertion` | `tests/invert-pattern.test.ts:257` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c3da95` | `test.no-assertion` | `tests/invert-pattern.test.ts:273` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_4f11bb` | `test.no-assertion` | `tests/invert-pattern.test.ts:283` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_51fc67` | `test.no-assertion` | `tests/invert-pattern.test.ts:302` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_23fcb1` | `test.no-assertion` | `tests/invert-pattern.test.ts:333` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_31238d` | `test.no-assertion` | `tests/invert-pattern.test.ts:343` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c290ab` | `test.no-assertion` | `tests/invert-pattern.test.ts:352` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_de77bc` | `test.no-assertion` | `tests/invert-pattern.test.ts:362` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_b07379` | `test.no-assertion` | `tests/invert-pattern.test.ts:376` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_59a078` | `test.no-assertion` | `tests/invert-pattern.test.ts:386` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_59a078` | `test.no-assertion` | `tests/invert-pattern.test.ts:394` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_20dbcd` | `test.no-assertion` | `tests/invert-pattern.test.ts:47` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_b221c4` | `test.no-assertion` | `tests/invert-pattern.test.ts:75` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_1763ae` | `test.no-assertion` | `tests/is-matching.test.ts:105` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_aec750` | `test.no-assertion` | `tests/is-matching.test.ts:123` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e3962d` | `test.no-assertion` | `tests/lists.test.ts:62` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_de4eba` | `test.no-assertion` | `tests/multiple-patterns.test.ts:216` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_b180d8` | `test.no-assertion` | `tests/multiple-patterns.test.ts:225` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f4b494` | `test.no-assertion` | `tests/multiple-patterns.test.ts:86` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_90637f` | `test.no-assertion` | `tests/multiple-patterns.test.ts:93` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_92584c` | `test.no-assertion` | `tests/narrow.test.ts:16` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_703399` | `test.no-assertion` | `tests/narrow.test.ts:5` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_5e90cc` | `test.no-assertion` | `tests/not.test.ts:153` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_38abe4` | `test.no-assertion` | `tests/not.test.ts:253` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e29b8d` | `test.no-assertion` | `tests/not.test.ts:265` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_8ee11d` | `test.no-assertion` | `tests/numbers.test.ts:45` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_6542a2` | `test.no-assertion` | `tests/objects.test.ts:11` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_42d671` | `test.no-assertion` | `tests/objects.test.ts:65` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_9edc7b` | `test.no-assertion` | `tests/output-type.test.ts:100` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_5d6831` | `test.no-assertion` | `tests/output-type.test.ts:115` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_4b1a10` | `test.no-assertion` | `tests/output-type.test.ts:144` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_b7d24b` | `test.no-assertion` | `tests/output-type.test.ts:154` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_59241d` | `test.no-assertion` | `tests/output-type.test.ts:36` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_0284e6` | `test.no-assertion` | `tests/output-type.test.ts:46` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_3826ec` | `test.no-assertion` | `tests/output-type.test.ts:61` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_177601` | `test.no-assertion` | `tests/output-type.test.ts:7` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_dd9165` | `test.no-assertion` | `tests/output-type.test.ts:90` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_a647b0` | `test.no-assertion` | `tests/pattern.test.ts:17` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_60d216` | `test.no-assertion` | `tests/pattern.test.ts:38` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_71edbf` | `test.no-assertion` | `tests/readonly.test.ts:28` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_afd366` | `test.no-assertion` | `tests/readonly.test.ts:44` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_5360e0` | `test.no-assertion` | `tests/readonly.test.ts:6` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_143884` | `test.no-assertion` | `tests/readonly.test.ts:75` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_a4a890` | `test.no-assertion` | `tests/record.test.ts:400` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_f4faae` | `test.no-assertion` | `tests/record.test.ts:511` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_d33bfe` | `test.no-assertion` | `tests/record.test.ts:525` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_2c319f` | `test.no-assertion` | `tests/return-type.test.ts:29` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_515c20` | `test.no-assertion` | `tests/return-type.test.ts:4` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_12a144` | `test.no-assertion` | `tests/return-type.test.ts:40` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_0c0ccb` | `test.no-assertion` | `tests/select.test.ts:228` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7fa06b` | `test.no-assertion` | `tests/select.test.ts:237` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_1071e2` | `test.no-assertion` | `tests/select.test.ts:403` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_3da01d` | `test.no-assertion` | `tests/type-error.test.ts:119` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_34cb58` | `test.no-assertion` | `tests/type-error.test.ts:132` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_63c852` | `test.no-assertion` | `tests/type-error.test.ts:34` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_b67ca7` | `test.no-assertion` | `tests/type-error.test.ts:46` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_185783` | `test.no-assertion` | `tests/type-error.test.ts:65` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e6e4f1` | `test.no-assertion` | `tests/type-error.test.ts:8` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_a6d5ff` | `test.no-assertion` | `tests/type-is-matching.test.ts:211` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7e8851` | `test.no-assertion` | `tests/type-is-matching.test.ts:249` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e68c20` | `test.no-assertion` | `tests/type-is-matching.test.ts:259` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e87687` | `test.no-assertion` | `tests/type-is-matching.test.ts:26` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7e8851` | `test.no-assertion` | `tests/type-is-matching.test.ts:291` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7d506a` | `test.no-assertion` | `tests/type-is-matching.test.ts:305` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c375c8` | `test.no-assertion` | `tests/type-is-matching.test.ts:340` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_6a7751` | `test.no-assertion` | `tests/type-is-matching.test.ts:40` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e87687` | `test.no-assertion` | `tests/type-is-matching.test.ts:48` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c188e5` | `test.no-assertion` | `tests/type-is-matching.test.ts:64` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_a9bb86` | `test.no-assertion` | `tests/type-is-matching.test.ts:7` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_27a98d` | `test.no-assertion` | `tests/types.test.ts:105` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_8e25c8` | `test.no-assertion` | `tests/types.test.ts:229` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_c2ce52` | `test.no-assertion` | `tests/types.test.ts:23` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_a001bd` | `test.no-assertion` | `tests/types.test.ts:286` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_53de79` | `test.no-assertion` | `tests/types.test.ts:312` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_e70c42` | `test.no-assertion` | `tests/types.test.ts:332` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_4769dd` | `test.no-assertion` | `tests/types.test.ts:349` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_1a567d` | `test.no-assertion` | `tests/types.test.ts:374` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_7f9957` | `test.no-assertion` | `tests/variadic-tuples.test.ts:185` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_d1e35e` | `test.no-assertion` | `tests/variadic-tuples.test.ts:243` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_6c3d5e` | `test.no-assertion` | `tests/variadic-tuples.test.ts:371` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_726941` | `test.no-assertion` | `tests/when.test.ts:193` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_a3666a` | `test.no-assertion` | `tests/when.test.ts:28` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_3a999f` | `test.no-assertion` | `tests/when.test.ts:70` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_8a4d92` | `test.no-assertion` | `tests/wildcards.test.ts:117` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_664291` | `test.weak-assertion` | `tests/record.test.ts:194` | false | Survey 1 only: the case's `Expect<Equal<…>>` or `@ts-expect-error` assertions were not read (fixed in 1a2a807). | fixed |
| `fnd_2680a0` | `test.duplicate-setup` | `tests/invert-pattern.test.ts:257` | true | The tests open with the same three statements, names and literals aside, read by hand. | extract a fixture |
| `fnd_d6b6e4` | `test.duplicate-setup` | `tests/invert-pattern.test.ts:386` | true | The tests open with the same three statements, names and literals aside, read by hand. | extract a fixture |
| `fnd_a27b72` | `test.duplicate-setup` | `tests/record.test.ts:157` | true | The tests open with the same three statements, names and literals aside, read by hand. | extract a fixture |
| `fnd_a037eb` | `test.logic` | `tests/branded-nominal-types.test.ts:27` | true | Read by hand: a try/catch or a ternary in the test. | use `toThrow` or a table |
| `fnd_8765c7` | `test.logic` | `tests/exhaustive-match.test.ts:811` | true | Read by hand: a try/catch or a ternary in the test. | use `toThrow` or a table |
| `fnd_9e02d6` | `test.logic` | `tests/exhaustive-match.test.ts:827` | true | Read by hand: a try/catch or a ternary in the test. | use `toThrow` or a table |
| `fnd_83df8d` | `test.logic` | `tests/when.test.ts:369` | true | Read by hand: a try/catch or a ternary in the test. | use `toThrow` or a table |
| `fnd_96faa4` | `test.no-assertion` | `tests/branded-nominal-types.test.ts:27` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_1d45a6` | `test.no-assertion` | `tests/exhaustive-match.test.ts:320` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_8100c1` | `test.no-assertion` | `tests/exhaustive-match.test.ts:413` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_e56cb1` | `test.no-assertion` | `tests/exhaustive-match.test.ts:494` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_a35637` | `test.no-assertion` | `tests/exhaustive-match.test.ts:635` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_273aa3` | `test.no-assertion` | `tests/exhaustive-match.test.ts:701` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_e8560a` | `test.no-assertion` | `tests/exhaustive-match.test.ts:748` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_e05df7` | `test.no-assertion` | `tests/exhaustive-match.test.ts:983` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_e2dcb8` | `test.no-assertion` | `tests/intersection-and-union.test.ts:165` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_b151ee` | `test.no-assertion` | `tests/large-exhaustive.test.ts:155` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_d42860` | `test.no-assertion` | `tests/lists.test.ts:43` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_173572` | `test.no-assertion` | `tests/multiple-patterns.test.ts:108` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_f31808` | `test.no-assertion` | `tests/multiple-patterns.test.ts:114` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_e6bb82` | `test.no-assertion` | `tests/not.test.ts:235` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_8a6fd5` | `test.no-assertion` | `tests/pattern.test.ts:8` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_1f25c7` | `test.no-assertion` | `tests/tuples.test.ts:188` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_c2d163` | `test.no-assertion` | `tests/types.test.ts:360` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_fba982` | `test.no-assertion` | `tests/types.test.ts:8` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_0ea327` | `test.no-assertion` | `tests/when.test.ts:287` | true | No `expect`, type assertion, or `@ts-expect-error`; it fails only if `.exhaustive()` throws or the file does not type-check. | assert on the result |
| `fnd_c82918` | `test.weak-assertion` | `tests/exhaustive-fallback.test.ts:19` | true | Its only assertion is a `toThrow()` without an expected error, or `@ts-expect-error`. | pin the error |
| `fnd_aa343b` | `test.weak-assertion` | `tests/exhaustive-fallback.test.ts:29` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_ba44dc` | `test.weak-assertion` | `tests/exhaustive-fallback.test.ts:41` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_27a173` | `test.weak-assertion` | `tests/exhaustive-match.test.ts:106` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_0ebb9d` | `test.weak-assertion` | `tests/exhaustive-match.test.ts:129` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_e3ba32` | `test.weak-assertion` | `tests/exhaustive-match.test.ts:258` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_49093a` | `test.weak-assertion` | `tests/exhaustive-match.test.ts:287` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_f016ea` | `test.weak-assertion` | `tests/exhaustive-match.test.ts:330` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_26127b` | `test.weak-assertion` | `tests/exhaustive-match.test.ts:356` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_f57bf5` | `test.weak-assertion` | `tests/exhaustive-match.test.ts:375` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_5ede66` | `test.weak-assertion` | `tests/exhaustive-match.test.ts:430` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_90256c` | `test.weak-assertion` | `tests/exhaustive-match.test.ts:603` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_0f7dca` | `test.weak-assertion` | `tests/exhaustive-match.test.ts:652` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_58ff42` | `test.weak-assertion` | `tests/exhaustive-match.test.ts:842` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_4b8c0a` | `test.weak-assertion` | `tests/exhaustive-match.test.ts:897` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_41d91d` | `test.weak-assertion` | `tests/infer.test.ts:26` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_acbff6` | `test.weak-assertion` | `tests/intersection-and-union.test.ts:205` | true | Its only assertion is a `toThrow()` without an expected error, or `@ts-expect-error`. | pin the error |
| `fnd_7a7d94` | `test.weak-assertion` | `tests/is-matching.test.ts:123` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_088a22` | `test.weak-assertion` | `tests/multiple-patterns.test.ts:216` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_88d18e` | `test.weak-assertion` | `tests/multiple-patterns.test.ts:86` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_fb2516` | `test.weak-assertion` | `tests/multiple-patterns.test.ts:93` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_da0ef9` | `test.weak-assertion` | `tests/not.test.ts:253` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_3a8c1e` | `test.weak-assertion` | `tests/not.test.ts:265` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_4726d1` | `test.weak-assertion` | `tests/numbers.test.ts:45` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_bc5a99` | `test.weak-assertion` | `tests/record.test.ts:511` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_1c6aaa` | `test.weak-assertion` | `tests/record.test.ts:525` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_1bf3f2` | `test.weak-assertion` | `tests/return-type.test.ts:29` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_4adf99` | `test.weak-assertion` | `tests/return-type.test.ts:4` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_400c7c` | `test.weak-assertion` | `tests/return-type.test.ts:40` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_75c662` | `test.weak-assertion` | `tests/type-error.test.ts:132` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_060f67` | `test.weak-assertion` | `tests/type-error.test.ts:46` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_7f1f96` | `test.weak-assertion` | `tests/type-error.test.ts:65` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_6b5986` | `test.weak-assertion` | `tests/type-error.test.ts:8` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_56b0fc` | `test.weak-assertion` | `tests/types.test.ts:286` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_1597fb` | `test.weak-assertion` | `tests/types.test.ts:294` | true | Its only assertion is a `toThrow()` without an expected error, or `@ts-expect-error`. | pin the error |
| `fnd_98ac47` | `test.weak-assertion` | `tests/types.test.ts:312` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_3a8bf3` | `test.weak-assertion` | `tests/types.test.ts:332` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |
| `fnd_c72d9a` | `test.weak-assertion` | `tests/types.test.ts:349` | true | Its only assertions are `@ts-expect-error` lines: a type error, not which. | pin the error (`expectTypeOf` or a message) |

## False-positive rate per check

`false / (true + false)`, disputed rows apart.

| Check | True | False | Disputed | Rate |
|---|---|---|---|---|
| `gen.property` | 14 | 0 | 5 | 0.00 |
| `static.clones` | 46 | 11 | 0 | 0.19 |
| `static.complexity` | 48 | 1 | 0 | 0.02 |
| `static.crap` | 9 | 0 | 0 | 0.00 |
| `static.dead-code` | 6 | 24 | 4 | 0.80 |
| `static.deps` | 0 | 0 | 1 | — |
| `tests.assertion-density` | 15 | 54 | 15 | 0.78 |
| `tests.coverage` | 13 | 0 | 0 | 0.00 |
| `tests.mutation` | 149 | 0 | 0 | 0.00 |
| `tests.quality` | 64 | 207 | 0 | 0.76 |

Every other check that ran (`perf.footprint`, `static.analyze`, `tests.flaky`) reported nothing: no rate. The false rows are all from the first two surveys and all fixed; the final survey has none.

## Check bugs fixed

- **TypeScript's type-level assertions assert** (`tests.quality`, `tests.assertion-density`): ts-pattern tests its types with `type t = Expect<Equal<typeof x, string>>` and `// @ts-expect-error`, which ts-jest (and the project's `tsc`) enforce, and Cupel read 198 such tests as asserting nothing. A TypeScript-only `queries/typescript/tests.scm` captures `Expect<Equal<…>>` as `type-equal` (equality, strong), any other `Expect<…>` as `type-true` (truthiness, weak), and a `// @ts-expect-error` comment as `type-error` (throws, weak: a type error, not which). Fixture: `crates/cupel-fixtures/repos/type-assertions/typescript/`; tests `ecma_tests::type_level_assertions_assert_in_typescript_tests` and `crates/cupel-fixtures/tests/type_assertions.rs`. Commit 1a2a807. It removed 198 `test.no-assertion`, 1 `test.weak-assertion` (a `toBeDefined` beside an `Expect<Equal<…>>`), 1 `test.logic` (branches that guard type assertions, ADR-069), and 54 density rows; it added 35 `test.weak-assertion` rows, tests whose only assertions are `@ts-expect-error` lines (true), and 20 density rows, from the repository's density rising from 0.86 to 3.07 assertions per test (true or disputed above).
- **A return that ends an inline callback is not an early exit** (`static.complexity`): decisions in an anonymous callback stay with the function that passes it (ADR-030), and the `return` that ends the callback was judged final against that function's body, so `should infer values correctly in handler` (sixteen `.with(P.x, (x) => {{ …; return 'ok'; }})`) read cognitive 16. `[decisions] functions` names the anonymous function kinds whose own final `return` ends only them (ECMAScript: `arrow_function`, `function_expression`, `generator_function`). Fixture: `crates/cupel-fixtures/repos/inline-callbacks/typescript/`; tests `ecma_decisions::a_return_that_ends_an_inline_callback_is_not_early_and_one_inside_it_is` and `crates/cupel-fixtures/tests/inline_callbacks.rs`. Commit 90d3669. It removed 1 `complexity.high`.
- **A TypeScript type declaration keeps its type arguments when it normalizes** (`static.clones`, `tests.quality`'s `test.duplicate-setup`): `[normalize] erase` drops `type_arguments` so a typed function and its JavaScript twin normalize alike, which also emptied every `type cases = [Expect<Equal<…>>, …]`, so tests checking different types were type-1 clones (the 15-line `DeepExclude.Primitives` of a 67-line `FindSelected` test) and tests opening with three different `type` declarations repeated their setup. `[normalize] keep` names kinds below which nothing is erased (ECMAScript: `type_alias_declaration`). Fixture: `crates/cupel-fixtures/repos/type-clones/typescript/`; tests `ecma_norm::a_type_declaration_keeps_its_type_arguments` and `crates/cupel-fixtures/tests/type_clones.rs`. Commit 6a582a5. It removed 11 clone findings (6 pairs, 5 clusters) and 7 `test.duplicate-setup`; the clusters re-formed by shape (6 new clone rows, and 1 new `test.duplicate-setup` whose type declarations really repeat; true above).

- **A function a module's top-level code calls is an entry point** (`static.dead-code`): `scripts/completions/run.mjs` is run with `node` and calls its functions from top-level statements (`const refRoot = checkoutRef();`, `[ref, await run(refRoot)]`), and each benny benchmark's top-level `suite("…", add("…", () => testExhaustive(9)), …)` runs the callbacks it is handed; nothing imports either, so the module pass reported their 24 functions unreachable. ECMAScript's `api.scm` makes a function an entry point when a top-level statement calls it, directly, awaited, or as a declaration's value (alone or in an array), or when a callback it hands to a call (or to a call among that call's arguments) calls it, as ADR-138 §9 does for Python's module-level calls (group 2). Fixture: `crates/cupel-fixtures/repos/script-entry/typescript/`; tests `ecma_api::functions_module_level_code_calls_are_entry_points` and `crates/cupel-fixtures/tests/script_entry.rs`. Commit e12d282. It removed 24 `dead.symbol`; the remaining 4 `dead.export` of `benchmarks/compile-time/` (functions `tsc` checks and nothing calls) stay disputed.

Rerun after the fixes: the removed rows are gone and every other finding of the first survey is still reported (by check, kind, file, and line), with the additions listed above.

Not fixed (open items, none counted false):

- **The survey's protect targets are test functions** whenever a run mutates and covers source (as ADR-138's consequences already record), so the deep check grades four test files and runs no mutant.
- **Rankings name every clone group's language `javascript`** (`rankings.dimensions[].entries[].lang` for `static.clones`), TypeScript groups included, while their findings say `typescript`: a ranking defect, not a finding.

## What a maintainer would do first

The 21 surviving mutants are boundaries no test pins: `P.number.positive()`/`negative()` and their bigint twins never see `0` (`> 0` → `>= 0` and `0` → `1` survive on all four, eight mutants), `.with()`'s handling of several patterns before the handler (`src/match.ts:62–64`) survives four, and `P.map`'s empty map and key-and-value match, `P.record`'s one-argument form, `P.nonNullable`, `P.number.lt`, `P.select`'s key, and the variadic iterator survive one or two each; one test per boundary would kill most. Then `P.set` and `P.map` selections (their `selector` closures never run) and `.narrow()`, which no test calls. The 38 `@ts-expect-error`-only tests and the 19 tests that only call `.exhaustive()` are how this library tests its types; the first are fine as they are, and the second would read better with an `expect` on the result.
