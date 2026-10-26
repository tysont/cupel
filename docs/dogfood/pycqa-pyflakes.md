# External dogfood: PyCQA/pyflakes

Group 2 of P9.9 ([ADR-138](../adr/CUPEL_DECISION_138.md)): the first Python repository. Cupel at `8e8bef5` (branch `p9-dogfood-python-java`) for the first runs and `8b6e07b` for the final ones.

## Repository

- **URL:** https://github.com/PyCQA/pyflakes, commit `f0a8b98` (2026-10-14), default branch `main`.
- **Lines:** 10,237 lines of Python in 24 files (7,223 in `pyflakes/test/`); one package.
- **License:** MIT.
- **Suite:** `python -m unittest` passes in 0.4 s (795 tests, 28 skipped for other Python versions; CPython 3.12.10 from `make dev-setup`). Selection: the Python linter most projects run through flake8, a unittest suite with an asserting harness (`pyflakes/test/harness.py`), no services. It also qualified as the repository the previous group 2 agent's helper fixture was shaped on (a `TestCase` subclass's `self.flakes(...)` defined in the package's harness).

## Commands

From Cupel's worktree, with `C=target/release/cupel` (the first survey and check ran the debug build `target/debug/cupel` at `8e8bef5`; the final ones the release build at `8b6e07b`), `R=<scratchpad>/p9-group2c/repos/pyflakes`, each command through `scripts/heavy.sh`:

```
$C init --write --cwd $R                                                        # kept, two keys set by hand (below)
$C survey --depth standard --budget 30m --no-install --format json --cwd $R    # first: 106.7 s, exit 0; final: 3.2 s (facts reused), exit 0
$C check --depth deep --no-install --format json --cwd $R \
   --paths pyflakes/checker.py pyflakes/messages.py                             # first: 51.9 s, exit 0; final: 3.4 s, exit 0
```

`cupel init` wrote `[lang.python] test = "python -m unittest"` and `coverage = "python -m coverage run -m unittest && python -m coverage lcov -o {out}"`. Two keys were set by hand: `mutation = { jobs = 2 }` (the brief's two mutant slots, inside ADR-148's memory budget), and the coverage command became `python -m coverage run --rcfile=/dev/null -m unittest && python -m coverage lcov --rcfile=/dev/null -o {out}`, because `setup.cfg` names the coverage plugin `covdefaults`, which the pinned environment does not install: the first survey's `tests.coverage` ended in `error` (`exited with 1`) and `tests.mutation` with it (`fact mutation-outcomes is declared but was not produced`). Without the plugin, covdefaults' version pragmas (`# pragma: >=3.15 cover`) are not applied; the rows they would have excluded are disputed below. The survey's top ten targets are in `pyflakes/checker.py` (8) and `pyflakes/messages.py` (2), the two files the check ran on. Raw envelopes (2.6–8.8 MB) stay in the agent's scratchpad.

## Survey

Final `survey` at `8b6e07b`: `pass`, exit 0, 2,212 findings (2,041 fail, 112 warn, 59 info), of which 2,038 are `static.analyze`; nothing was left out by the budget. The first survey at `8e8bef5` (same configuration) reported 2,221: the nine the fixes removed. The rankings, verbatim (`rank. target value (location)`):

- `static.clones.cluster_size` per group, highest worst, 26 measured: 1. `pyflakes.test.test_imports.TestImportationObject.test_importfrom_relative and 7 others` 8 (pyflakes/test/test_imports.py:42); 2. `pyflakes.test.test_imports.Test.test_aliasedImportShadowModule and 6 others` 7 (pyflakes/test/test_imports.py:152); 3. `pyflakes.test.test_imports.Test.test_shadowedByForDeep and 5 others` 6 (pyflakes/test/test_imports.py:516); 4. `pyflakes.test.test_dict.Test.test_multiple_duplicate_keys and 4 others` 5 (pyflakes/test/test_dict.py:28); 5. `pyflakes.test.test_doctests.Test.test_offsetInDoctests and 4 others` 5 (pyflakes/test/test_doctests.py:272); 6. `pyflakes.test.test_imports.TestImportationObject.test_import_basic and 3 others` 4 (pyflakes/test/test_imports.py:16); 7. `pyflakes.test.test_other.Test.test_redefinedInGenerator and 3 others` 4 (pyflakes/test/test_other.py:24); 8. `pyflakes.test.test_other.TestUnusedAssignment.test_ifexp and 3 others` 4 (pyflakes/test/test_other.py:1430); 9. `pyflakes.test.test_api.CheckTests.test_missingTrailingNewline and 2 others` 3 (pyflakes/test/test_api.py:364); 10. `pyflakes.test.test_api.TestReporter.test_syntaxError and 2 others` 3 (pyflakes/test/test_api.py:232)
- `static.complexity.cyclomatic` per symbol, highest worst, 774 measured: 1. `pyflakes.checker.Checker._handle_percent_format` 34 (pyflakes/checker.py:1622); 2. `pyflakes.checker.Checker.handleNodeLoad` 33 (pyflakes/checker.py:1018); 3. `pyflakes.checker.Checker.CALL` 30 (pyflakes/checker.py:1487); 4. `pyflakes.checker.Checker.checkDeadScopes` 26 (pyflakes/checker.py:819); 5. `pyflakes.checker.Checker.addBinding` 25 (pyflakes/checker.py:933); 6. `pyflakes.checker.Checker._handle_string_dot_format` 20 (pyflakes/checker.py:1372); 7. `pyflakes.checker.Checker.handleNodeStore` 18 (pyflakes/checker.py:1115); 8. `pyflakes.checker.Checker.IMPORTFROM` 17 (pyflakes/checker.py:2137); 9. `pyflakes.checker.Checker.DICT` 13 (pyflakes/checker.py:1807); 10. `pyflakes.checker.Checker.LAMBDA` 11 (pyflakes/checker.py:1981)
- `static.complexity.cognitive` per symbol, highest worst, 774 measured: 1. `pyflakes.checker.Checker.checkDeadScopes` 68 (pyflakes/checker.py:819); 2. `pyflakes.checker.Checker.handleNodeLoad` 60 (pyflakes/checker.py:1018); 3. `pyflakes.checker.Checker._handle_percent_format` 56 (pyflakes/checker.py:1622); 4. `pyflakes.checker.Checker.CALL` 52 (pyflakes/checker.py:1487); 5. `pyflakes.checker.Checker._handle_string_dot_format` 39 (pyflakes/checker.py:1372); 6. `pyflakes.checker.Checker.addBinding` 39 (pyflakes/checker.py:933); 7. `pyflakes.checker.Checker.IMPORTFROM` 29 (pyflakes/checker.py:2137); 8. `pyflakes.checker.Checker.handleNodeStore` 23 (pyflakes/checker.py:1115); 9. `pyflakes.checker.Checker.DICT` 22 (pyflakes/checker.py:1807); 10. `pyflakes.checker.parse_percent_format._parse_inner` 18 (pyflakes/checker.py:110)
- `static.crap.crap` per symbol, highest worst, 180 measured: 1. `pyflakes.checker.Checker._handle_percent_format` 34 (pyflakes/checker.py:1622); 2. `pyflakes.checker.Checker.handleNodeLoad` 33.0073 (pyflakes/checker.py:1018); 3. `pyflakes.checker.Checker.CALL` 30.0158 (pyflakes/checker.py:1487); 4. `pyflakes.checker.Checker.checkDeadScopes` 26 (pyflakes/checker.py:819); 5. `pyflakes.checker.Checker.addBinding` 25 (pyflakes/checker.py:933); 6. `pyflakes.checker.Checker._handle_string_dot_format` 20 (pyflakes/checker.py:1372); 7. `pyflakes.checker.Checker._in_try` 20 (pyflakes/checker.py:2109); 8. `pyflakes.checker.Checker.IMPORTFROM` 18.905 (pyflakes/checker.py:2137); 9. `pyflakes.checker.Checker.handleNodeStore` 18 (pyflakes/checker.py:1115); 10. `pyflakes.checker.Checker.DICT` 13 (pyflakes/checker.py:1807)
- `static.deps.distance` per module, highest worst, 17 measured: 1. `pyflakes/messages.py` 1 (pyflakes/messages.py:1); 2. `pyflakes/reporter.py` 1 (pyflakes/reporter.py:1); 3. `pyflakes/test/harness.py` 0.929 (pyflakes/test/harness.py:1); 4. `pyflakes/checker.py` 0.889 (pyflakes/checker.py:1); 5. `pyflakes/test/test_other.py` 0.333 (pyflakes/test/test_other.py:1); 6. `pyflakes/test/test_imports.py` 0.25 (pyflakes/test/test_imports.py:1); 7. `pyflakes/test/test_undefined_names.py` 0.25 (pyflakes/test/test_undefined_names.py:1); 8. `pyflakes/test/test_api.py` 0 (pyflakes/test/test_api.py:1); 9. `pyflakes/test/test_builtin.py` 0 (pyflakes/test/test_builtin.py:1); 10. `pyflakes/test/test_code_segment.py` 0 (pyflakes/test/test_code_segment.py:1)
- `static.deps.coupling` per module, highest worst, 22 measured: 1. `pyflakes/test/harness.py` 14 (pyflakes/test/harness.py:1); 2. `pyflakes/messages.py` 13 (pyflakes/messages.py:1); 3. `pyflakes/checker.py` 9 (pyflakes/checker.py:1); 4. `pyflakes/api.py` 6 (pyflakes/api.py:1); 5. `pyflakes/test/test_api.py` 6 (pyflakes/test/test_api.py:1); 6. `pyflakes/test/test_doctests.py` 6 (pyflakes/test/test_doctests.py:1); 7. `pyflakes/test/test_imports.py` 4 (pyflakes/test/test_imports.py:1); 8. `pyflakes/test/test_undefined_names.py` 4 (pyflakes/test/test_undefined_names.py:1); 9. `pyflakes/test/test_code_segment.py` 3 (pyflakes/test/test_code_segment.py:1); 10. `pyflakes/test/test_custom_builtins.py` 3 (pyflakes/test/test_custom_builtins.py:1)
- `static.deps.cycle_size` per group, highest worst, 0 measured: none
- `tests.coverage.uncovered_lines` per symbol, highest worst, 180 measured: 1. `pyflakes.checker.Checker.IMPORTFROM` 6 (pyflakes/checker.py:2137); 2. `pyflakes.checker.Checker.TEMPLATESTR` 6 (pyflakes/checker.py:1793); 3. `pyflakes.checker.Checker._in_try` 6 (pyflakes/checker.py:2109); 4. `pyflakes.checker.Checker.IMPORT` 4 (pyflakes/checker.py:2117); 5. `pyflakes.checker.Checker.CALL` 2 (pyflakes/checker.py:1487); 6. `pyflakes.messages.EagerUseOfLazyImport.__init__` 2 (pyflakes/messages.py:367); 7. `pyflakes.messages.LazyImportStarNotPermitted.__init__` 2 (pyflakes/messages.py:375); 8. `pyflakes.checker.Checker.handleNodeLoad` 1 (pyflakes/checker.py:1018); 9. `pyflakes.api._get_version` 0 (pyflakes/api.py:158); 10. `pyflakes.api.check` 0 (pyflakes/api.py:18)
- `tests.coverage.line_coverage` per symbol, lowest worst, 180 measured: 1. `pyflakes.checker.Checker.TEMPLATESTR` 0 (pyflakes/checker.py:1793); 2. `pyflakes.checker.Checker._in_try` 0 (pyflakes/checker.py:2109); 3. `pyflakes.messages.EagerUseOfLazyImport.__init__` 0 (pyflakes/messages.py:367); 4. `pyflakes.messages.LazyImportStarNotPermitted.__init__` 0 (pyflakes/messages.py:375); 5. `pyflakes.checker.Checker.IMPORT` 0.692308 (pyflakes/checker.py:2117); 6. `pyflakes.checker.Checker.IMPORTFROM` 0.8125 (pyflakes/checker.py:2137); 7. `pyflakes.checker.Checker.CALL` 0.974026 (pyflakes/checker.py:1487); 8. `pyflakes.checker.Checker.handleNodeLoad` 0.981132 (pyflakes/checker.py:1018); 9. `pyflakes.api._get_version` 1 (pyflakes/api.py:158); 10. `pyflakes.api.check` 1 (pyflakes/api.py:18)
- `tests.mutation.survivors` per symbol, highest worst, 81 measured: 1. `pyflakes.checker.Checker.CALL` 11 (pyflakes/checker.py:1487); 2. `pyflakes.checker.Checker.TUPLE` 5 (pyflakes/checker.py:2084); 3. `pyflakes.checker.parse_percent_format._parse_inner` 5 (pyflakes/checker.py:110); 4. `pyflakes.checker.Checker.handleNodeLoad` 4 (pyflakes/checker.py:1018); 5. `pyflakes.checker.Checker.NAME` 2 (pyflakes/checker.py:1909); 6. `pyflakes.checker.Checker.getCommonAncestor` 2 (pyflakes/checker.py:897); 7. `pyflakes.messages.Message.__str__` 2 (pyflakes/messages.py:15); 8. `pyflakes.api.main` 1 (pyflakes/api.py:166); 9. `pyflakes.checker.Checker.CLASSDEF` 1 (pyflakes/checker.py:2030); 10. `pyflakes.checker.Checker.IMPORT` 1 (pyflakes/checker.py:2117)
- `tests.mutation.mutation_score` per symbol, lowest worst, 81 measured: 1. `pyflakes.checker.Checker.CLASSDEF` 0 (pyflakes/checker.py:2030); 2. `pyflakes.checker.Checker.TEMPLATESTR` 0 (pyflakes/checker.py:1793); 3. `pyflakes.checker.Checker._in_try` 0 (pyflakes/checker.py:2109); 4. `pyflakes.checker.Checker._type_param_scope` 0 (pyflakes/checker.py:2272); 5. `pyflakes.checker.ExportBinding.__init__` 0 (pyflakes/checker.py:488); 6. `pyflakes.checker._is_typing_helper._bare_name_is_attr` 0 (pyflakes/checker.py:624); 7. `pyflakes.checker._is_typing_helper._module_scope_is_typing` 0 (pyflakes/checker.py:635); 8. `pyflakes.messages.Message.__str__` 0 (pyflakes/messages.py:15); 9. `setup.get_version` 0 (setup.py:20); 10. `pyflakes.checker.Checker.getCommonAncestor` 0.333333 (pyflakes/checker.py:897)
- `tests.mutation.tests` per symbol, lowest worst, 81 measured: 1. `pyflakes.checker.Checker.TEMPLATESTR` 0 (pyflakes/checker.py:1793); 2. `pyflakes.checker.Checker._in_try` 0 (pyflakes/checker.py:2109); 3. `setup.get_version` 0 (setup.py:20); 4. `pyflakes.api.check` 795 (pyflakes/api.py:18); 5. `pyflakes.api.checkPath` 795 (pyflakes/api.py:54); 6. `pyflakes.api.checkRecursive` 795 (pyflakes/api.py:115); 7. `pyflakes.api.isPythonFile` 795 (pyflakes/api.py:74); 8. `pyflakes.api.main` 795 (pyflakes/api.py:166); 9. `pyflakes.checker.Annotation.redefines` 795 (pyflakes/checker.py:460); 10. `pyflakes.checker.Binding.__init__` 795 (pyflakes/checker.py:222)
- `tests.quality.mock_density` per symbol, highest worst, 563 measured: 1. `pyflakes.test.test_custom_builtins.TestCustomBuiltins.test_custom_builtins_from_env` 33 (pyflakes/test/test_custom_builtins.py:23); 2. `pyflakes.test.test_api.CheckTests.test_CRLFLineEndings` 0 (pyflakes/test/test_api.py:544); 3. `pyflakes.test.test_api.CheckTests.test_checkPathNonExisting` 0 (pyflakes/test/test_api.py:373); 4. `pyflakes.test.test_api.CheckTests.test_checkRecursive` 0 (pyflakes/test/test_api.py:585); 5. `pyflakes.test.test_api.CheckTests.test_check_unknown_error` 0 (pyflakes/test/test_api.py:628); 6. `pyflakes.test.test_api.CheckTests.test_encodedFileUTF8` 0 (pyflakes/test/test_api.py:532); 7. `pyflakes.test.test_api.CheckTests.test_eofSyntaxError` 0 (pyflakes/test/test_api.py:409); 8. `pyflakes.test.test_api.CheckTests.test_eofSyntaxErrorWithTab` 0 (pyflakes/test/test_api.py:423); 9. `pyflakes.test.test_api.CheckTests.test_invalidEscape` 0 (pyflakes/test/test_api.py:479); 10. `pyflakes.test.test_api.CheckTests.test_legacyScript` 0 (pyflakes/test/test_api.py:360)
- Targets, protect first (70 recommended, top 10): 1. `pyflakes.checker.Checker.TEMPLATESTR` protect (mutation score 0, coverage 0) (pyflakes/checker.py:1793); 2. `pyflakes.checker.Checker._in_try` protect (mutation score 0, coverage 0) (pyflakes/checker.py:2109); 3. `pyflakes.messages.EagerUseOfLazyImport.__init__` protect (mutation score None, coverage 0) (pyflakes/messages.py:367); 4. `pyflakes.messages.LazyImportStarNotPermitted.__init__` protect (mutation score None, coverage 0) (pyflakes/messages.py:375); 5. `pyflakes.messages.Message.__str__` protect (mutation score 0, coverage 1) (pyflakes/messages.py:15); 6. `pyflakes.checker.Checker.CLASSDEF` protect (mutation score 0, coverage 1) (pyflakes/checker.py:2030); 7. `pyflakes.checker.Checker._type_param_scope` protect (mutation score 0, coverage 1) (pyflakes/checker.py:2272); 8. `pyflakes.checker.ExportBinding.__init__` protect (mutation score 0, coverage 1) (pyflakes/checker.py:488); 9. `pyflakes.checker._is_typing_helper._bare_name_is_attr` protect (mutation score 0, coverage 1) (pyflakes/checker.py:624); 10. `pyflakes.checker._is_typing_helper._module_scope_is_typing` protect (mutation score 0, coverage 1) (pyflakes/checker.py:635)

