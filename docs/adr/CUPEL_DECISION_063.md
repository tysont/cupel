# CUPEL_DECISION_063: Gate metrics and ranking dimensions of the P2 checks

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P2.9

## Context

P2.9 (ADR-060, ADR-062) gave checks two ways into gates and `survey`: a `scope` metric's value in `CheckResult.metrics`, combined across languages by `Check::combine_metric`, and per-target values through `sink.measure`. It integrated only `static.complexity`, because `static.crap`, `static.clones`, `static.deps`, `tests.quality`, `tests.weakening`, and `static.dead-code` landed in parallel. Merging them leaves six questions:

1. **Combining ratios.** ADR-060 §2 says a ratio combines by a weighted mean, but `combine_metric(name, values)` receives only the per-language values, so no weight is available. `tests.quality`'s `weak_ratio` is a share of assertions, not of tests, so even the score's `n` is the wrong weight for it.
2. **What each metric measures under a narrowed scope** (`--changed`, `--paths`), where `[gate.changed]` rules are evaluated.
3. **Which per-target dimension each check declares**, given that ADR-062 ranks higher values first and has no direction yet.
4. **Checks with no meaningful per-target value.**
5. **The two scheduler extensions P2.9 did not know about**: a family-wide task (ADR-051) runs once for several languages, and a check with `base_needs` (ADR-056) is skipped with `needs-range` on a tree scope.
6. **Where `tests.weakening = "none"` can be evaluated**, since the check only runs under a range.

## Options

**(1) Weights.** Report each ratio's numerator and denominator as extra `scope` metrics and combine them elsewhere (they would become gate keys nobody asked for, and `combine_metric` still sees one metric at a time); pass `(value, n)` pairs using the score's `n` (wrong for `weak_ratio`); or pass the per-language results that reported the metric, so a check can weight by anything its result carries (its score's `n`, or another of its metrics).

**(3) `tests.quality`'s dimension.** Assertions per test ranks the best-asserted tests first, the opposite of what a ranking is for, until `MetricSpec` has a direction; mock density (doubles as a percentage of doubles plus assertions, the value `test.mock-heavy` already reports) is worse when higher.

**(6) `tests.weakening` in `[gate.tree]`.** Pass the rule when the check was skipped for `needs-range` (a gate that silently checks nothing), reject the rule at configuration load (a new validation tied to one check's `base_needs`), or keep ADR-060's rule — skipped everywhere means `skipped`, which fails the gate — and make the action say how to run it.

## Decision

1. **`Check::combine_metric(&self, name, results: &[&CheckResult]) -> Option<f64>`** receives the per-language results that reported `name` (each with its score and whole metric map). The default is still the largest value. This replaces the signature in ADR-060 §2; its rule is unchanged. `cupel-checks/src/metrics.rs` holds the combining helpers checks share: `largest`, `sum`, `weighted_mean(name, results, weight)`, and `scored` (a result's `score.n` as a weight).
2. **Per check** (gate keys are `per: scope`; the rest are ranking dimensions):

   | Check | Metric | `per` | Combined across languages | Measured target |
   |---|---|---|---|---|
   | `static.complexity` | `cyclomatic_max`, `cognitive_max`, `nesting_max`, `params_max`, `length_max` | scope | largest | — |
   | | `cyclomatic`, `cognitive` | symbol | — | every symbol in scope |
   | `static.crap` | `max` | scope | largest | — |
   | | `mean` | scope | mean weighted by scored symbols (`score.n`) | — |
   | | `crap` | symbol | — | every scored symbol (test files excluded) |
   | `static.clones` | `pairs`, `clusters` | scope | sum | — |
   | | `max_jaccard` | scope | largest | — |
   | | `cluster_size` | group | — | every reported pair or cluster (members) |
   | `static.deps` | `cycles`, `violations` | scope | sum | — |
   | | `max_distance` | scope | largest | — |
   | | `distance` | module | — | every module where D is defined |
   | | `coupling` | module | — | every module (Ca + Ce) |
   | | `cycle_size` | group | — | every cycle (modules) |
   | `tests.quality` | `assertion_density` | scope | mean weighted by tests (`score.n`) | — |
   | | `weak_ratio` | scope | mean weighted by assertions (`assertion_density × score.n`) | — |
   | | `no_assertion` | scope | sum | — |
   | | `mock_density` | symbol | — | every test in scope with a symbol |
   | `tests.weakening` | `weakened` | scope | sum | — |
   | `static.dead-code` | `dead_symbols`, `dead_exports` | scope | sum | — |

   A language that scores nothing reports 0 for each metric (a ratio over nothing is 0); a language skipped for its own reason (`static.crap` without `no_coverage`) reports none.
3. **Narrowed scopes.** Metrics count what the check reports for the scope: `static.clones` counts the pairs, clusters, and similarity of the groups it reports (a group with a member in scope), while its score still counts every similar pair; `static.deps` measures the graph its producer built over the files in scope (ADR-052); the others count the symbols or tests in scope.
4. **Dimensions.** `tests.quality` ranks `mock_density`, not assertion density, until ranking has a direction. `static.deps` adds `coupling` and `cycle_size` beside `distance` because the plan names "most-coupled modules" and "dependency cycles" among `survey`'s rankings. `static.dead-code` and `tests.weakening` declare `scope` metrics only: a symbol is dead or not, and a weakening is about a test that may no longer exist.
5. **Families and ranges.** Measurements from a family-wide task carry the task's language (the family's first in scope), so a TypeScript clone group found beside JavaScript ranks with `lang: javascript`; its `location` names the real file. A `needs-range` skip gets a `rerun` action whose command is the skip's `see` (`cupel check --changed --only <id>`), and a skipped gate rule on such a check uses the same command rather than rerunning the tree scope, which would skip it again; the two de-duplicate to one action.
6. **`tests.weakening = "none"` belongs in `[gate.changed]`**, where it is evaluated as a presence rule over the range. In `[gate.tree]` it is always `skipped` and fails the gate (ADR-060 §6), with the rerun action above; it is not rejected at load.
7. **Shared manifest builders.** `kind::<Evidence>(name, action, summary)`, `metric` (a `scope` metric), `ranked(name, per, summary)`, and `evidence` live in `cupel-checks/src/manifest.rs`; every check uses them, so a check passes its action explicitly instead of each module fixing one.

## Consequences

- Every gate key a P2 check declares has a value, and every check but `static.dead-code` and `tests.weakening` appears in `survey`'s rankings; `golden/survey/*` and every check golden changed accordingly.
- A check whose ratio needs a weight it does not report must add that weight to its result; `combine_metric` cannot see raw counts.
- A `[gate.tree]` rule on `tests.weakening` can never pass. Rejecting it at configuration load (from `base_needs`) is a possible follow-up if users trip on it.
- When `MetricSpec` gains a direction (P3 mutation score), `tests.quality` can rank assertion density too.
