# CUPEL_DECISION_158: Two misleading results: `tests.flaky` on a runner whose order cannot matter, and trivial forms as clones

**Date:** 2026-10-31 · **Status:** §1 accepted; amends [078](CUPEL_DECISION_078.md) (a runner whose order cannot matter is a skip, not an error). §2 proposed: awaiting the owner's ruling on the report rows it loses · **Phase:** B.4

## Context

The owner's v0.2.2 ruling names two results that mislead.

1. **`tests.flaky` on a nextest project reads `error`.** The flake engine (ADR-078) reruns the baseline's tests with the baseline's runner, varying their order. `cargo nextest` runs each test in its own process, so no order of its tests can change an outcome (`RunOrdering::Fixed`), and ADR-078 says the engine "does not rerun it and says so". It said so with `Error::InvalidValue`, which the scheduler reports as the check's `error`. Nothing is broken, and the gate, the renderers, and an agent read an error as something to fix. The ruling: `skipped` with a reason, never `error`, the runner named.
2. **`static.clones` pairs trivial forms.** The tree gate's clone ratchets twice forced a rewrite of code that is not duplication worth extracting (P10 Notes "P10 — trivial setters as clones"). P10.1's `TestResults::with_reuse` (`self.reuse = Some(reuse); self`, 22 nodes, 4 lines) joined two existing setters into a cluster. B.1's `SuitePhase::as_str`, a four-arm `match` of variants to names, joined a cluster of four such accessors. ADR-050 compares every symbol of at least `min_lines` 4 and `min_nodes` 20. The ruling: a form whose body is a single statement is not a candidate, the default `min_nodes` rises so such setters cannot pair, and every row the external dogfood reports (ADR-138) judged a true positive must still be found.

## Options

**§1, how the skip is reported.**

- (a) Keep the error and document it. The check reads `error` on every nextest project, which is the misleading result.
- (b) The engine produces an empty `TestReruns` naming the runner, and the check skips itself when it sees no rerun. An empty `runs` also means `flake_runs = 0` or no baseline, so the check could not say why. Naming the runner's reason would also put a runner in a check, which the fact model rules out.
- (c) A new `SkipReason`, such as `inapplicable`. That is a schema enum value every consumer must learn, for what `unsupported` already means: no producer can make the fact here.
- (d) An error variant a producer returns when it cannot make its fact here for a reason only the run shows, such as the baseline's runner. The scheduler turns it into the existing skip, `unsupported`, with the fact, the tool, and the producer's sentence as the check's note. The gate, the renderers, and `next_actions` already handle that skip. No check names a runner, and any producer with such a reason takes the same path.

**§2, what makes a form too trivial to compare.**

- *Statements.* `NormTree` labels are each grammar's node kinds, so `static.clones`, which may not name a language, cannot ask a grammar what a statement is. It can locate the body: `Symbols` gives a symbol's `body` span, and each `NormNode` carries the span of the node it came from. The body's statements are that node's children. Go alone wraps them in one more node (`block` → `statement_list`). A lone child is therefore one statement unless it is such a list.
  - A list is a node whose children each start on a later line than the one before ends (Go's statements).
  - A single statement's parts share a line with their keyword: `match self {`, `if cond {`, `switch (x) {`, `return f(…)`.
  - Python's `if … :` / `else:` lays its parts on separate lines, so a Python body that is one `if` reads as several statements and is still compared. That errs toward comparing, as before.
  - A body the producer gives no span for (Clojure, whose `defn` has no separate body node, and any tree without spans) is compared as before.
- *Size.* `min_nodes` is the only size floor; `min_lines` (4) already admits a four-line setter. The value is chosen from the node counts below.

**§2, the data.** `check --only static.clones --no-install` on the eight repositories of `docs/dogfood/` at the reports' commits, through `scripts/heavy.sh`; no tool ran (each envelope's `toolchain` is empty). Two builds: `dee39a5` (before), and `dee39a5` with the single-statement rule above, run at the default `min_nodes` 20 and at 23, 25, and 30 set by `--config`. A report row judged true is *lost* when its location is a member of a clone group before and of none after. Each cell gives pairs / clusters / findings, then true rows lost.

| Repository | True rows | Before | Rule, 20 | Rule, 23 | Rule, 25 | Rule, 30 |
| --- | --- | --- | --- | --- | --- | --- |
| apache/commons-cli | 48 | 78 / 6 / 48 | 74 / 6 / 44, 4 | 64 / 5 / 39, 9 | 64 / 5 / 39, 9 | 61 / 5 / 36, 12 |
| cljfx/cljfx | 12 | 14 / 1 / 12 | 14 / 1 / 12, 0 | 13 / 1 / 11, 1 | 12 / 1 / 10, 2 | 11 / 1 / 9, 3 |
| gvergnaud/ts-pattern | 46 | 167 / 14 / 46 | 148 / 11 / 35, 11 | 148 / 11 / 35, 11 | 148 / 11 / 35, 11 | 147 / 11 / 34, 12 |
| more-itertools | 42 | 56 / 3 / 42 | 52 / 3 / 38, 4 | 52 / 3 / 38, 4 | 52 / 3 / 38, 4 | 50 / 3 / 36, 6 |
| PyCQA/pyflakes | 4 | 132 / 14 / 26 | 122 / 13 / 25, 0 | 121 / 13 / 24, 0 | 79 / 9 / 20, 0 | 74 / 8 / 17, 0 |
| rust-itertools | 66 | 98 / 11 / 67 | 37 / 4 / 29, 37 | 37 / 4 / 29, 37 | 37 / 4 / 29, 37 | 37 / 4 / 29, 37 |
| serde-rs/json | 86 | 586 / 20 / 102 | 140 / 5 / 36, 54 | 140 / 5 / 36, 54 | 140 / 5 / 36, 54 | 139 / 5 / 35, 55 |
| spf13/cobra | 64 | 207 / 31 / 65 | 197 / 29 / 59, 6 | 197 / 29 / 59, 6 | 197 / 29 / 59, 6 | 197 / 29 / 59, 6 |
| **Total** | 368 | 1 338 / 100 / 408, 0 | 784 / 72 / 278, 116 | 772 / 71 / 271, 122 | 729 / 67 / 266, 123 | 716 / 66 / 255, 131 |