## Check

Final run at `8b6e07b`: status `pass`, exit 0 (no gate is configured), 614 findings (526 fail, 40 warn, 48 info), scope 2 files and 171 symbols. Every count is the same as the first run's at `8e8bef5`; `tests.mutation` took 47.4 s there (294 mutants, two slots) and reused its outcomes in the final run.

| Check | Status | Findings | Score | Time |
|---|---|---|---|---|
| `conc.linearizability` | skipped (unconfigured) | 0 | — | 0.0 s |
| `conc.race` | skipped (unsupported) | 0 | — | 0.0 s |
| `gen.fuzz` | skipped (unavailable) | 0 | — | 0.0 s |
| `gen.metamorphic` | skipped (unconfigured) | 0 | — | 0.0 s |
| `gen.property` | pass | property.missing 10 | 0.0  (n 0) | 0.1 s |
| `meta.oracle-independence` | skipped (needs-range) | 0 | — | 0.0 s |
| `meta.traceability` | skipped (needs-range) | 0 | — | 0.0 s |
| `perf.bench` | pass | 0 | 0.0  (n 0) | 0.7 s |
| `perf.footprint` | pass | 0 | 3.0  (n 3) | 0.0 s |
| `spec.contracts` | pass | 0 | — | 0.0 s |
| `static.analyze` | fail | analyze.mypy.arg-type 4, analyze.mypy.assignment 42, analyze.mypy.attr-defined 3, analyze.mypy.misc 1, analyze.mypy.no-untyped-call 267, analyze.mypy.no-untyped-def 159, analyze.mypy.type-arg 1, analyze.mypy.union-attr 2, analyze.mypy.var-annotated 2, analyze.ruff.I001 1, analyze.ruff.RUF012 1, analyze.ruff.SIM102 2, analyze.ruff.SIM114 3, analyze.ruff.SIM201 1, analyze.ruff.TRY004 1, analyze.ruff.UP032 3 | 493.0  (n 2) | 0.1 s |
| `static.api-surface` | skipped (needs-range) | 0 | — | 0.0 s |
| `static.clones` | fail | clone.pair 3 | 3.0  (n 97) | 0.0 s |
| `static.complexity` | fail | complexity.high 11, function.long 6, nesting.deep 1 | 34.0  (n 171) | 0.0 s |
| `static.crap` | fail | crap.high 3, crap.watch 33 | 34.0  (n 167) | 0.0 s |
| `static.dead-code` | pass | 0 | 0.0  (n 171) | 0.0 s |
| `static.deps` | pass | 0 | 0.0  (n 2) | 0.0 s |
| `tests.assertion-density` | pass | 0 | 0.0  (n 0) | 0.0 s |
| `tests.coverage` | fail | coverage.uncovered-symbol 4 | 0.9743362831858408  (n 1130) | 0.0 s |
| `tests.flaky` | pass | 0 | 0.0  (n 0) | 1.4 s |
| `tests.mutation` | fail | mutant.survived 45, mutant.timeout 1, mutant.uncovered 4 | 0.8333333333333334  (n 294) | 0.3 s |
| `tests.quality` | pass | 0 | 0.0  (n 0) | 0.0 s |
| `tests.weakening` | skipped (needs-range) | 0 | — | 0.0 s |
| `gen.differential` | skipped (unconfigured) | 0 | — | 0.0 s |

## Triage

Every finding of the final check, every finding of the final survey outside the check's two files, and every finding an earlier run reported that a fix removed (verdict `false`, marked by the fix); rows marked "(survey)" come from the survey. Verification per ADR-138 §4, all from outside the clone except the replays: every `static.analyze` row matched one diagnostic of an independent `mypy --strict --output json` and `ruff check --output-format json` over the same files (2,038 of 2,038, by file, line, column, and rule); every `mutant.survived` and the `mutant.timeout` were replayed (`python -m unittest` in a copy of the clone with the mutation applied, under `scripts/heavy.sh`: 47 green, one past 240 s); every `static.crap`, `coverage.uncovered-symbol`, and `mutant.uncovered` row was recomputed from an independent `coverage run --rcfile=/dev/null -m unittest` (the `def` line excluded, as it runs when the module loads); every cyclomatic count and function length was recomputed with `ast`; the cognitive counts that decide a finding (`_parse_inner`, 18), the nesting rows, and every `tests.quality`, `tests.assertion-density`, `static.dead-code`, and `static.clones` row were read by hand.

