# CUPEL_DECISION_158: Two misleading results: `tests.flaky` on a runner whose order cannot matter, and trivial forms as clones

**Date:** 2026-10-31 · **Status:** accepted; §1 amends [078](CUPEL_DECISION_078.md) (a runner whose order cannot matter is a skip, not an error); §2 amends [050](CUPEL_DECISION_050.md) (the default `min_nodes` is 23); §3 added 2026-11-01 (v0.2.3): enum mappings are `info` · **Phase:** B.4

## Context

The owner's v0.2.2 ruling names two results that mislead.

1. **`tests.flaky` on a nextest project reads `error`.** The flake engine (ADR-078) reruns the baseline's tests with the baseline's runner, varying their order. `cargo nextest` runs each test in its own process, so no order of its tests can change an outcome (`RunOrdering::Fixed`), and ADR-078 says the engine "does not rerun it and says so". It said so with `Error::InvalidValue`, which the scheduler reports as the check's `error`. Nothing is broken, and the gate, the renderers, and an agent read an error as something to fix. The ruling: `skipped` with a reason, never `error`, the runner named.
2. **`static.clones` pairs trivial forms.** The tree gate's clone ratchets twice forced a rewrite of code that is not duplication worth extracting (P10 Notes "P10 — trivial setters as clones"). P10.1's `TestResults::with_reuse` (`self.reuse = Some(reuse); self`, 22 nodes, 4 lines) joined two existing setters into a cluster. B.1's `SuitePhase::as_str`, a four-arm `match` of variants to names, joined a cluster of four such accessors. ADR-050 compares every symbol of at least `min_lines` 4 and `min_nodes` 20. The first ruling: a form whose body is a single statement is not a candidate, the default `min_nodes` rises so such setters cannot pair, and every row the external dogfood reports (ADR-138) judged a true positive must still be found. The measurement below showed the two rules cannot keep every true row. The owner's second ruling, on that data: "Raise default min_nodes to 23; no single-statement rule", with the measurement recorded here as the basis for the threshold, so future tuning starts from it.

## Options

**§1, how the skip is reported.**

- (a) Keep the error and document it. The check reads `error` on every nextest project, which is the misleading result.
- (b) The engine produces an empty `TestReruns` naming the runner, and the check skips itself when it sees no rerun. An empty `runs` also means `flake_runs = 0` or no baseline, so the check could not say why. Naming the runner's reason would also put a runner in a check, which the fact model rules out.
- (c) A new `SkipReason`, such as `inapplicable`. That is a schema enum value every consumer must learn, for what `unsupported` already means: no producer can make the fact here.
- (d) An error variant a producer returns when it cannot make its fact here for a reason only the run shows, such as the baseline's runner. The scheduler turns it into the existing skip, `unsupported`, with the fact, the tool, and the producer's sentence as the check's note. The gate, the renderers, and `next_actions` already handle that skip. No check names a runner, and any producer with such a reason takes the same path.

**§2, what makes a form too trivial to compare.** Two rules were built and measured; the second is the decision, the first is rejected.

- *Statements.* `NormTree` labels are each grammar's node kinds, so `static.clones`, which may not name a language, cannot ask a grammar what a statement is. It can locate the body: `Symbols` gives a symbol's `body` span, and each `NormNode` carries the span of the node it came from. The body's statements are that node's children. Go alone wraps them in one more node (`block` → `statement_list`). A lone child is therefore one statement unless it is such a list.
  - A list is a node whose children each start on a later line than the one before ends (Go's statements).
  - A single statement's parts share a line with their keyword: `match self {`, `if cond {`, `switch (x) {`, `return f(…)`.
  - Python's `if … :` / `else:` lays its parts on separate lines, so a Python body that is one `if` reads as several statements and is still compared. That errs toward comparing, as before.
  - A body the producer gives no span for (Clojure, whose `defn` has no separate body node, and any tree without spans) is compared as before.
- *Size.* `min_nodes` is the only size floor; `min_lines` (4) already admits a four-line setter. The value is chosen from the node counts below.
- The single-statement rule was built (`one_statement` over the body span and a line-based list test, with three tests) and set aside unmerged when it was rejected.

**§2, the data: the basis for the threshold.** `check --only static.clones --no-install` on the eight repositories of `docs/dogfood/` at the reports' commits, cloned fresh, one `scripts/heavy.sh` invocation per run; no tool ran (each envelope's `toolchain` is empty). The reports judged 368 clone rows true (ADR-138 §3: the stated similarity is correct). A true row is *lost* when its location is a member of a clone group before and of none after. Each cell gives pairs / clusters / findings, then true rows lost.