Every true row was found before. Without the rule, from the before envelopes (a group survives when at least two of its members reach the floor), `min_nodes` alone would lose 9 true rows at 23, 10 at 25, and 24 at 30.

- The 9 rows lost at 23 are members of 20 to 22 nodes, the shapes the ruling excludes. They are commons-cli's builder setters (7 rows), cobra's `preRun`/`postRun`, and cljfx's `table-sort-policy`.
- The 116 rows the rule alone loses have members of 20 to 1 813 nodes, median 51; 11 are over 100 nodes. They are one statement that holds the whole body:
  - a `match` (serde_json's `Deserializer` methods);
  - a `loop` (itertools `FlattenOk::next`/`next_back`, 162 nodes, 33 lines);
  - a `for` (more-itertools `TestRunningMin`/`Max.test_basic`, 125 nodes, 20 lines);
  - a call with a closure (itertools' bench functions);
  - a `return match(…)…` chain (ts-pattern's benchmark `handle`, 1 813 nodes).
  
  Many are duplication a maintainer would extract.

On Cupel's own tree (`cupel.toml`'s scope, which leaves out the fixtures) the before binary finds 162 pairs and 17 clusters (the baseline holds 164 and 17). The rule leaves 87 pairs and 11 clusters, removing 39 groups. Among them:

- both `as_str` accessor clusters (`OracleKind::as_str` with `Setting::key` and `env_var`, 35 nodes; `Format::as_str` with `TimeUnit::suffix`, `CostClass::est_ms_per_target`, and `OracleState::as_str`, 39 to 42 nodes);
- the pair `TestResults::with_reuse` (now the struct update `Self { reuse: Some(reuse), ..self }`, one statement, 21 nodes) with `NormNode::at`;
- the check constructors `StaticClones::new` and its kin (one struct expression, 85 to 127 nodes);
- Rust tests that are one `assert_eq!` (92 to 121 nodes).

What each recorded case needs:

- `SuitePhase::as_str` as a four-arm `match` is one statement of the same 35-to-42-node shape, so only the rule skips it.
- `with_reuse` as first written (`self.reuse = Some(reuse); self`, two statements, 22 nodes) is not one statement, so only `min_nodes` of at least 23 skips it.

ADR-138 §3 calls a measurement true when the value it states about the code is correct, whatever a maintainer would do, so a correctly measured trivial setter pair is a true row. No setting of the ruling's two rules keeps every true row; the question for the owner is which losses are intended:

- **(A) As ruled: the rule and `min_nodes` 23.**
  - It skips both recorded cases.
  - It loses 122 true rows: 116 to the rule, 6 more to the floor.
- **(B) `min_nodes` 23 alone.**
  - It skips `with_reuse` as first written.
  - It loses 9 true rows, all 20-to-22-node setters and guards of the kind ruled out.
  - It keeps the `as_str` clusters and every large single-statement clone.
- **(C) The rule limited to small single statements (below about 50 nodes) and `min_nodes` 23.**
  - It skips both recorded cases.
  - It would keep the large single-statement clones.
  - It is not measured, and its cut is one more number to choose.

## Decision

1. **`tests.flaky` on a runner whose order cannot matter is `skipped`, `unsupported`** (option (d)).
   - `cupel_core::Error` gains `Unsupported { fact, tool, message }`: a producer cannot make `fact` here, for a reason only the run shows.
   - The scheduler reports a check that ends in it as `skipped` with `reason: unsupported`, `fact`, `tool`, `lang`, and `see: cupel describe <check>`. The message goes into the check's notes as an error's would. It is not an error and does not make the run partial.
   - `engine.flake` returns it for a baseline whose runner's ordering is `Fixed`: fact `test-reruns`, the runner's tool (`cargo-nextest`), and "engine.flake does not rerun tool.cargo-nextest: it runs each test in its own process, so the order of the tests cannot change an outcome".
   - A baseline runner that is not registered at all stays `Error::InvalidValue`: that is a broken configuration. The engine's behaviour on every runner it reruns is unchanged.
2. **`static.clones`**: proposed, not implemented on the branch. The single-statement rule exists as a tested change set aside (the plan's B.4 Notes say where) until the owner chooses between (A), (B), and (C) above; `min_nodes` stays 20.

## Consequences

- A nextest project, Cupel's own included, reads `tests.flaky` as skipped, with the fact it lacks, the runner's tool, and why. A gate rule on it is skipped, not failed, as for any `unsupported` skip. Cupel's dogfood still passes `--skip tests.flaky` to save the baseline run.
- `Skipped.tool` now names the tool an `unsupported` skip concerns as well as an `unavailable` skip's missing tool; the schema's field is unchanged.
- ADR-078's Consequences sentence "the engine does not rerun it and says so" now says so as a skip.