**`gen.property`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_71ed8d` | `property.missing` | `pyflakes/api.py:18` (survey) | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_cf8e6d` | `property.missing` | `pyflakes/api.py:54` (survey) | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_484648` | `property.missing` | `pyflakes/api.py:96` (survey) | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_2c7c1f` | `property.missing` | `pyflakes/api.py:115` (survey) | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_82fbcb` | `property.missing` | `pyflakes/api.py:166` (survey) | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_d63e0d` | `property.missing` | `pyflakes/checker.py:48` | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_61ea75` | `property.missing` | `pyflakes/checker.py:104` | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_3ab55b` | `property.missing` | `pyflakes/checker.py:168` | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_09aa40` | `property.missing` | `pyflakes/checker.py:191` | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_a0bc74` | `property.missing` | `pyflakes/checker.py:195` | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_545620` | `property.missing` | `pyflakes/checker.py:206` | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_e882fb` | `property.missing` | `pyflakes/checker.py:601` | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_d11716` | `property.missing` | `pyflakes/checker.py:679` | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_df60f5` | `property.missing` | `pyflakes/checker.py:696` | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_29ec52` | `property.missing` | `pyflakes/checker.py:704` | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |
| `fnd_5b2467` | `property.missing` | `setup.py:20` (survey) | true | No test of the repository uses hypothesis, so the function has no property test. | none needed |

**`static.analyze`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_a9e7d0` | `analyze.mypy.no-untyped-call` | `pyflakes/__main__.py:5` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d4d57f` | `analyze.ruff.I001` | `pyflakes/api.py:4` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_ead600` | `analyze.mypy.no-untyped-def` | `pyflakes/api.py:18` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_efd07b` | `analyze.mypy.no-untyped-call` | `pyflakes/api.py:36` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_08301d` | `analyze.ruff.BLE001` | `pyflakes/api.py:43` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_69bdc6` | `analyze.mypy.no-untyped-call` | `pyflakes/api.py:47` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a0e48b` | `analyze.mypy.no-untyped-def` | `pyflakes/api.py:54` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_03b4bd` | `analyze.mypy.no-untyped-call` | `pyflakes/api.py:64` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_62d480` | `analyze.mypy.no-untyped-call` | `pyflakes/api.py:71` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a98b6d` | `analyze.mypy.no-untyped-def` | `pyflakes/api.py:74` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_106e19` | `analyze.mypy.no-untyped-def` | `pyflakes/api.py:96` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d98bb8` | `analyze.mypy.no-untyped-call` | `pyflakes/api.py:109` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7edcf0` | `analyze.mypy.no-untyped-def` | `pyflakes/api.py:115` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b2e5a5` | `analyze.mypy.no-untyped-call` | `pyflakes/api.py:126` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cdb929` | `analyze.mypy.no-untyped-call` | `pyflakes/api.py:127` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7dce29` | `analyze.mypy.no-untyped-def` | `pyflakes/api.py:131` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_805e05` | `analyze.mypy.no-untyped-def` | `pyflakes/api.py:147` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b25bbf` | `analyze.mypy.no-untyped-def` | `pyflakes/api.py:158` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_08f247` | `analyze.ruff.UP031` | `pyflakes/api.py:162` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_fa16d4` | `analyze.mypy.no-untyped-def` | `pyflakes/api.py:166` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a7fa5d` | `analyze.mypy.no-untyped-call` | `pyflakes/api.py:171` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9cad78` | `analyze.mypy.no-untyped-call` | `pyflakes/api.py:172` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_197ea9` | `analyze.mypy.no-untyped-call` | `pyflakes/api.py:176` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6b4740` | `analyze.mypy.no-untyped-call` | `pyflakes/api.py:180` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f5c46e` | `analyze.mypy.no-untyped-call` | `pyflakes/api.py:182` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_209036` | `analyze.mypy.no-untyped-call` | `pyflakes/api.py:184` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_578951` | `analyze.ruff.I001` | `pyflakes/checker.py:7` | true | An independent ruff run reports it here. | fix or configure |
| `fnd_a7a1d5` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:43` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9c3d4d` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:48` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_23f09f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:70` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3a9cf2` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:74` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7da238` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:79` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c3c99f` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:98` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ded1ed` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:104` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_86887b` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:110` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f639c1` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:135` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_506121` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:139` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f1d5c4` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:143` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3c2d6b` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:148` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_22dbd5` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:165` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_aa8c14` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:168` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_233dd4` | `analyze.mypy.arg-type` | `pyflakes/checker.py:192` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_de8a54` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:195` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3faa4a` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:199` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_14bcf6` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:201` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1150d4` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:206` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_33e86f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:207` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5c6631` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:222` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_29f705` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:227` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_99fe8c` | `analyze.ruff.UP032` | `pyflakes/checker.py:228` | true | An independent ruff run reports it here. | fix or configure |
| `fnd_0fba9f` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:235` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ef2540` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:243` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4ec194` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:245` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0d5f4b` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:253` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2742d7` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:254` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a4b4fa` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:256` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c203a2` | `analyze.ruff.UP032` | `pyflakes/checker.py:257` | true | An independent ruff run reports it here. | fix or configure |
| `fnd_7e57b4` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:276` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f9c68d` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:279` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_52152d` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:285` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8c424f` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:298` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_14f254` | `analyze.mypy.var-annotated` | `pyflakes/checker.py:300` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_07764e` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:302` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3ad45b` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:304` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_583d62` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:310` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dd6c68` | `analyze.ruff.SIM201` | `pyflakes/checker.py:312` | true | An independent ruff run reports it here. | fix or configure |
| `fnd_db4d53` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:320` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c3cf97` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:323` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4fa9d6` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:328` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ce6deb` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:350` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_501a69` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:357` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_467e16` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:360` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_90967b` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:363` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ec51d7` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:367` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4df71e` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:373` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8cefbc` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:385` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d592b3` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:389` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a6aed2` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:397` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_57c97e` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:405` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5fbf45` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:409` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a6ab1c` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:424` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_986222` | `analyze.mypy.assignment` | `pyflakes/checker.py:426` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_54007e` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:460` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_55c4e5` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:488` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3e4247` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:494` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_94cc65` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:500` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d75809` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:507` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_072d1c` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:513` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_649edf` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:519` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_37ebdb` | `analyze.mypy.type-arg` | `pyflakes/checker.py:522` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_40abb4` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:525` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3233bc` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:531` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9cbcf5` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:549` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bdad1d` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:554` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_86077f` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:566` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2b8eb0` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:601` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3f84a5` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:614` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7be851` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:624` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_87cede` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:635` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_08e9ee` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:648` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3b913c` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:652` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8a6357` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:658` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_26ce7d` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:666` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_48febb` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:669` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ae9409` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:676` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_17bfe7` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:679` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2302cc` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:683` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_193e24` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:696` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a24137` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:698` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4701a3` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:704` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0ed019` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:706` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_eaf6ae` | `analyze.ruff.RUF012` | `pyflakes/checker.py:715` | true | An independent ruff run reports it here. | fix or configure |
| `fnd_34ebb5` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:731` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d47d5f` | `analyze.mypy.var-annotated` | `pyflakes/checker.py:736` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f1b03f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:752` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4a0f1f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:752` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_41bce6` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:753` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ece16a` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:754` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5bcc2c` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:756` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_711930` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:758` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7a1747` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:769` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0a55d0` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:779` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_847c45` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:784` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_431a3f` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:791` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1616f4` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:797` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_02782d` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:802` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5b7aa2` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:808` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f7b493` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:812` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9d48e7` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:819` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_27bda1` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:827` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_807229` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:834` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_61f067` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:835` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ea6626` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:836` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b06b28` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:837` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f76e7a` | `analyze.mypy.assignment` | `pyflakes/checker.py:850` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_609053` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:857` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_853bc7` | `analyze.mypy.assignment` | `pyflakes/checker.py:868` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1432cb` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:870` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8e6d01` | `analyze.ruff.SIM102` | `pyflakes/checker.py:875` | true | An independent ruff run reports it here. | fix or configure |
| `fnd_4e4370` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:877` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_279b01` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:882` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e51811` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:887` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3093fa` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:890` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_178b36` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:897` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f9da0d` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:907` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5f5f30` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:909` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6adade` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:910` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_055e6b` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:916` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_338882` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:918` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f9e609` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:922` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f7fa5b` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:924` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d6986b` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:925` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4612bf` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:928` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_540392` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:929` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3a1113` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:933` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_53882c` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:947` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e4b6a6` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:949` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b4abbd` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:951` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bdc1f1` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:958` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6f3887` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:960` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6ca6c8` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:992` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_13491c` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1006` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ac0f27` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1008` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_925a35` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1018` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6623de` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1019` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0fc6a5` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1054` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5d7b27` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1063` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_00a1fc` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1072` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ad2d98` | `analyze.mypy.assignment` | `pyflakes/checker.py:1096` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_65274e` | `analyze.mypy.assignment` | `pyflakes/checker.py:1100` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a65fb4` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1101` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1615fe` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1113` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c4c449` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1115` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_401113` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1116` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_efcd83` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1131` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cb805d` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1135` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fecc1f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1137` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_942657` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1140` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d47c94` | `analyze.mypy.assignment` | `pyflakes/checker.py:1141` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f05c5f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1141` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8354a4` | `analyze.mypy.assignment` | `pyflakes/checker.py:1150` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_88a077` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1150` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_53228e` | `analyze.mypy.assignment` | `pyflakes/checker.py:1152` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_42c4e4` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1152` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a40ad6` | `analyze.mypy.assignment` | `pyflakes/checker.py:1154` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7508b4` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1154` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ae54d6` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1155` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_41c453` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1157` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_97bde4` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1159` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cbae7d` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1170` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_49f926` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1181` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e78254` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1184` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5b02b6` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1192` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ff0ed6` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1201` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ebcde3` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1202` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fda50d` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1203` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0c9f40` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1205` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4c05df` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1212` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_371be1` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1223` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3901b2` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1233` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9d81a2` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1243` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bda9ec` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1250` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6ac975` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1255` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_36388f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1257` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fc090a` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1272` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9d53b8` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1272` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_70428d` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1279` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_534fbb` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1283` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bfb1e2` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1288` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f15307` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1292` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2406dc` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1297` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b8bef0` | `analyze.mypy.attr-defined` | `pyflakes/checker.py:1306` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1ba374` | `analyze.mypy.attr-defined` | `pyflakes/checker.py:1307` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0ee53b` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1309` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b5b46c` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1311` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c7edd7` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1312` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7daa71` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1313` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cdc4a4` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1316` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ed52d6` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1318` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4bbc43` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1320` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_105799` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1322` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3cc6ef` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1330` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a669b0` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1331` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b6d1f2` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1332` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e50c18` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1340` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_80f28f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1342` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_78d776` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1344` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1d4f42` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1346` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f9a8c2` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1355` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_222224` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1358` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dc151c` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1362` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_909925` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1364` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f50887` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1366` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_010526` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1368` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_511319` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1370` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e47cec` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1372` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_927eaf` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1376` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_88337f` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1385` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c9e7b9` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1409` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b65c14` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1425` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_85ac10` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1433` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5dc226` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1439` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f0d0db` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1445` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_14a6b8` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1469` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_94008b` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1475` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0457d2` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1481` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_29ecce` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1487` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1d380e` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1494` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_358c78` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1498` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c277c6` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1506` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c0685a` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1512` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_68960f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1526` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dfae96` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1534` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_57d056` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1549` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cbf3d3` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1550` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a89c26` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1563` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a0e7ec` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1576` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2d0761` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1595` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c8de04` | `analyze.mypy.arg-type` | `pyflakes/checker.py:1606` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_15e19d` | `analyze.mypy.arg-type` | `pyflakes/checker.py:1607` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3beb4c` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1620` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_edd66a` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1622` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c3e5ec` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1624` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_67c393` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1626` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_838fad` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1645` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ed6483` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1657` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4d8b95` | `analyze.ruff.SIM114` | `pyflakes/checker.py:1664` | true | An independent ruff run reports it here. | fix or configure |
| `fnd_e84c09` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1665` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e4676f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1671` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d28fb5` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1692` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_844379` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1699` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0d1e18` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1709` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3d81ca` | `analyze.mypy.union-attr` | `pyflakes/checker.py:1712` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5b0169` | `analyze.mypy.union-attr` | `pyflakes/checker.py:1712` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bf10e6` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1716` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_88a8db` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1722` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_77e4d0` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1728` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_406f56` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1734` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2d680d` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1735` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3fad5e` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1737` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d29ba5` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1746` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5f6329` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1760` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ba9f35` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1761` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2bb85f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1766` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_67cf0e` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1768` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7024a2` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1769` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_88c43b` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1771` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5116ac` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1778` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_06be20` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1785` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6b2ca5` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1789` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4c3762` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1793` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_029dd3` | `analyze.mypy.attr-defined` | `pyflakes/checker.py:1794` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e7cb08` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1795` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bf9371` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1801` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bfe6dc` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1807` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fc17d8` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1812` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f917ce` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1825` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cb1c26` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1832` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_04f26b` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1836` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_111214` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1843` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7fc78b` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1844` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_32c90f` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1846` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c7549b` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1848` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1e1fe3` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1849` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6c0d7d` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1853` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0b1df2` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1855` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_983320` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1856` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_90ef3d` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1858` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4e22e3` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1862` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f2c690` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1870` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_513ab2` | `analyze.mypy.misc` | `pyflakes/checker.py:1878` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ed2add` | `analyze.mypy.assignment` | `pyflakes/checker.py:1884` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_73f5fa` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1892` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_caa946` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1894` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_76f834` | `analyze.mypy.assignment` | `pyflakes/checker.py:1899` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e95a07` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1902` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ae689b` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1904` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_387945` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1905` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d86ca5` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1909` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_860b13` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1915` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_763a7b` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1915` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_77dc11` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1921` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6fbb8f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1923` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d5c30b` | `analyze.ruff.TRY004` | `pyflakes/checker.py:1926` | true | An independent ruff run reports it here. | fix or configure |
| `fnd_46e5fa` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1928` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_77542d` | `analyze.ruff.SIM102` | `pyflakes/checker.py:1935` | true | An independent ruff run reports it here. | fix or configure |
| `fnd_223519` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1942` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_88d068` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1944` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6a92f5` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1948` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_63c791` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1950` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1ee62e` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1953` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_791668` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1955` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_10dec9` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1957` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_81064a` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1960` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ccd254` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1964` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_90aa3e` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1966` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7d14c6` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1969` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_95c4b2` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1971` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c76e97` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1971` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_16e295` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1975` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6a1267` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1977` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f8402b` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:1977` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3a6740` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:1981` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1b52ac` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2007` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4367a6` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2013` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cdfba2` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2015` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_18581a` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2017` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1a9b41` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2022` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_af3430` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2024` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a1296c` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2025` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a39fad` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2027` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7a6c17` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2028` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_99e9ed` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2028` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d140f1` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2030` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e7fd7f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2037` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_148ef6` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2041` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ed7bf5` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2043` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5d8333` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2048` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_abf3c7` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2050` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_eb4aa8` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2050` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3d7cad` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2052` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a9ce63` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2054` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7b1a06` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2054` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_000e5d` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2056` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8f7f32` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2057` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3c0e96` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2058` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3b1077` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2062` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4bdae4` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2063` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_58e1c2` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2064` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b4100f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2065` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b9628f` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2067` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2cead4` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2072` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c20f09` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2074` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e8b19e` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2076` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7b425c` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2077` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_afc633` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2079` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_22c394` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2081` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7503db` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2082` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_11d1a1` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2084` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4a4a72` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2097` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a39660` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2104` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a3e5b2` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2105` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6aa474` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2109` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6e48b7` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2111` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4cc266` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2117` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9b2021` | `analyze.ruff.SIM114` | `pyflakes/checker.py:2119` | true | An independent ruff run reports it here. | fix or configure |
| `fnd_fc64e7` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2120` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_511c41` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2122` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9e1fe8` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2123` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cb532b` | `analyze.mypy.assignment` | `pyflakes/checker.py:2134` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8eed92` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2135` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7bb8bb` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2137` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bf4c7a` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2140` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f1c1eb` | `analyze.ruff.SIM114` | `pyflakes/checker.py:2145` | true | An independent ruff run reports it here. | fix or configure |
| `fnd_394493` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2146` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_410486` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2148` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_349d00` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2149` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_50d401` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2157` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_140fdb` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2159` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_40519a` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2165` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_92b5db` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2168` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_810795` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2172` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_442ce3` | `analyze.mypy.assignment` | `pyflakes/checker.py:2173` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c36ec4` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2173` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4dab74` | `analyze.mypy.assignment` | `pyflakes/checker.py:2175` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ba5e63` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2179` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6fd75b` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2181` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fe99fb` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2187` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_23fca9` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2189` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_47973d` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2192` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_486bc1` | `analyze.mypy.arg-type` | `pyflakes/checker.py:2194` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8f1f2e` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2196` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bdebce` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2199` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_993580` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2203` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_35c4ba` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2205` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d7f4be` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2211` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_95af66` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2224` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7c76fa` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2225` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1ba20f` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2242` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_677aba` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2248` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_048173` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2257` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_17970c` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2260` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a1318d` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2265` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8832f1` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2266` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9d50bb` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2267` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_86c790` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2272` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_da4857` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2277` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_401b7b` | `analyze.mypy.no-untyped-def` | `pyflakes/checker.py:2280` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7315e5` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2281` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9675c2` | `analyze.mypy.no-untyped-call` | `pyflakes/checker.py:2282` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7ff10d` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:10` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d10fc6` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:15` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_38cc8d` | `analyze.ruff.UP032` | `pyflakes/messages.py:16` | true | An independent ruff run reports it here. | fix or configure |
| `fnd_fbafbb` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:23` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2c24b6` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:24` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1d5b44` | `analyze.mypy.assignment` | `pyflakes/messages.py:25` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_09c920` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:31` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7d62f7` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:32` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_66bdc6` | `analyze.mypy.assignment` | `pyflakes/messages.py:33` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_32220b` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:39` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d8c321` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:40` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e38855` | `analyze.mypy.assignment` | `pyflakes/messages.py:41` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_334b57` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:47` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_78bff5` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:48` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1239da` | `analyze.mypy.assignment` | `pyflakes/messages.py:49` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1e36df` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:55` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cf21fe` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:56` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_678112` | `analyze.mypy.assignment` | `pyflakes/messages.py:57` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_134815` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:63` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_68f8f2` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:64` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_092dcf` | `analyze.mypy.assignment` | `pyflakes/messages.py:65` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_35da83` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:71` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fa9150` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:72` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_405bc6` | `analyze.mypy.assignment` | `pyflakes/messages.py:73` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_918d3b` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:79` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e71e99` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:80` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f0b30d` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:88` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_19cc5e` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:89` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9ad777` | `analyze.mypy.assignment` | `pyflakes/messages.py:90` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c79f1e` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:99` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fc1a08` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:100` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1e8e80` | `analyze.mypy.assignment` | `pyflakes/messages.py:106` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_706802` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:112` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8ef8e7` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:113` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fbcc06` | `analyze.mypy.assignment` | `pyflakes/messages.py:114` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e6c2da` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:120` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4161a2` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:121` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ed1fa3` | `analyze.mypy.assignment` | `pyflakes/messages.py:122` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2722e5` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:128` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e78ec1` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:129` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8a9e51` | `analyze.mypy.assignment` | `pyflakes/messages.py:130` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c9427b` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:141` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e66a9a` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:142` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_239d32` | `analyze.mypy.assignment` | `pyflakes/messages.py:143` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_43a947` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:153` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4a7a66` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:154` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_db5995` | `analyze.mypy.assignment` | `pyflakes/messages.py:155` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0ae90b` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:165` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1b04ee` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:166` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fb1708` | `analyze.mypy.assignment` | `pyflakes/messages.py:167` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9b87af` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:174` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_45eb66` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:175` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8e1210` | `analyze.mypy.assignment` | `pyflakes/messages.py:176` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8ab949` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:247` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7a7d43` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:248` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_19e3b1` | `analyze.mypy.assignment` | `pyflakes/messages.py:249` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_156754` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:275` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_91d206` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:276` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_62c660` | `analyze.mypy.assignment` | `pyflakes/messages.py:277` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7c3d44` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:283` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e4b25b` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:284` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3606f7` | `analyze.mypy.assignment` | `pyflakes/messages.py:285` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a5e8a2` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:291` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_93a8b5` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:292` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a578ca` | `analyze.mypy.assignment` | `pyflakes/messages.py:293` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_61da4c` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:303` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e46ba2` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:304` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b5c3f8` | `analyze.mypy.assignment` | `pyflakes/messages.py:305` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_185718` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:311` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f5dfda` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:312` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d1b32d` | `analyze.mypy.assignment` | `pyflakes/messages.py:313` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a6d787` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:323` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_de0795` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:324` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_137a82` | `analyze.mypy.assignment` | `pyflakes/messages.py:325` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3f5e80` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:331` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_50cc2f` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:332` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0bdbff` | `analyze.mypy.assignment` | `pyflakes/messages.py:333` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_366738` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:339` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e453ea` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:340` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6c4c39` | `analyze.mypy.assignment` | `pyflakes/messages.py:341` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_193edf` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:347` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d1dfa4` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:348` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_325a18` | `analyze.mypy.assignment` | `pyflakes/messages.py:349` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a1c158` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:367` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_09ef76` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:368` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_aeaf8e` | `analyze.mypy.assignment` | `pyflakes/messages.py:369` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_265cc8` | `analyze.mypy.no-untyped-def` | `pyflakes/messages.py:375` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1240ab` | `analyze.mypy.no-untyped-call` | `pyflakes/messages.py:376` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5453ca` | `analyze.mypy.assignment` | `pyflakes/messages.py:377` | true | An independent mypy run reports it here. | fix or configure |
| `fnd_48aaf1` | `analyze.mypy.no-untyped-def` | `pyflakes/reporter.py:14` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b5529b` | `analyze.mypy.no-untyped-def` | `pyflakes/reporter.py:28` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9a69b8` | `analyze.mypy.no-untyped-def` | `pyflakes/reporter.py:39` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f6cf9e` | `analyze.ruff.UP031` | `pyflakes/reporter.py:66` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_fd3718` | `analyze.ruff.UP031` | `pyflakes/reporter.py:69` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_e47bb6` | `analyze.mypy.no-untyped-def` | `pyflakes/reporter.py:78` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_86f70c` | `analyze.mypy.no-untyped-def` | `pyflakes/reporter.py:88` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_eda55c` | `analyze.mypy.no-untyped-call` | `pyflakes/reporter.py:92` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cb6357` | `analyze.mypy.no-untyped-def` | `pyflakes/test/harness.py:17` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a69bfe` | `analyze.mypy.assignment` | `pyflakes/test/harness.py:20` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a74257` | `analyze.mypy.no-untyped-call` | `pyflakes/test/harness.py:22` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_365c5c` | `analyze.mypy.assignment` | `pyflakes/test/harness.py:24` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2955f2` | `analyze.mypy.attr-defined` | `pyflakes/test/harness.py:26` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0becec` | `analyze.ruff.I001` | `pyflakes/test/test_api.py:5` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_ad6106` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:26` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_73b7b7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:41` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e8f9d2` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:52` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_44eea9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:55` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0f51be` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:66` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_75bb67` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:80` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_78f0d3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:88` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f1eccf` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:91` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8d23ef` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:94` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ce4fe5` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:103` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_329bbb` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:106` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d09605` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:109` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9cff3d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:115` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5163f6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:119` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0202ff` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:121` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f74a04` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:126` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dc1275` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:127` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_100e2e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:129` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8dae1a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:133` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_946986` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:134` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_467d81` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:136` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b30315` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:142` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6aa96e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:143` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_be4fae` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:145` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_93aa75` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:146` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_995e6b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:148` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8ef40b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:151` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_797d47` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:160` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_817823` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:191` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4baa39` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:197` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_15743c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:205` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4c743e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:207` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_01d85c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:209` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_19ea56` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:212` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_af80fa` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:217` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e091ab` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:218` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_227bce` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:221` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fbc480` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:223` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4b912d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:224` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b369af` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:232` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8b1dc1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:240` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_50b6de` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:241` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a4c538` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:248` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e1def5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:254` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_735223` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:255` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dbb1ce` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:262` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_625248` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:269` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bae263` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:270` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fee490` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:273` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4e3284` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:284` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_45076c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:285` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c3bbc4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:293` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ac5e89` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:298` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e033ea` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:299` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_73d672` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:302` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4e420b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:308` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f529c8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:309` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c32d37` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:309` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f1205e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:310` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_adc8e0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:320` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e498e8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:334` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4dbe07` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:342` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c092cf` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:346` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a8be0c` | `analyze.mypy.var-annotated` | `pyflakes/test/test_api.py:355` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f852ee` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:356` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_efeceb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:357` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_052283` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:360` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c3d5e8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:364` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6e68fe` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:371` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4b9abb` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:373` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_607e9b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:377` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b9c6a8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:383` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ac751e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:401` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2fb6f7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:409` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f7b73c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:421` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8c8088` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:423` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_02ab1d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:429` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_82d174` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:437` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_79db48` | `analyze.ruff.RUF100` | `pyflakes/test/test_api.py:449` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_8f37cb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:453` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7fcee5` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:461` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1f754e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:471` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_46f0e3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:479` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_99293e` | `analyze.ruff.RUF100` | `pyflakes/test/test_api.py:488` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_057856` | `analyze.ruff.UP031` | `pyflakes/test/test_api.py:495` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_2c0766` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:501` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0762a7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:505` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_40c675` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:515` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_750642` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:521` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f9d9a5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:527` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c6c1f5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:530` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1fa135` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:530` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_33220c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:532` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_501762` | `analyze.ruff.UP031` | `pyflakes/test/test_api.py:537` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_ec71b1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:542` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_703599` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:544` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_edca94` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:549` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_809374` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:551` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_69fb44` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:567` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6e7e5c` | `analyze.ruff.RUF100` | `pyflakes/test/test_api.py:569` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_d7f638` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:571` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_409f64` | `analyze.ruff.UP031` | `pyflakes/test/test_api.py:577` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_404704` | `analyze.ruff.RUF100` | `pyflakes/test/test_api.py:582` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_f3c6bb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:583` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_04b79a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:585` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7adab1` | `analyze.mypy.var-annotated` | `pyflakes/test/test_api.py:599` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_da1db2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:600` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_774967` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:601` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cd86fb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:605` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b20842` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:605` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_869dda` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:607` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_70dfdf` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:607` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1b73ca` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:611` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d0d9f0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:617` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3ee875` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:628` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_189518` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:630` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6a452e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:640` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bccbd8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:644` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a8f23f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:647` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3e2364` | `analyze.ruff.PLW1510` | `pyflakes/test/test_api.py:657` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_00cd76` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:660` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_53b6d9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:666` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_675eba` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:669` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2be346` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:676` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5a8da0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:677` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3c857f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:677` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f9f086` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:680` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6161b6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:686` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_63e9c0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:690` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_215b60` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:698` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_82ea41` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:708` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6a828a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:712` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2089a0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:713` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8f6ab1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:713` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cac7c7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_api.py:721` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_12affc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:724` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fce3c7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_api.py:726` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_257f0f` | `analyze.mypy.arg-type` | `pyflakes/test/test_api.py:729` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_274ca5` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_builtin.py:10` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2c9c75` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_builtin.py:11` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f51e42` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_builtin.py:22` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6e1e64` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_builtin.py:23` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fd4ac0` | `analyze.ruff.I001` | `pyflakes/test/test_code_segment.py:1` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_737e7b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_code_segment.py:12` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_927d4d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_code_segment.py:13` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7c0326` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_code_segment.py:19` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8988ee` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_code_segment.py:25` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1b1798` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_code_segment.py:26` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a0a6b4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_code_segment.py:32` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_74deb3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_code_segment.py:38` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_309175` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_code_segment.py:39` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7b318d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_code_segment.py:82` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_09db27` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_code_segment.py:83` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0a545e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_code_segment.py:128` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dbec0a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_code_segment.py:129` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ecab75` | `analyze.ruff.I001` | `pyflakes/test/test_custom_builtins.py:1` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_97b97a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_custom_builtins.py:10` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_719dae` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_custom_builtins.py:19` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_04fa60` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_custom_builtins.py:20` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_15b929` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_custom_builtins.py:21` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_533390` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_custom_builtins.py:23` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_df8d92` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_custom_builtins.py:24` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b94174` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_custom_builtins.py:29` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0cd8a7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:11` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_920b13` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:12` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_577af4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:18` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_699fe3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:19` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0c808c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:21` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cd8b81` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:22` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ac59c5` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:28` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ab6cf5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:29` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_169874` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:37` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c64883` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:38` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_28c868` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:48` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3cf1cc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:49` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c17986` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:55` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_07c7ff` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:56` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a6b125` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:62` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_155c97` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:63` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cd71ec` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:69` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5ddf89` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:70` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d5f93e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:76` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fc252c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:77` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2597d7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:83` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a1c4d7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:85` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8be4a3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:91` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5bccd1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:92` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a22756` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:98` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_845692` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:99` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_571eba` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:108` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_726a39` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:109` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8e78d0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:119` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2f3eee` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:122` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1f5591` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:132` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6bb093` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:138` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_214312` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:146` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_845e36` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:147` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_989049` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:151` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f9e447` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:152` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_25dee9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:156` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a6f1ea` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:157` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6368e2` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:159` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_710647` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:160` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5505ef` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:166` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d46931` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:167` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5fcbd8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:169` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_56db48` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:170` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_00c0f7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:174` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7d9ac1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:175` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1d2e42` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:181` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1e3e28` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:182` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0c3f00` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_dict.py:186` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_53f449` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_dict.py:187` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ae6ff4` | `analyze.ruff.I001` | `pyflakes/test/test_doctests.py:1` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_2bf78f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:20` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3f8a23` | `analyze.mypy.var-annotated` | `pyflakes/test/test_doctests.py:21` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_11c354` | `analyze.ruff.PIE810` | `pyflakes/test/test_doctests.py:25` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_aafbfe` | `analyze.ruff.UP031` | `pyflakes/test/test_doctests.py:32` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_8df067` | `analyze.ruff.UP031` | `pyflakes/test/test_doctests.py:34` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_6b5f63` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:44` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_434011` | `analyze.mypy.misc` | `pyflakes/test/test_doctests.py:45` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1f7733` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:45` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_696eec` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:52` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c36def` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:54` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_98e04a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:92` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_55f925` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:94` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_249d18` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:135` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8b947f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:137` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_66cf9e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:178` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c38a8a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:179` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4ac4ee` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:188` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f05347` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:190` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9e3bcd` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:203` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_532d44` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:210` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9f4041` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:224` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ce4988` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:226` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_48a1c0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:236` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f8cea5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:237` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f5600c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:247` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f8d5f3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:248` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_110d76` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:260` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d93562` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:261` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fd7503` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:272` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c92559` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:273` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b65b1c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:284` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_44e95a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:285` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_30d02a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:296` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e54cb8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:297` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4bd0e8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:310` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a76368` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:311` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_94dc70` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:343` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9d38d8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:344` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8070b4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:354` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e2419d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:355` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fde51c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:372` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b87e82` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:373` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9a31cd` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:380` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_302373` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:381` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3c009a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:392` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0ff2f1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:393` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_401ae4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:411` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_98cf2f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:412` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3b0049` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:424` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b8e5ce` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:425` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d91bfe` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_doctests.py:434` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b39292` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_doctests.py:435` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9f0022` | `analyze.mypy.no-redef` | `pyflakes/test/test_doctests.py:447` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cff102` | `analyze.mypy.no-redef` | `pyflakes/test/test_doctests.py:451` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2e57fb` | `analyze.mypy.no-redef` | `pyflakes/test/test_doctests.py:455` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_819cd4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:16` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_884f6a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:21` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ea73c9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:26` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5d292f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:31` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b17bd8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:37` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_be528d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:42` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_691eec` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:47` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_52e26d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:52` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_57669f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:57` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e3af4d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:62` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2a2abe` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:67` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2be086` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:72` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5dd330` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:77` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_05dfce` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:82` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8be99c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:83` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c52a42` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:87` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8e7c8f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:88` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c92200` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:92` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e72cb6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:93` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_982558` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:97` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_064908` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:101` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d0a80b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:105` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_40ffdd` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:109` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d009cc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:114` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5446ed` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:119` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_115ce2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:120` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_874478` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:121` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e836cc` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:123` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_29b085` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:124` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_82d8a2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:125` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1fc28e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:126` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_550876` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:127` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c9deea` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:128` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_19c233` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:129` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_58a601` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:130` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_198dd2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:131` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2a2cf8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:132` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fc1a9f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:134` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b60126` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:140` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_523d6e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:146` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_97af55` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:147` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5c9acb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:149` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dfb0b6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:152` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4f189f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:154` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_663f97` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:155` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_91c939` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:156` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d936c3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:158` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4538f2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:159` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_259042` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:160` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_826105` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:161` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3b8960` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:163` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bbb011` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:164` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_19774c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:165` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e2e8e3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:166` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8a029f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:167` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1786e9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:169` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_169aeb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:170` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a966b7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:171` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8480e8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:172` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_03d28b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:174` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8113ad` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:179` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fc1782` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:186` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_96e3c9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:191` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fd9c6e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:199` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7e021a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:204` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_099f91` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:212` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_90e9b2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:217` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_063d89` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:224` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_077499` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:229` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ef9416` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:238` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5233d6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:239` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_03ec98` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:251` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6c829b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:252` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a18263` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:262` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c1c314` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:263` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5dd884` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:274` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f92142` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:275` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fd2616` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:285` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4f4ebd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:286` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1daa6c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:299` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_585bf3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:300` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6ee189` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:306` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d84d78` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:311` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1958fe` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:319` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_193566` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:324` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a26b35` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:335` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_077b59` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:340` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5724af` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:353` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_355912` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:354` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f4996b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:360` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_20ab6d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:365` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_145bf6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:371` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9bb82b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:376` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2c46f4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:383` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_66b711` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:387` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7287ee` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:392` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ff305c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:399` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_36cafd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:400` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7a6df7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:406` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fd015f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:407` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_016aae` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:413` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_78f1d6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:420` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_039784` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:421` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ea2c57` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:423` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f7fa3b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:424` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5bdd62` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:425` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4fcaa8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:427` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_295dee` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:428` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0e477d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:430` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7d003b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:431` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_349be3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:436` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ba25b7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:437` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_77b1b6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:442` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a4165d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:443` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1c91d3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:449` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ec433f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:450` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3ed4ab` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:456` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0b8fdc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:457` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_805948` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:459` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f5992b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:460` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_75d789` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:466` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_898c0f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:467` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_48e117` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:473` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_81ec3f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:474` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_48180b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:481` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_901147` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:482` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_42ca00` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:488` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7213e5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:489` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dde1bd` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:497` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_921b2b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:498` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_39112c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:504` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_808eb1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:509` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_65075d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:516` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fdcb02` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:521` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ce7545` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:528` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_58c1e2` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:535` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_457b7f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:536` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_aa9fca` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:542` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6a0096` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:543` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_051313` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:544` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_89c7ba` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:545` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2ac430` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:546` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_12b2cb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:547` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_721b53` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:548` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_93b2a3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:549` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_852d22` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:550` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_34b951` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:551` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8dc02b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:552` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c4c344` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:553` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a5bbd2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:554` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_860ac2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:555` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6d48ff` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:556` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a78291` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:557` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_349fa0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:559` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4f35cf` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:560` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fb5fb1` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:562` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fb39e7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:563` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_21ab81` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:565` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_44bb7d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:566` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_93031b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:567` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e9a0f4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:568` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_eb1fc4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:570` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5bac44` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:571` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a72e18` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:573` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ab5a2d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:574` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_05c4fe` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:576` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b20d28` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:577` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ed5a5d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:583` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9f1de9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:584` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_adc330` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:590` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_28fe8f` | `analyze.mypy.arg-type` | `pyflakes/test/test_imports.py:593` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5e7041` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:594` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_606684` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:600` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dead63` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:601` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_95c3c0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:606` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_618cc4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:607` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b4dda6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:613` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cbd359` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:614` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fda0e2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:615` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_99e510` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:617` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_48454d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:618` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f694b0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:624` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_de6e15` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:625` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dd7c53` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:627` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_20eaaf` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:628` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e3bfdb` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:630` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b435f7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:631` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3b0ab3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:632` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4aa5e5` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:634` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a4595d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:635` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f5e757` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:636` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_098b5d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:638` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_16eb51` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:639` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8f560a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:645` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_45cea8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:651` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_14d77f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:652` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_43f036` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:658` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a43d94` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:663` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0eb7dc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:668` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0b0a0f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:673` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_133394` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:678` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1d845c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:684` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_aa3064` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:689` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_98c297` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:694` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b0d740` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:696` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5cc604` | `analyze.ruff.UP031` | `pyflakes/test/test_imports.py:696` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_fb35e9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:698` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_576248` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:699` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_763aac` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:701` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b3ccf7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:702` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1b141b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:704` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_437b35` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:706` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ca3976` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:707` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_59b3b3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:709` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_45bc86` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:710` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2dd855` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:716` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1ee6ad` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:717` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_17acec` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:724` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_53b800` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:725` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f397c3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:732` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_25857b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:733` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_eedb61` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:741` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_abc2e0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:743` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_de1636` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:744` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a5ad65` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:751` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_476a48` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:762` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cf4730` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:764` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4d7a89` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:765` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e7a743` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:772` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e77d8f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:783` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e82502` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:794` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3f8ce7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:796` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_461cb3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:800` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5eafae` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:805` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_981a1e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:813` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_469693` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:817` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8dbde9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:822` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a13b62` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:827` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b96192` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:829` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_339527` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:834` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c3b047` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:838` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5da96a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:844` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_944842` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:845` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_930f4b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:851` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7abd8d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:856` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e3f011` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:860` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_19de3d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:866` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bc772e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:870` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_48e373` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:876` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_76d52b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:882` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_004502` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:886` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0478ee` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:892` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_374707` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:898` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b02534` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:902` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_06472f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:911` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d872c4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:912` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_78d55f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:913` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5c21f2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:914` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_756ade` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:915` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d9fae0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:917` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_96b811` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:918` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5aa0df` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:926` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1b9bc8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:927` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ed3378` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:935` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_00e886` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:936` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_966fcf` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:942` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_829831` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:943` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_206d8d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:950` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_75bb94` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:951` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_25f42a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:954` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_34ff72` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:955` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4e18c1` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:962` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f32835` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:964` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_86d95f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:971` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f398f2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:975` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a57830` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:983` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f09366` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:985` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6ecf68` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:986` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c0da28` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:991` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d61f07` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:995` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3a32a9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:999` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_66d1ee` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1005` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_deff72` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1007` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_32a7c3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1015` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4c937a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1017` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9b8d80` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1022` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ee3014` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1024` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_494274` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1033` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2e3e99` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1038` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3df204` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1044` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d6ae84` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1048` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_09697e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1054` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3597b9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1055` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_94feea` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1060` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8202a9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1065` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_00137d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1069` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a663d0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1074` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5d04ee` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1078` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6b4ace` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1087` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d34ed3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1091` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f47795` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1096` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c5f7f1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1100` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4e8f7f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1105` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7f108a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1106` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b806a0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1111` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bac647` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1112` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4a3169` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1117` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6ae9d0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1118` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0a7f2a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1124` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_70e8cd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1125` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6051c7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1130` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bfd80b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1132` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_976944` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1137` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_07dbc9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1138` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5896e4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1144` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_248901` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1145` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_646ae7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1151` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7b4935` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1155` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_725b66` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1162` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b99ff9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1166` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8b547e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1170` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_df40ba` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1176` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_751559` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1178` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8dc6d2` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1184` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cb0b3c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1188` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2b2528` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1189` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9cfe98` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1191` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0dbf1f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1196` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f4c4af` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1199` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b01bef` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1205` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_12bb84` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1212` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_346694` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1219` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0cbbbf` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_imports.py:1225` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c5d77d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1231` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bd7cc4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1238` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_323540` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_imports.py:1245` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c8f893` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:6` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ffe61b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:7` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1718c4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:13` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_836ced` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:14` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0b7b4b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:20` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4a7c15` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:21` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3d834b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:27` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ba8aa5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:28` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1ebb71` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:34` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_94187d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:35` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_584a14` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:41` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e0ece1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:42` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bae5bd` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:48` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dd7dfc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:49` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_26769a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:55` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0c5c63` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:56` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c276ad` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:62` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ba346b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:63` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d7d5b9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:69` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5dc387` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:70` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1f8909` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:76` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e0e1a7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:77` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bfe29c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:83` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_23334b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:84` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9dc2f8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:90` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_060856` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:91` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c6ef56` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:97` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9ac0ff` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:98` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8fb99e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:104` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ef0718` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:105` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_24a95b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:111` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_373661` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:112` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1cb203` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:118` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4a6201` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:119` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4cb993` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:125` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_57ac58` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:126` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fea6f0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:132` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bd9633` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:133` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_836fa1` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:139` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7abd00` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:140` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5fa885` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:146` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_46559f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:147` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7b8ab9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:153` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_820b13` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:154` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3f3dc3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:160` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e13b55` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:161` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ce53e6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:167` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_102e32` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:168` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_28d33d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:174` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d78913` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:175` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ec8699` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:181` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_065604` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:182` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5095ff` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:188` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f233c1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:189` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_da0736` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:195` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_be8b77` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:196` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e1bd68` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:202` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b8005e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:203` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0f9aab` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:209` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_162524` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:210` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7f6d02` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_is_literal.py:216` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b006de` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_is_literal.py:218` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bb2bc3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_lazy_imports.py:9` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4f237e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_lazy_imports.py:10` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_966352` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_lazy_imports.py:15` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_96204a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_lazy_imports.py:16` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5a2fdf` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_lazy_imports.py:20` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2065d7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_lazy_imports.py:21` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4bb37a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_lazy_imports.py:28` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_29f460` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_lazy_imports.py:29` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_68a073` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_lazy_imports.py:47` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2fb755` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_lazy_imports.py:55` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f31dba` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_lazy_imports.py:56` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_940803` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_lazy_imports.py:66` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a530e9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_lazy_imports.py:67` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3b81a8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_lazy_imports.py:85` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_843261` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_lazy_imports.py:86` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4b6459` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_lazy_imports.py:92` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d8dced` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_lazy_imports.py:93` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_932250` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_lazy_imports.py:99` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0af629` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_lazy_imports.py:100` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fdc072` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_match.py:5` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9bf61a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_match.py:6` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_282a6f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_match.py:13` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_212f2e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_match.py:20` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9d1a5b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_match.py:28` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_27d0e3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_match.py:29` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bfca04` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_match.py:36` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_46ecdc` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_match.py:44` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4d2d37` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_match.py:45` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4e5636` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_match.py:52` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6951c3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_match.py:53` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a4c3e8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_match.py:59` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c3f775` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_match.py:60` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0f3808` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_match.py:66` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_515278` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_match.py:67` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e1318a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_match.py:74` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8e1849` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_match.py:75` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7b2837` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_match.py:82` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_30ed6d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_match.py:83` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7884a4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:13` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_00d544` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:14` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0030bc` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:16` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a13130` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:17` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_62ac1d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:24` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_51ca31` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:29` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_197e15` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:33` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3219d9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:38` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_52c3cd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:43` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9b0a35` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:47` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a0bff2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:52` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6d995a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:57` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_51953e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:62` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_58a1d1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:66` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_abf473` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:71` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d80ee3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:76` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6d920c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:80` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_944294` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:86` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_323f49` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:91` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5606da` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:95` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1769cd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:100` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_544d84` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:105` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a53a30` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:109` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d2b340` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:115` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b33c09` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:120` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3fc63b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:125` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7cdc59` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:126` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0f0086` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:131` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_db165c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:136` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9c4256` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:141` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b6625e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:145` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_899364` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:150` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4a9f78` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:155` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c07b58` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:161` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4b97c5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:166` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_205b5c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:173` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5f48a5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:178` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7f7281` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:184` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5ba8d3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:189` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c73ceb` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:196` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_530a82` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:201` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9f9b64` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:209` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e1c64d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:214` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4a22fb` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:221` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6d5e07` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:226` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6af406` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:233` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5ac374` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:238` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a5d58c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:244` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_68931a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:245` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1c8acd` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:258` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1cf641` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:260` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4e7124` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:262` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cddf07` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:267` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6728f0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:272` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6be8e7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:277` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fa496d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:282` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0ecb4e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:288` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9d904a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:294` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_af218d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:298` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_07173b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:305` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_15c6ce` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:309` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b81f5c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:316` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1f1180` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:320` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2c1424` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:327` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_be9aac` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:331` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4c0796` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:336` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_695f65` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:340` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_55f8ba` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:344` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_166c16` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:348` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_92fdd9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:353` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1cf69e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:357` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ecb396` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:361` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_01b6ef` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:365` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8a8dcf` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:370` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_abd433` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:374` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_40896f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:378` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6828cd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:379` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_50b704` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:383` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c8211a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:388` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_afefd3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:395` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_acc3fb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:404` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5b078e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:410` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2b4a00` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:416` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5028d0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:417` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7b3c69` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:422` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0d31ae` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:427` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3754c7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:433` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2803bf` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:439` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d49692` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:449` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ac86b9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:458` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_df3c6c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:459` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fc1358` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:463` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a463b8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:468` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7e9ac6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:475` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a5914d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:484` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_091bf3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:490` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_caf8e0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:496` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5121e8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:503` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b63f11` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:504` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2b7edd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:509` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a387a1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:514` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_239181` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:520` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bf970f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:526` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9fade5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:536` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2ed945` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:545` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_08f058` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:553` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_699b61` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:563` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5e3736` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:585` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bad7c0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:592` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6391c5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:601` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0f77e8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:608` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ab2066` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:617` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_209173` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:626` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_421bcf` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:637` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_564ad6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:638` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b8eb51` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:647` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_130766` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:656` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_600758` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:667` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5cd380` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:680` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2fc2ec` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:691` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4153bb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:702` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fc5ea9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:715` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_85f162` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:730` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e5cf54` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:741` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c39fc3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:752` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ebd2fe` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:765` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e0d689` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:780` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_749de2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:793` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_df03ad` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:806` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4e15cf` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:821` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_433864` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:838` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1d970a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:842` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_632318` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:846` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_54f4bb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:850` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8ffad9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:854` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ac251a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:858` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_954d18` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:862` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_61bc08` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:866` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b58394` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:870` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_81cc35` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:874` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_954f10` | `analyze.ruff.UP031` | `pyflakes/test/test_other.py:879` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_b1ea27` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:881` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a09718` | `analyze.ruff.UP031` | `pyflakes/test/test_other.py:883` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_0e537a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:885` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b65f44` | `analyze.ruff.UP031` | `pyflakes/test/test_other.py:887` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_fbdbe1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:889` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_75a58a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:891` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_584f28` | `analyze.ruff.UP031` | `pyflakes/test/test_other.py:897` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_326603` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:899` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1188e3` | `analyze.ruff.UP031` | `pyflakes/test/test_other.py:901` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_f500f2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:903` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_98b8fd` | `analyze.ruff.UP031` | `pyflakes/test/test_other.py:905` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_90a456` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:907` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c180cf` | `analyze.ruff.UP031` | `pyflakes/test/test_other.py:909` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_856204` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:911` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_de3d10` | `analyze.ruff.UP031` | `pyflakes/test/test_other.py:913` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_fd36c2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:915` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6d91f2` | `analyze.ruff.UP031` | `pyflakes/test/test_other.py:917` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_f198d1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:919` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_294320` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:926` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fc1eac` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:930` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ba8420` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:934` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_61f52d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:938` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_315f09` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:942` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_51a0e5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:946` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a5bcac` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:950` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6ce820` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:954` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_15f37f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:958` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3beadc` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:963` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_abbb04` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:968` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_91ff29` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:973` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_907d9c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:978` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_de1fd6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:984` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_71d23f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:989` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_da27f0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:995` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_51237a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1000` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3463c5` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1011` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f55971` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1016` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7146af` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1023` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_698b46` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1028` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1345b6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1035` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b65a10` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1039` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_56b768` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1043` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_71ef77` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1048` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1d8ab4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1052` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_355d84` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1056` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_861749` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1060` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_802250` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1065` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e34e7d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1070` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6408a0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1075` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_735166` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1080` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3bc0d2` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1085` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_37dd90` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1089` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5b9fa5` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1094` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_092105` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1095` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_899edb` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1105` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_58687b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1106` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_54b748` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1119` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_94bdb7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1120` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e1b78d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1128` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0f83b2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1129` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3c2959` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1141` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_74e4ad` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1142` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1bf523` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1149` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_da11e2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1150` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_322fb3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1158` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7bf749` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1162` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ac7a07` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1167` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ee4caf` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1172` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d7fe61` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1177` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ed2dca` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1182` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_94da5e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1183` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d4df2b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1194` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e9536b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1199` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_44205c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1204` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_96619b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1209` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_35f78d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1214` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2dae99` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1218` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b54fee` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1224` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6de527` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1228` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_677aad` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1237` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3ba49c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1241` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_67b8d9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1248` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_343f9c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1253` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0d0a77` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1260` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5c47f3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1265` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_76b4ba` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1272` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2775a8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1277` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0c373a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1284` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f608f4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1292` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_57b2c9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1299` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3f989f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1303` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_344364` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1309` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f67128` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1314` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e17a80` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1319` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_74cc0f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1324` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4980be` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1329` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_438b5c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1333` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3a6b0d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1342` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d3d6c1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1348` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_284db5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1352` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8e1b9f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1356` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e017dc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1362` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c2b182` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1366` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bea79b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1371` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0937c6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1375` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fb945f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1379` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6ee290` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1384` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a0caa6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1388` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4390ca` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1396` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_84e35e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1401` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0106ce` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1410` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d218fa` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1415` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_78b68e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1420` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6c002f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1425` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_65d9bc` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1430` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d82e25` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1434` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c9bdd3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1435` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_267826` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1436` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_485ed0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1438` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ac6433` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1442` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c92bcf` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1443` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_565f82` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1449` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dce6fe` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1455` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_26d93d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1461` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c7a3bd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1466` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ae73e0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1473` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b55339` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1478` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5cc521` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1484` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8448f5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1489` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bf67fe` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1495` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0465d2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1500` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9c4f0c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1506` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_60a865` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1511` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2c49a8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1517` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bff694` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1522` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9c89bc` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1528` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fd73a5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1533` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6ba5ee` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1539` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e8b7fc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1547` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_03f952` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1554` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6e477e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1559` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_47d499` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1565` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_243fe0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1571` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f5570c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1577` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0fefa8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1582` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2d541e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1588` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_be1dec` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1594` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bfc220` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1600` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3298f6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1605` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d4ccaa` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1610` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_196fbe` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1615` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_52081a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1621` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4bc00c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1626` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bc0b42` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1631` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ee34fd` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1636` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5306a0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1640` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e706b0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1644` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d9e9bc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1648` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b65459` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1653` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e45683` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1654` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a9a612` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1659` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_54a3b4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1665` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c42a3d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1666` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_29a058` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1671` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ce1163` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1672` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_044f6f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1679` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d5721d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1680` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a681d4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1686` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_aaaed2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1692` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_55ed61` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1699` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_35edba` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1704` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bd1e35` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1711` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_23f853` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1716` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_58b0e8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1722` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_78b3c7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1724` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9aaa5c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1729` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d5a641` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1731` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3f83a5` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1736` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c5b217` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1738` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_715323` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1743` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c52826` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1745` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b9ed1f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1749` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f96c58` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1751` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_776a01` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1756` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2b15b4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1760` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c1b6fc` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1765` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cfe787` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1767` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0817ea` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1773` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4e6d27` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1774` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_538b65` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1779` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_55c7b9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1781` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e511f4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1787` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_316c92` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1788` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5bfacd` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1794` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dbd570` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1796` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_61ae6b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1801` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f83851` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1802` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7dd335` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1808` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_13ab88` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1810` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ab0383` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1816` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9b675a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1818` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e28ae8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1826` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_33457c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1827` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6f7478` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1836` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_de5688` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1837` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6fecdc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1838` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_57e620` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1843` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c6e81f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1850` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_88f9e6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1852` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_02e2c5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1857` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_38b33d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1862` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8ffb5d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1868` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_aa79d3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1869` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7d5b87` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1871` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_340d34` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1876` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f9319e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1877` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e5e8b5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1880` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_41806f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1883` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c6cf52` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1886` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_aa57b5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1889` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_633ba7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1892` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_62b396` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1895` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6fcb0b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1898` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_efc6a6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1901` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_05535d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1904` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_95a0dc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1907` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bf2074` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1911` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f623e2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1915` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_33701d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1916` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e75dd0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1918` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4e93a6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1922` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a8e53c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1926` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_35bb07` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1930` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5c137e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1934` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d92d8d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1939` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cb4826` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1940` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8f1d14` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1943` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7c006b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1946` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_752290` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1949` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7437e1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1952` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b48919` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1955` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0f1861` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1958` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_02a6ef` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1961` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_02ee60` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1964` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_09bb3f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1967` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_29dcd9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1970` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_25676e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1973` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_724524` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1977` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0c39a3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1981` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_938b8f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1985` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9c1b40` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1989` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f2921e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:1994` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cc7160` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:1995` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8f811e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2000` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_88a3a4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2005` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ac3d09` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2006` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cb5f1d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2014` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8553f9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2015` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5ccfd7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2020` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5b63e0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2021` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_946c07` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2026` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b550cc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2027` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_07f6c4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2032` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f5e383` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2033` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_565db1` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2041` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c06e0a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2042` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a92bb3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2048` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9605c4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2049` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8c13c5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2059` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1061d3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2069` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_147b81` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2070` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6e39e8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2080` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_37c367` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2090` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b95c43` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2091` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ac09e6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2097` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c6e57f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2098` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_35cb8f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2106` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a8b3f3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2107` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e49e8d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2112` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f14966` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2113` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e60093` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2119` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a9a8dd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2120` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2e5400` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2124` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f623dd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2128` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fb8568` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2132` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_abbd6b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2142` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d29ba9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2143` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3f8543` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2147` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6a8c82` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2148` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0f76e7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2157` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fc43d5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2159` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d79e15` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2161` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1ac578` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2165` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e243b5` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2171` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ac1d35` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2172` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_31b03f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2177` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d74fc0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2178` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2629d8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_other.py:2184` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_38cd21` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_other.py:2185` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cbcd16` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:13` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_011a96` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:15` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_342703` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:42` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c37187` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:44` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_77ca9b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:71` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0130c9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:73` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_52cbf6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:88` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_45270b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:89` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d592a1` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:107` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_14f8cf` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:108` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_94f5c2` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:123` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cb9e7b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:125` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_686832` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:140` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ff6925` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:142` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cb6244` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:154` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fda633` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:155` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6bf297` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:158` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c0a47e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:162` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_587887` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:166` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4b2754` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:171` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a96768` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:176` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_410c8c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:181` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3affdb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:187` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ca800b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:192` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_30cd58` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:197` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a76044` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:200` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4ee23a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:203` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_69b414` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:207` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bf3e68` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:211` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2c474a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:216` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4d287a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:221` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1efe03` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:225` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_56e3fb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:229` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7908f0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:233` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_06e60d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:237` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a16e45` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:241` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7abca7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:245` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7dff58` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:249` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f8b232` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:253` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_abd2a5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:256` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6124dc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:259` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_99c5ce` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:263` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e54a7f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:267` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fc6869` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:272` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_951871` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:276` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_92ece2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:280` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2e2cd3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:284` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_122373` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:288` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_24bd4f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:292` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_182ee9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:295` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a453d4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:298` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ca518d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:301` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_90edca` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:305` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_07d7ab` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:306` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_68f0b2` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:310` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1f5783` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:311` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_606597` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:317` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2ed4cc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:323` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4b6798` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:330` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_60847d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:337` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0fc30c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:342` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e74231` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:349` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cb8550` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:350` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_96059e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:356` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b324f4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:358` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5d0bae` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:363` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a7d9dc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:368` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0aef70` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:374` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a33d0f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:375` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bc2718` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:381` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_99787f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:382` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7f9e56` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:387` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_56a51c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:388` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_755090` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:393` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b9a05c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:394` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_698b8f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:402` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_10a419` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:410` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_32c8a9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:417` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_284f51` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:418` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_526ff6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:426` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5931aa` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:427` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0510f9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:432` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bdbe05` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:433` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_08c14b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:444` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_69b017` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:445` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f8c470` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:454` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0a260d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:455` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a160e6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:462` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bc4586` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:463` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4b4e54` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:469` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_881822` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:470` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4a2182` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:478` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fabfb8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:479` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b4e2df` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:486` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f34386` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:487` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4ccf6a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:494` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d6a37d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:495` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ae7d8e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:501` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_df9a26` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:502` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8c8917` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:507` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3f66f5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:511` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5f970f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:517` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c522f3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:518` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d9d31b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:524` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e62dca` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:525` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_096c9e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:530` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_41d419` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:531` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6663ae` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:537` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bb2196` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:538` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_762f2f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:545` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_378a0d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:546` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0bd8c0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:551` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c1918f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:552` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e32c9e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:558` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b4d61f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:559` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fb8bb9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:568` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d501be` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:569` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_45d3fc` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:575` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_04fa8a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:576` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_07f5d3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:582` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b48ecb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:583` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3e30f9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:589` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_25db13` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:590` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f847bb` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:597` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0c8a40` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:598` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5725c9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:605` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0b665d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:606` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d7a02c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:613` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cf9e0d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:614` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2df4f7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:621` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b94540` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:622` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f9d7ff` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:629` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c8b188` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:630` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5dcfd8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:637` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d9c196` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:639` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7e8efe` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:649` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7c3fb7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:650` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3fd565` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:657` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_79efe5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:658` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_16285a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:667` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cada5c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:668` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ce0cea` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:679` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a7fd30` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:681` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_aefce4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:693` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_380f5e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:695` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4868dc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:705` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ea82a1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:713` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8dd73a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:723` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_998850` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:724` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_55fc5a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:737` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c44e27` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:738` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_74161a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:746` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ee052f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:755` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dffed9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:765` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e256c2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:767` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f9a773` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:773` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5ec3df` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:779` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_47348a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:781` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0ba930` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:787` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f256c6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:793` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fd1ade` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:794` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_19ead9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:800` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a8ed0b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:801` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0d2b90` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:807` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a284a8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:808` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_37ad5b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:817` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_eb7243` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:818` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_34104a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:825` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f1239b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:826` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7a71d6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:833` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_675423` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:834` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d729b2` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:848` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c52e9f` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:849` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d582ef` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:864` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4e7944` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:865` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f41ec7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:880` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2c70c0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:881` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a6fd4d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:891` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_59529a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:892` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bfb82b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:904` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8b95ce` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:905` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_32f175` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:915` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_05eef3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:916` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bb62f6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:921` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_298599` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:922` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8bb039` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_type_annotations.py:932` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e717dc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_type_annotations.py:933` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9e84f9` | `analyze.ruff.I001` | `pyflakes/test/test_undefined_names.py:1` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_aa41cc` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:8` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_133d6c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:9` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_65e9e6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:11` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7d1c5e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:12` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0b28d4` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:14` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b4ffef` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:15` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ed614e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:17` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fbb07b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:18` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2e4f90` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:19` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c00532` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:21` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_aa42e1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:22` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c4784a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:28` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_742157` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:32` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_043af6` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:40` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_be6191` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:45` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fa7c92` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:54` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9699e0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:60` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_75f64f` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:70` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9f14c0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:76` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_290ba2` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:85` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a81617` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:90` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_884671` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:100` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2fd790` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:102` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_63504a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:109` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fa3180` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:114` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fa57d2` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:128` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_364703` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:134` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d03f6e` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:146` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3a0be4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:154` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2880c1` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:166` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3745f5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:171` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_655e9d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:183` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_43f2bd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:188` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_98853b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:203` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a9d858` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:204` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c57595` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:211` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_493eb8` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:212` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_148db7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:214` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_69682a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:219` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_085000` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:221` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_709b9a` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:226` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0782e0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:228` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7023aa` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:233` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7236cd` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:235` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_55abd1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:240` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_30f339` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:242` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6b1db1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:247` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0798fa` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:249` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3181d9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:254` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0ab5f3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:255` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d4d8b9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:257` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_394a88` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:262` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9e24d4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:263` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0f6ca6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:267` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ba45cf` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:273` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0182ca` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:278` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_757beb` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:279` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_030188` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:283` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9fefc1` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:289` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7650e7` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:291` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2e19cf` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:294` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_0fd13b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:299` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b87a6e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:303` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6b5850` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:308` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b38805` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:312` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cd6e78` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:317` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cdc05e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:321` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bc1fa0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:327` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b7cdc6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:331` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f32583` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:340` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_da3489` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:342` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c4cf66` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:350` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c74b12` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:352` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_da4f44` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:354` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5c4012` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:356` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4e39f5` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:364` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_06522b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:366` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_058a08` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:368` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_12a321` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:372` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_471e75` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:380` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_059ce5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:385` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_903897` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:394` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_543604` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:399` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_46b290` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:407` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_75f666` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:412` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cdc3f7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:420` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d85268` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:425` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_023717` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:436` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3635ed` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:437` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_98f0c5` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:442` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_46383d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:444` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ae2691` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:451` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a1e7b3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:456` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d59353` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:464` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a595fd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:470` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_68c393` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:480` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_728626` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:487` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_31073a` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:498` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9241a5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:505` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7dfc8b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:523` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_381ea0` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:528` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a090a3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:538` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dde345` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:539` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_e188ea` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:556` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_810ff3` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:558` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bb8b39` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:569` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_3bb057` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:571` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_02027b` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:580` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_afb302` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:582` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fe36b5` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:587` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_796040` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:589` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1c764b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:593` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dca8dd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:597` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6c025c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:602` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_970eca` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:606` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_25ad3b` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:610` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ec4f02` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:614` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7e6683` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:619` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_29fe1d` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:623` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_843227` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:627` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5544d9` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:631` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_068dad` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:636` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9a0379` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:638` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ac3d3e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:643` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_43d8e3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:649` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bd67d1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:651` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_709407` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:656` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_d596f4` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:658` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_14c512` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:664` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_047b41` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:670` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7be251` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:671` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_b002b8` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:676` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_12f1e5` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:681` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_1e9858` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:683` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2e2f49` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:685` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dede98` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:690` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_9ba807` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:693` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7560b7` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:696` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_31b1c6` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:701` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_4277fd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:707` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ce6380` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:714` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a441a2` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:720` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c2ef33` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:727` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_14fd41` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:731` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_642f17` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:740` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7764fc` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:748` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_685913` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:750` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c85fd0` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:757` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_7091df` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:761` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_a63ebd` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:765` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_100f3e` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:768` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_25c8bc` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:772` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5fa7d1` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:777` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_6e95e3` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:781` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bb8d5c` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:786` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_8c4a3c` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:790` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_cea4cf` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:795` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_28425d` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:799` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_fcfdbc` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:804` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_057b02` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:808` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_f47831` | `analyze.mypy.no-untyped-call` | `pyflakes/test/test_undefined_names.py:817` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dd11a9` | `analyze.mypy.no-untyped-def` | `pyflakes/test/test_undefined_names.py:824` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_2cbd3d` | `analyze.mypy.attr-defined` | `pyflakes/test/test_undefined_names.py:831` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_ec6898` | `analyze.mypy.import-untyped` | `setup.py:7` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_993d86` | `analyze.mypy.import-not-found` | `setup.py:9` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_5682ff` | `analyze.mypy.dict-item` | `setup.py:13` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_bf86f4` | `analyze.mypy.dict-item` | `setup.py:14` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_657683` | `analyze.mypy.no-untyped-def` | `setup.py:20` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_69f6a7` | `analyze.ruff.B008` | `setup.py:20` (survey) | true | An independent ruff run reports it here. | fix or configure |
| `fnd_6731e0` | `analyze.mypy.no-untyped-def` | `setup.py:27` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_c0e34b` | `analyze.mypy.no-untyped-call` | `setup.py:38` (survey) | true | An independent mypy run reports it here. | fix or configure |
| `fnd_dfe76d` | `analyze.mypy.no-untyped-call` | `setup.py:40` (survey) | true | An independent mypy run reports it here. | fix or configure |

**`static.clones`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_b53158` | `clone.pair` | `pyflakes/checker.py:696` | true | Normalized trees match as the clone type states, read by hand. | extract a shared helper |
| `fnd_d49962` | `clone.pair` | `pyflakes/checker.py:1846` | true | Normalized trees match as the clone type states, read by hand. | extract a shared helper |
| `fnd_e9306c` | `clone.pair` | `pyflakes/checker.py:1948` | true | Normalized trees match as the clone type states, read by hand. | extract a shared helper |
| `fnd_642687` | `clone.cluster` | `pyflakes/test/test_api.py:232` (survey) | true | Normalized trees match as the clone type states, read by hand. | extract a shared helper |
| `fnd_dc07e7` | `clone.cluster` | `pyflakes/test/test_api.py:364` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_dea550` | `clone.pair` | `pyflakes/test/test_api.py:383` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_aea73d` | `clone.pair` | `pyflakes/test/test_code_segment.py:12` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_e37d4a` | `clone.cluster` | `pyflakes/test/test_dict.py:28` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_5f72d4` | `clone.pair` | `pyflakes/test/test_doctests.py:92` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_838770` | `clone.cluster` | `pyflakes/test/test_doctests.py:272` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_fa953b` | `clone.cluster` | `pyflakes/test/test_imports.py:16` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_28b0bd` | `clone.cluster` | `pyflakes/test/test_imports.py:42` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_a99ba8` | `clone.pair` | `pyflakes/test/test_imports.py:82` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_6399ca` | `clone.cluster` | `pyflakes/test/test_imports.py:152` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_d03b16` | `clone.cluster` | `pyflakes/test/test_imports.py:163` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_4228db` | `clone.pair` | `pyflakes/test/test_imports.py:169` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_99708c` | `clone.pair` | `pyflakes/test/test_imports.py:383` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_4d4ac9` | `clone.cluster` | `pyflakes/test/test_imports.py:406` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_c43005` | `clone.cluster` | `pyflakes/test/test_imports.py:516` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_651923` | `clone.cluster` | `pyflakes/test/test_imports.py:1199` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_1ad4a8` | `clone.cluster` | `pyflakes/test/test_other.py:24` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_49684b` | `clone.pair` | `pyflakes/test/test_other.py:57` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_fe4abf` | `clone.cluster` | `pyflakes/test/test_other.py:1430` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_8aa3c6` | `clone.cluster` | `pyflakes/test/test_other.py:1438` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_1178cc` | `clone.pair` | `pyflakes/test/test_other.py:1876` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |
| `fnd_101582` | `clone.pair` | `pyflakes/test/test_type_annotations.py:765` (survey) | disputed | The trees match as the clone type states, but the members differ only in the Python source literal each hands `self.flakes`, which is the test's input: a clone in shape only. | none |

