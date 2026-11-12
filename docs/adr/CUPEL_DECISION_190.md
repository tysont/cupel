# CUPEL_DECISION_190: A check run chooses a range's impacted tests before it starts, over the whole walk

**Date:** 2026-11-12 · **Status:** accepted · **Phase:** v0.2.7, item 12 · Details [186](CUPEL_DECISION_186.md), [077](CUPEL_DECISION_077.md), [006](CUPEL_DECISION_006.md)

## Context
ADR-186 has `tests.flaky` rerun only the tests a range impacts, as `tests --impacted` selects them. `tests --impacted` chooses by the language's per-test map where it can produce one, else by the import graph: the test files that import a changed file, directly or through other files (ADR-006). It produces its facts over the whole walk, because a changed file's importers are mostly outside the range. A check run's producers see only the files in scope: under `--changed`, the import graph holds the changed files' own imports and none of their importers, so the flake engine cannot choose by imports from the facts the run gives it. And `tests --impacted` makes the per-test map whatever it costs, while a check run makes an optional fact only within its depth (ADR-080, ADR-162): a per-test map that runs each test alone under coverage costs far more than three reruns.

## Options
(a) The flake engine chooses from the run's facts: by imports it finds nothing to choose by. (b) The scheduler produces the import graph over the whole walk for every run: every static check would read it, a change to every `--changed` run. (c) The check verb, under a range, chooses each language's impacted tests before the run, with `tests --impacted`'s own code over the whole walk, and hands them to the flake engine.

## Decision
(c). When a check run has a range (`--changed`, `--base`) and a selected check needs `TestReruns`, and it is not a dry run or a recomputed base's run, the check verb runs `tests --impacted`'s choice for each language in scope over the whole walk, with the range's changed lines and the run's `--symbols` selectors, in a production of its own. One difference: the per-test map is made only where the run's depth affords it (made already, or its producer estimates it within the depth, as a want), else the import graph chooses. The choice reaches producers of that language as `Focus::impacted`: the runner's test IDs where a per-test map chose (less the tests attributions keep, ADR-151), else the test files; `All` where neither a map nor an import graph could choose.

The flake engine reruns only those tests, narrowing the runner to them where it can and keeping only their results; IDs chosen with another runner than the baseline's, or `All`, rerun every test, as a run without a range does. With none impacted it reruns nothing, records `impacted: []` in `TestReruns`, and `tests.flaky` is skipped with the reason `no-impacted-tests` (`see`: `cupel tests --impacted --changed`). The skip has no next action: there is nothing to rerun.

## Consequences
A range run whose checks rerun tests pays for its languages' syntactic facts over the whole walk once (read from the fact store when warm, ADR-181), and saves the reruns of every test the range does not reach. `tests --impacted` and the reruns agree, except where the run's depth cannot afford the per-test map that `tests --impacted` would make: the import graph's choice is then the wider one. Other producers do not read `Focus::impacted`; the mutation engine keeps its own per-mutant selection.