`min_nodes` 23 alone, the decision (`dee39a5` with `min_nodes = 23` by `--config`, which is the shipped change, measured):

| Repository | True rows | Before (20) | 23 | True rows lost |
| --- | --- | --- | --- | --- |
| apache/commons-cli | 48 | 78 / 6 / 48 | 66 / 5 / 41 | 7 |
| cljfx/cljfx | 12 | 14 / 1 / 12 | 13 / 1 / 11 | 1 |
| gvergnaud/ts-pattern | 46 | 167 / 14 / 46 | 167 / 14 / 46 | 0 |
| more-itertools | 42 | 56 / 3 / 42 | 56 / 3 / 42 | 0 |
| PyCQA/pyflakes | 4 | 132 / 14 / 26 | 121 / 13 / 24 | 0 |
| rust-itertools | 66 | 98 / 11 / 67 | 98 / 11 / 67 | 0 |
| serde-rs/json | 86 | 586 / 20 / 102 | 586 / 20 / 102 | 0 |
| spf13/cobra | 64 | 207 / 31 / 65 | 206 / 31 / 64 | 1 |
| **Total** | 368 | 1 338 / 100 / 408 | 1 313 / 98 / 396 | **9** |

The 9 true rows lost at 23, every member 20 to 22 nodes:

- commons-cli `CommandLine.Builder.addArg` / `addOption` (22), `CommandLine.addArg` / `addOption` (`if (x != null) { xs.add(x); }`, 20), `setDeprecatedHandler` in `CommandLine.Builder` and `DefaultParser.Builder` (21), `HelpFormatter.Builder.setShowDeprecated` / `OptionFormatter.Builder.setDeprecatedFormatFunction` (22), `OptionFormatter.Builder`'s four `set…` setters (21, a cluster), `OptionTest.testBuilderInvalidOptionName1` / `2` (21), `ValueTest.testLongNoArg` / `testShortNoArg` (22);
- cljfx `table-sort-policy` / `tree-table-sort-policy` (one `cond`, 21);
- cobra `Command.preRun` / `postRun` (one `for`, 22).

The two pyflakes groups that go (22 and 20 nodes) were rows the report judged disputed.

The single-statement rule, rejected (`dee39a5` with the rule, at `min_nodes` 20, 23, 25, and 30):

| Repository | True rows | Rule, 20 | Rule, 23 | Rule, 25 | Rule, 30 |
| --- | --- | --- | --- | --- | --- |
| apache/commons-cli | 48 | 74 / 6 / 44, 4 | 64 / 5 / 39, 9 | 64 / 5 / 39, 9 | 61 / 5 / 36, 12 |
| cljfx/cljfx | 12 | 14 / 1 / 12, 0 | 13 / 1 / 11, 1 | 12 / 1 / 10, 2 | 11 / 1 / 9, 3 |
| gvergnaud/ts-pattern | 46 | 148 / 11 / 35, 11 | 148 / 11 / 35, 11 | 148 / 11 / 35, 11 | 147 / 11 / 34, 12 |
| more-itertools | 42 | 52 / 3 / 38, 4 | 52 / 3 / 38, 4 | 52 / 3 / 38, 4 | 50 / 3 / 36, 6 |
| PyCQA/pyflakes | 4 | 122 / 13 / 25, 0 | 121 / 13 / 24, 0 | 79 / 9 / 20, 0 | 74 / 8 / 17, 0 |
| rust-itertools | 66 | 37 / 4 / 29, 37 | 37 / 4 / 29, 37 | 37 / 4 / 29, 37 | 37 / 4 / 29, 37 |
| serde-rs/json | 86 | 140 / 5 / 36, 54 | 140 / 5 / 36, 54 | 140 / 5 / 36, 54 | 139 / 5 / 35, 55 |
| spf13/cobra | 64 | 197 / 29 / 59, 6 | 197 / 29 / 59, 6 | 197 / 29 / 59, 6 | 197 / 29 / 59, 6 |
| **Total** | 368 | 784 / 72 / 278, 116 | 772 / 71 / 271, 122 | 729 / 67 / 266, 123 | 716 / 66 / 255, 131 |

`min_nodes` alone, estimated from the before envelopes (a group survives when at least two of its members reach the floor), loses 10 true rows at 25 and 24 at 30.