**`static.complexity`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_a78c88` | `nesting.deep` | `pyflakes/api.py:96` (survey) | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_8125c3` | `function.long` | `pyflakes/checker.py:104` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_601c54` | `complexity.high` | `pyflakes/checker.py:110` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_6cfa69` | `complexity.high` | `pyflakes/checker.py:819` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_3d36a8` | `function.long` | `pyflakes/checker.py:819` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_d6658f` | `nesting.deep` | `pyflakes/checker.py:819` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_1119ec` | `complexity.high` | `pyflakes/checker.py:933` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_c1209b` | `complexity.high` | `pyflakes/checker.py:1018` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_cad1b5` | `function.long` | `pyflakes/checker.py:1018` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_ba3503` | `complexity.high` | `pyflakes/checker.py:1115` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_63dfd4` | `complexity.high` | `pyflakes/checker.py:1372` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_bec8f6` | `function.long` | `pyflakes/checker.py:1372` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_8fd898` | `complexity.high` | `pyflakes/checker.py:1487` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_65ed2d` | `function.long` | `pyflakes/checker.py:1487` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_03b719` | `complexity.high` | `pyflakes/checker.py:1622` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_95140e` | `function.long` | `pyflakes/checker.py:1622` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_a72cc1` | `complexity.high` | `pyflakes/checker.py:1807` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_4da519` | `complexity.high` | `pyflakes/checker.py:1981` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_d08239` | `complexity.high` | `pyflakes/checker.py:2137` | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | split the function |
| `fnd_aa2f3c` | `complexity.high` | `pyflakes/test/test_doctests.py:20` (survey) | true | Cyclomatic complexity and length recomputed from the source with `ast` match; cognitive complexity and nesting checked by hand. | none needed |
| `fnd_4ac798` | `function.long` | `pyflakes/test/test_other.py:563` (survey) | disputed | The line count is right, but most of the test is the Python source it hands `self.flakes` as a string literal. | none |
| `fnd_67a6f7` | `function.long` | `pyflakes/test/test_other.py:637` (survey) | disputed | The line count is right, but most of the test is the Python source it hands `self.flakes` as a string literal. | none |
| `fnd_c18331` | `function.long` | `pyflakes/test/test_other.py:891` (survey) | disputed | The line count is right, but most of the test is the Python source it hands `self.flakes` as a string literal. | none |
| `fnd_18b94b` | `function.long` | `pyflakes/test/test_other.py:1876` (survey) | disputed | The line count is right, but most of the test is the Python source it hands `self.flakes` as a string literal. | none |
| `fnd_cacb26` | `function.long` | `pyflakes/test/test_type_annotations.py:154` (survey) | disputed | The line count is right, but most of the test is the Python source it hands `self.flakes` as a string literal. | none |

**`static.crap`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_80560a` | `crap.watch` | `pyflakes/api.py:18` (survey) | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_45dbc1` | `crap.watch` | `pyflakes/api.py:74` (survey) | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_1f0cee` | `crap.watch` | `pyflakes/api.py:96` (survey) | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_42b85f` | `crap.watch` | `pyflakes/checker.py:48` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_fe3d83` | `crap.watch` | `pyflakes/checker.py:110` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_949d89` | `crap.watch` | `pyflakes/checker.py:168` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_7dedbd` | `crap.watch` | `pyflakes/checker.py:195` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_090e4c` | `crap.watch` | `pyflakes/checker.py:488` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_3170b5` | `crap.watch` | `pyflakes/checker.py:554` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_447fe7` | `crap.watch` | `pyflakes/checker.py:614` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_992bb0` | `crap.watch` | `pyflakes/checker.py:624` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_b0d551` | `crap.watch` | `pyflakes/checker.py:819` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_da387e` | `crap.watch` | `pyflakes/checker.py:897` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_886dd2` | `crap.watch` | `pyflakes/checker.py:933` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_209449` | `crap.high` | `pyflakes/checker.py:1018` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | split or test |
| `fnd_db8d31` | `crap.watch` | `pyflakes/checker.py:1115` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_9dd91a` | `crap.watch` | `pyflakes/checker.py:1233` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_38d669` | `crap.watch` | `pyflakes/checker.py:1255` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_d7bb44` | `crap.watch` | `pyflakes/checker.py:1288` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_98a6a5` | `crap.watch` | `pyflakes/checker.py:1340` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_b2be89` | `crap.watch` | `pyflakes/checker.py:1372` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_50cb98` | `crap.watch` | `pyflakes/checker.py:1385` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_d1a776` | `crap.high` | `pyflakes/checker.py:1487` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | split or test |
| `fnd_9ca723` | `crap.high` | `pyflakes/checker.py:1622` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | split or test |
| `fnd_d0a1e1` | `crap.watch` | `pyflakes/checker.py:1793` | disputed | True on the pinned Python 3.12, where no test reaches it; the repository marks it `# pragma: >=3.14 cover` or `>=3.15 cover` (Python 3.14 t-strings, 3.15 lazy imports), which its covdefaults configuration excludes from coverage on older versions. | none |
| `fnd_9441e9` | `crap.watch` | `pyflakes/checker.py:1807` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_ad8480` | `crap.watch` | `pyflakes/checker.py:1858` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_a7f6c4` | `crap.watch` | `pyflakes/checker.py:1909` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_422984` | `crap.watch` | `pyflakes/checker.py:1928` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_1021b4` | `crap.watch` | `pyflakes/checker.py:1964` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_a9a927` | `crap.watch` | `pyflakes/checker.py:1981` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_b8434b` | `crap.watch` | `pyflakes/checker.py:2030` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_3757b1` | `crap.watch` | `pyflakes/checker.py:2084` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_449d56` | `crap.watch` | `pyflakes/checker.py:2109` | disputed | True on the pinned Python 3.12, where no test reaches it; the repository marks it `# pragma: >=3.14 cover` or `>=3.15 cover` (Python 3.14 t-strings, 3.15 lazy imports), which its covdefaults configuration excludes from coverage on older versions. | none |
| `fnd_c904f0` | `crap.watch` | `pyflakes/checker.py:2117` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_d05a5f` | `crap.watch` | `pyflakes/checker.py:2137` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_c0a894` | `crap.watch` | `pyflakes/checker.py:2181` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_7d82e6` | `crap.watch` | `pyflakes/checker.py:2203` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_cdf760` | `crap.watch` | `pyflakes/checker.py:2248` | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |
| `fnd_23cc6e` | `crap.watch` | `pyflakes/reporter.py:39` (survey) | true | Cyclomatic recomputed from the source and line coverage from an independent coverage.py run (the `def` line excluded) give this CRAP value. | none needed |

**`static.dead-code`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_db33c9` | `dead.export` | `setup.py:20` | false | `setup.py` is the script setuptools runs, and its body calls both functions; the module holds an entry point (fixed in 8b6e07b: functions module-level code calls are entry points). | check fix |
| `fnd_8b1e4d` | `dead.export` | `setup.py:27` | false | `setup.py` is the script setuptools runs, and its body calls both functions; the module holds an entry point (fixed in 8b6e07b: functions module-level code calls are entry points). | check fix |

**`tests.assertion-density`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_b494ab` | `density.sparse-test` | `pyflakes/test/test_api.py:151` (survey) | true | The test writes eight files and checks them with one assertion over 45 lines. | none needed |
| `fnd_99e521` | `density.sparse-test` | `pyflakes/test/test_api.py:383` (survey) | disputed | The count is right, but most lines are the source or expected output the test writes as a literal. | none |
| `fnd_e7675e` | `density.sparse-test` | `pyflakes/test/test_api.py:437` (survey) | disputed | The count is right, but most lines are the source or expected output the test writes as a literal. | none |
| `fnd_e38f7d` | `density.sparse-test` | `pyflakes/test/test_api.py:479` (survey) | disputed | The count is right, but most lines are the source or expected output the test writes as a literal. | none |
| `fnd_d37d56` | `density.sparse-test` | `pyflakes/test/test_type_annotations.py:13` (survey) | disputed | The count is right, but most lines are the source or expected output the test writes as a literal. | none |
| `fnd_c59d80` | `density.sparse-test` | `pyflakes/test/test_type_annotations.py:42` (survey) | disputed | The count is right, but most lines are the source or expected output the test writes as a literal. | none |