- The 116 rows the rule alone loses have members of 20 to 1 813 nodes, median 51; 11 are over 100 nodes. They are one statement that holds the whole body:
  - a `match` (serde_json's `Deserializer` methods);
  - a `loop` (itertools `FlattenOk::next`/`next_back`, 162 nodes, 33 lines);
  - a `for` (more-itertools `TestRunningMin`/`Max.test_basic`, 125 nodes, 20 lines);
  - a call with a closure (itertools' bench functions);
  - a `return match(…)…` chain (ts-pattern's benchmark `handle`, 1 813 nodes).
  
  Many are duplication a maintainer would extract.

On Cupel's own tree (`cupel.toml`'s scope, which leaves out the fixtures) the before binary finds 162 pairs and 17 clusters (the baseline holds 164 and 17).

- **At `min_nodes` 23:** 155 pairs and 17 clusters. The seven pairs that go are all 20 to 22 nodes: `outcome::status` / `format::default_for`, `Process::terminate` with a test's `Run::scope`, `TestResults::with_reuse` / `NormNode::at`, two `Repo::new`s, two `json_schema`s, two test-support helpers, and two normalizer tests.
- **Under the rejected rule:** 87 pairs and 11 clusters, removing 39 groups. Among them:

  - both `as_str` accessor clusters (`OracleKind::as_str` with `Setting::key` and `env_var`, 35 nodes; `Format::as_str` with `TimeUnit::suffix`, `CostClass::est_ms_per_target`, and `OracleState::as_str`, 39 to 42 nodes);
  - the pair `TestResults::with_reuse` (now the struct update `Self { reuse: Some(reuse), ..self }`, one statement, 21 nodes) with `NormNode::at`;
  - the check constructors `StaticClones::new` and its kin (one struct expression, 85 to 127 nodes);
  - Rust tests that are one `assert_eq!` (92 to 121 nodes).

What each recorded case needs:

- `SuitePhase::as_str` as a four-arm `match` is one statement of the same 35-to-42-node shape, so only the rule skips it.
- `with_reuse` as first written (`self.reuse = Some(reuse); self`, two statements, 22 nodes) is not one statement, so only `min_nodes` of at least 23 skips it.

ADR-138 §3 calls a measurement true when the value it states about the code is correct, whatever a maintainer would do, so a correctly measured trivial setter pair is a true row. No setting of the two rules keeps every true row. The options put to the owner:

- **(A) The first ruling: the rule and `min_nodes` 23.** Rejected.
  - It skips both recorded cases.
  - It loses 122 true rows: 116 to the rule, 6 more to the floor.
- **(B) `min_nodes` 23 alone.** Chosen.
  - It skips `with_reuse` as first written.
  - It loses 9 true rows, all of 20 to 22 nodes: setters, guards, and two two-line tests, the kind ruled out.
  - It keeps the `as_str` clusters and every large single-statement clone.
- **(C) The rule limited to small single statements (below about 50 nodes) and `min_nodes` 23.** Rejected.
  - It skips both recorded cases.
  - It would keep the large single-statement clones.
  - It is not measured, and its cut is one more number to choose.
  - Deferred by the owner's v0.2.3 ruling: not measured, no rerun; future tuning starts from §2's tables.

## Decision

1. **`tests.flaky` on a runner whose order cannot matter is `skipped`, `unsupported`** (option (d)).
   - `cupel_core::Error` gains `Unsupported { fact, tool, message }`: a producer cannot make `fact` here, for a reason only the run shows.
   - The scheduler reports a check that ends in it as `skipped` with `reason: unsupported`, `fact`, `tool`, `lang`, and `see: cupel describe <check>`. The message goes into the check's notes as an error's would. It is not an error and does not make the run partial.
   - `engine.flake` returns it for a baseline whose runner's ordering is `Fixed`: fact `test-reruns`, the runner's tool (`cargo-nextest`), and "engine.flake does not rerun tool.cargo-nextest: it runs each test in its own process, so the order of the tests cannot change an outcome".
   - A baseline runner that is not registered at all stays `Error::InvalidValue`: that is a broken configuration. The engine's behaviour on every runner it reruns is unchanged.
2. **`static.clones`'s default `min_nodes` is 23; there is no single-statement rule** (option (B)). ADR-050's other settings (threshold 0.82, `min_lines` 4) and ADR-136's similarity are unchanged. The setting still configures per check and language.
3. **A clone group whose every member is an enum mapping is reported at `info`, with the evidence `shape: "enum-mapping"`, and is left out of `pairs`, `clusters`, and `max_jaccard`** (amended 2026-11-01, v0.2.3, on the owner's ruling: "keep min_nodes 23. For pairs whose forms are enum-variant mapping matches (as_str-style), set evidence shape = "enum-mapping" and severity info").
   - *An enum mapping* is a symbol whose body, past constants and punctuation one node at a time, reaches a *table*: two or more *arms* of one label, every other child a constant or punctuation, and a constant (what is matched) beside the arms or beside the table. An arm is a pattern followed by exactly one constant besides punctuation; a pattern that is a lone identifier is a field's name (`col: 1` in a struct or object literal), not a case. A constant is an identifier or literal marker, a node with no children (a keyword such as `self`), a path of identifiers (`Kind::Ref`), or one constant with only punctuation beside it (`return "ref"`, `-1`); a call (its callee is a named leaf) and an operation on two operands are not. The body is the `NormTree` node whose span is the body `Symbols` names, so a symbol without one (Clojure) is never a mapping.
   - The test reads shapes only, so `static.clones` still names no language. On the normalized trees of the four forms it holds for Rust's `match self { Self::Ref => "ref", … }` (`match_block` of `match_arm`s, `self` beside it), Python's `match k: case K.REF: return "ref"`, Go's `switch k { case Ref: return "ref" }` followed by `return ""`, and TypeScript's `switch (k) { case K.Ref: return "ref"; }`; it does not hold when an arm calls (`Self::Quick => x.max(1)`) or computes (`y + x`), for a struct literal of constant fields, or for TypeScript's `case Ref:` on a bare identifier, which reads as a field. Go's bare `case Ref:` holds, since Go wraps the case's values in a list. Checked on real Go, Python, and TypeScript files of three six-arm mappings each: one `info` cluster shaped `enum-mapping` per language.
   - The table must sit below the body, so a body that is a list of statements is not one, and an assignment to a lone name (`a = 1`) is not an arm. Checked on real files, each three times over: a Rust struct literal of constant fields (the `Span { line, col: 1, end_line, end_col: 1 }` test helper that paired on Cupel's tree), an `if` whose block assigns constants to five fields (Rust and Python), a Python function returning a dictionary of six constants, and a TypeScript mapping on bare `case A:` values: each pairs at `warn` without a shape. What the test cannot tell apart from a mapping is any other construct of two or more `pattern, constant` children beside a constant; none has been seen.
   - A group is shaped `enum-mapping` only when every member is a mapping; a pair's evidence says so when both are. Its findings are `info`, so the changed gate's `"static.clones" = "none"` (which counts findings above `info`) passes over them, and the check's status is `fail` only for a group that is not one. `pairs`, `clusters`, and `max_jaccard` count only the other groups, so the tree gate's `<= base` ratchets do not move when an accessor joins a mapping cluster, which is what forced B.1's rewrite of `SuitePhase::as_str`. The score (pairs found) and the `cluster_size` measurements still include them.
   - Fixtures: `repos/clone-mapping/rust`, three `pub const fn as_str(&self) -> &'static str` accessors of two arms, 30 nodes each, are one cluster at `info` shaped `enum-mapping`; `repos/clone-floor/rust`'s 35-node accessor pair (§2's case that must still pair) still pairs, now at `info` with the shape, and its setters at `min_nodes` 20 pair at `warn` without one.
   - On Cupel's own tree: three groups are mappings at `info`: the pair `differential::harness::Language::id` and `config::gate::Comparison::symbol`, and the two `as_str` clusters of three and four accessors that B.1 met, 10 pairs in all. So at the same score (155 pairs found) `pairs` falls from 155 to 145 and `clusters` from 17 to 15. The tree dogfood also found the one false positive above, the `Span` literal helper, which pairs at `warn` again.

## Consequences

- A nextest project, Cupel's own included, reads `tests.flaky` as skipped, with the fact it lacks, the runner's tool, and why. A gate rule on it is skipped, not failed, as for any `unsupported` skip. Cupel's dogfood still passes `--skip tests.flaky` to save the baseline run.
- `Skipped.tool` now names the tool an `unsupported` skip concerns as well as an `unavailable` skip's missing tool; the schema's field is unchanged.
- ADR-078's Consequences sentence "the engine does not rerun it and says so" now says so as a skip.
- A four-line setter or guard of 20 to 22 nodes no longer pairs: `with_reuse` as first written (`self.reuse = Some(reuse); self`, 22 nodes) would not have joined its cluster, and its struct-update rewrite (21 nodes) no longer pairs with `NormNode::at`.
- `SuitePhase::as_str`-style accessors still pair: a single-`match` body of 35 to 42 nodes is above the floor, so Cupel's two `as_str` clusters remain and a new accessor of that shape still joins one. Avoiding that needs the rewrite B.1 made (a name table), or a later decision. §3 is that decision: they still pair, at `info`, outside the counts the gate ratchets.
- Cupel's tree at 23 has 155 pairs and 17 clusters against the baseline's 164 and 17, so `[gate.tree]`'s `<= base` rules pass; `.cupel/baseline.json` is refreshed on `main` after the merge.
- Future tuning starts from the tables above: raising the floor to 25 costs about one more true row, and to 30 about fifteen more (estimated from the before envelopes). A size-limited single-statement rule (C) stays unmeasured.