**`tests.coverage`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_d86c3a` | `coverage.uncovered-symbol` | `pyflakes/checker.py:1793` | disputed | True on the pinned Python 3.12, where no test reaches it; the repository marks it `# pragma: >=3.14 cover` or `>=3.15 cover` (Python 3.14 t-strings, 3.15 lazy imports), which its covdefaults configuration excludes from coverage on older versions. | none |
| `fnd_5d16b7` | `coverage.uncovered-symbol` | `pyflakes/checker.py:2109` | disputed | True on the pinned Python 3.12, where no test reaches it; the repository marks it `# pragma: >=3.14 cover` or `>=3.15 cover` (Python 3.14 t-strings, 3.15 lazy imports), which its covdefaults configuration excludes from coverage on older versions. | none |
| `fnd_421cf0` | `coverage.uncovered-symbol` | `pyflakes/messages.py:367` | disputed | True on the pinned Python 3.12, where no test reaches it; the repository marks it `# pragma: >=3.14 cover` or `>=3.15 cover` (Python 3.14 t-strings, 3.15 lazy imports), which its covdefaults configuration excludes from coverage on older versions. | none |
| `fnd_61cc1c` | `coverage.uncovered-symbol` | `pyflakes/messages.py:375` | disputed | True on the pinned Python 3.12, where no test reaches it; the repository marks it `# pragma: >=3.14 cover` or `>=3.15 cover` (Python 3.14 t-strings, 3.15 lazy imports), which its covdefaults configuration excludes from coverage on older versions. | none |

**`tests.mutation`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_4bdb17` | `mutant.survived` | `pyflakes/api.py:172` (survey) | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_161f3b` | `mutant.survived` | `pyflakes/checker.py:111` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_fbe58b` | `mutant.survived` | `pyflakes/checker.py:112` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_bf5904` | `mutant.survived` | `pyflakes/checker.py:116` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_24a11f` | `mutant.survived` | `pyflakes/checker.py:136` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_222849` | `mutant.survived` | `pyflakes/checker.py:154` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_1c97d4` | `mutant.timeout` | `pyflakes/checker.py:154` | true | Replayed: with the mutation the suite runs past 240 s (the loop never ends). | none needed |
| `fnd_f6848d` | `mutant.survived` | `pyflakes/checker.py:308` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_c550f6` | `mutant.survived` | `pyflakes/checker.py:352` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_3e1dea` | `mutant.survived` | `pyflakes/checker.py:398` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_829ba1` | `mutant.survived` | `pyflakes/checker.py:489` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_0e9b47` | `mutant.survived` | `pyflakes/checker.py:633` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_516cc6` | `mutant.survived` | `pyflakes/checker.py:643` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_74fe80` | `mutant.survived` | `pyflakes/checker.py:906` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_abfe2d` | `mutant.survived` | `pyflakes/checker.py:908` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_b09f29` | `mutant.survived` | `pyflakes/checker.py:1030` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_5dc6ec` | `mutant.survived` | `pyflakes/checker.py:1035` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_75c2ed` | `mutant.survived` | `pyflakes/checker.py:1035` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_dcf3d8` | `mutant.survived` | `pyflakes/checker.py:1051` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_e51e42` | `mutant.survived` | `pyflakes/checker.py:1281` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_d037b3` | `mutant.survived` | `pyflakes/checker.py:1317` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_592080` | `mutant.survived` | `pyflakes/checker.py:1361` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_b5bfc2` | `mutant.survived` | `pyflakes/checker.py:1530` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_d2d365` | `mutant.survived` | `pyflakes/checker.py:1531` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_d08ffd` | `mutant.survived` | `pyflakes/checker.py:1565` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_bbda2f` | `mutant.survived` | `pyflakes/checker.py:1568` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_f2ccd5` | `mutant.survived` | `pyflakes/checker.py:1578` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_780a71` | `mutant.survived` | `pyflakes/checker.py:1587` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_72790f` | `mutant.survived` | `pyflakes/checker.py:1590` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_41ad6e` | `mutant.survived` | `pyflakes/checker.py:1597` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_2c0cd3` | `mutant.survived` | `pyflakes/checker.py:1606` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_e6a511` | `mutant.survived` | `pyflakes/checker.py:1612` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_2f74be` | `mutant.survived` | `pyflakes/checker.py:1615` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_0bf149` | `mutant.uncovered` | `pyflakes/checker.py:1799` | disputed | True on the pinned Python 3.12, where no test reaches it; the repository marks it `# pragma: >=3.14 cover` or `>=3.15 cover` (Python 3.14 t-strings, 3.15 lazy imports), which its covdefaults configuration excludes from coverage on older versions. | none |
| `fnd_a25653` | `mutant.survived` | `pyflakes/checker.py:1916` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_47ce64` | `mutant.survived` | `pyflakes/checker.py:1916` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_ba3a7f` | `mutant.survived` | `pyflakes/checker.py:2004` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_69f213` | `mutant.survived` | `pyflakes/checker.py:2047` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_1420a3` | `mutant.survived` | `pyflakes/checker.py:2093` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_1b9ee0` | `mutant.survived` | `pyflakes/checker.py:2103` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_906a3a` | `mutant.survived` | `pyflakes/checker.py:2103` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_30a479` | `mutant.survived` | `pyflakes/checker.py:2103` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_d20a97` | `mutant.survived` | `pyflakes/checker.py:2103` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_94b6f0` | `mutant.uncovered` | `pyflakes/checker.py:2110` | disputed | True on the pinned Python 3.12, where no test reaches it; the repository marks it `# pragma: >=3.14 cover` or `>=3.15 cover` (Python 3.14 t-strings, 3.15 lazy imports), which its covdefaults configuration excludes from coverage on older versions. | none |
| `fnd_9c31a2` | `mutant.uncovered` | `pyflakes/checker.py:2113` | disputed | True on the pinned Python 3.12, where no test reaches it; the repository marks it `# pragma: >=3.14 cover` or `>=3.15 cover` (Python 3.14 t-strings, 3.15 lazy imports), which its covdefaults configuration excludes from coverage on older versions. | none |
| `fnd_6a3fe2` | `mutant.uncovered` | `pyflakes/checker.py:2115` | disputed | True on the pinned Python 3.12, where no test reaches it; the repository marks it `# pragma: >=3.14 cover` or `>=3.15 cover` (Python 3.14 t-strings, 3.15 lazy imports), which its covdefaults configuration excludes from coverage on older versions. | none |
| `fnd_dfe196` | `mutant.survived` | `pyflakes/checker.py:2118` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_5056fd` | `mutant.survived` | `pyflakes/checker.py:2144` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_5d8493` | `mutant.survived` | `pyflakes/checker.py:2274` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_169446` | `mutant.survived` | `pyflakes/messages.py:16` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_7953da` | `mutant.survived` | `pyflakes/messages.py:16` | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_bc10e6` | `mutant.survived` | `pyflakes/reporter.py:61` (survey) | true | Replayed: the mutation applied to a copy of the clone leaves `python -m unittest` green (795 tests). | add an assertion that kills it |
| `fnd_9ec130` | `mutant.uncovered` | `setup.py:24` (survey) | disputed | No test runs `setup.py`, as stated; but it is the packaging script, which a suite is not expected to exercise. | none |

**`tests.quality`**

| Finding | Kind | Location | Verdict | Reasoning | Action |
|---|---|---|---|---|---|
| `fnd_0e7975` | `test.duplicate-setup` | `pyflakes/test/test_api.py:232` (survey) | true | The three tests open with the same `StringIO` and `Reporter` arrange code before they act. | extract a fixture |
| `fnd_ecaed2` | `test.logic` | `pyflakes/test/test_api.py:479` (survey) | true | The test chooses its expected value by a branch (`if PYPY: … elif …`) or a conditional expression (`5 if self.withDoctest else 3`). | split per platform or parametrize |
| `fnd_dc2bbf` | `test.duplicate-setup` | `pyflakes/test/test_code_segment.py:38` | false | The shared leading statements are assertions (`self.flakes(...)`, the harness's asserting helper), not arrange code (fixed in 8b6e07b: a statement holding an assertion ends the setup). | check fix |
| `fnd_bbd4dd` | `test.duplicate-setup` | `pyflakes/test/test_doctests.py:52` | false | The shared leading statements are assertions (`self.flakes(...)`, the harness's asserting helper), not arrange code (fixed in 8b6e07b: a statement holding an assertion ends the setup). | check fix |
| `fnd_6b9e46` | `test.duplicate-setup` | `pyflakes/test/test_imports.py:163` | false | The shared leading statements are assertions (`self.flakes(...)`, the harness's asserting helper), not arrange code (fixed in 8b6e07b: a statement holding an assertion ends the setup). | check fix |
| `fnd_bc6034` | `test.duplicate-setup` | `pyflakes/test/test_imports.py:741` | false | The shared leading statements are assertions (`self.flakes(...)`, the harness's asserting helper), not arrange code (fixed in 8b6e07b: a statement holding an assertion ends the setup). | check fix |
| `fnd_9c41c5` | `test.logic` | `pyflakes/test/test_imports.py:898` (survey) | true | The test chooses its expected value by a branch (`if PYPY: … elif …`) or a conditional expression (`5 if self.withDoctest else 3`). | split per platform or parametrize |
| `fnd_1c1d7d` | `test.duplicate-setup` | `pyflakes/test/test_other.py:24` | false | The shared leading statements are assertions (`self.flakes(...)`, the harness's asserting helper), not arrange code (fixed in 8b6e07b: a statement holding an assertion ends the setup). | check fix |
| `fnd_6be13d` | `test.duplicate-setup` | `pyflakes/test/test_other.py:378` | false | The shared leading statements are assertions (`self.flakes(...)`, the harness's asserting helper), not arrange code (fixed in 8b6e07b: a statement holding an assertion ends the setup). | check fix |
| `fnd_f26d4e` | `test.duplicate-setup` | `pyflakes/test/test_other.py:416` | false | The shared leading statements are assertions (`self.flakes(...)`, the harness's asserting helper), not arrange code (fixed in 8b6e07b: a statement holding an assertion ends the setup). | check fix |
| `fnd_ecf33a` | `test.logic` | `pyflakes/test/test_undefined_names.py:498` (survey) | true | The test chooses its expected value by a branch (`if PYPY: … elif …`) or a conditional expression (`5 if self.withDoctest else 3`). | split per platform or parametrize |

## False-positive rate per check

`false / (true + false)`, disputed rows apart.

| Check | True | False | Disputed | Rate |
|---|---|---|---|---|
| `gen.property` | 16 | 0 | 0 | 0.00 |
| `static.analyze` | 2038 | 0 | 0 | 0.00 |
| `static.clones` | 4 | 0 | 22 | 0.00 |
| `static.complexity` | 20 | 0 | 5 | 0.00 |
| `static.crap` | 38 | 0 | 2 | 0.00 |
| `static.dead-code` | 0 | 2 | 0 | 1.00 |
| `tests.assertion-density` | 1 | 0 | 5 | 0.00 |
| `tests.coverage` | 0 | 0 | 4 | — |
| `tests.mutation` | 48 | 0 | 5 | 0.00 |
| `tests.quality` | 4 | 7 | 0 | 0.64 |

Every other check that ran (`perf.bench`, `perf.footprint`, `spec.contracts`, `static.deps`, `tests.flaky`) reported nothing: no rate. The `tests.quality` and `static.dead-code` rates are those of the first run; after the fixes both checks report no false row.

## Check bugs fixed

- **A statement that holds an assertion is not setup** (`tests.quality`): 7 `test.duplicate-setup` rows (`test_usedImport_relative`'s three `self.flakes(...)` calls, `test_scope_class`'s `checker = self.flakes(...)` followed by the same scope inspection). Fixture: `crates/cupel-fixtures/repos/python/tests/test_ledger_case.py` (`test_balances_of_pairs`, `test_balances_of_larger_pairs`); test `tests_quality::a_statement_holding_an_assertion_ends_the_setup_tests_share`. Commit `8b6e07b`. The same test showed that the shared count compared statements with their spans and so never exceeded `setup_statements_min`; it now compares them by shape (the one true row, `fnd_0e7975`, still reads 3).
- **A function a module's top-level code calls is an entry point** (`static.dead-code`): 2 `dead.export` rows (`setup.get_version`, `setup.get_long_description`). Fixture: `crates/cupel-fixtures/repos/python/setup.py`; test `python_api::a_function_the_module_level_code_calls_is_an_entry_point`. Commit `8b6e07b`.

Rerun at `8b6e07b`: the 9 removed rows are gone and every other finding of the first survey and check is still reported (2,221 → 2,212 and 614 → 614).

## What a maintainer would do first

Kill the 47 surviving mutants, 11 of them in `Checker.CALL` and 5 each in the percent-format parser (`parse_percent_format._parse_inner`) and `Checker.TUPLE`: the suite runs those lines, and no test notices when one of their constants or comparisons changes. Then split `_handle_percent_format` (cyclomatic 34), `handleNodeLoad` (33), and `CALL` (30), the three `crap.high` functions, whose coverage is already near full. The 2,038 `static.analyze` rows are mypy `--strict` on an untyped code base and ruff's defaults on one that lints with flake8: a maintainer would configure both tools first, not fix them.
