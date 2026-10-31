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

**§2, the data** (static.clones at dee39a5 and with this change on the eight repositories of `docs/dogfood/`, at the reports' commits; `check --only static.clones --no-install` through `scripts/heavy.sh`, no tool run).

The report rows judged true at 20 to 25 nodes are the same shapes the ruling excludes:

- commons-cli `CommandLine.Builder.setDeprecatedHandler` and its `DefaultParser` twin (`this.x = x; return this;`, 21 nodes, type 1).
- commons-cli `OptionFormatter.Builder`'s four `set…Prefix` setters (21 nodes, a cluster).
- commons-cli `CommandLine.addArg` / `addOption` (`if (x != null) { xs.add(x); }`, 20 nodes, one statement).
- cobra `Command.preRun` / `postRun` (one `for` statement, 22 nodes).
- cljfx `table-sort-policy` (one `cond`, 21 nodes).
- serde_json `test_write_str` / `test_write_bool` (three statements, 25 nodes).

ADR-138 §3 calls a measurement true when the value it states about the code is correct, whatever a maintainer would do, so a correctly measured trivial setter pair is a true row. The ruling's two rules therefore cannot keep every true row: removing such pairs is what the ruling asks for. The before/after table and the rows each setting loses are in the plan's B.4 Notes; §2's Decision waits on the owner choosing between them.

## Decision

1. **`tests.flaky` on a runner whose order cannot matter is `skipped`, `unsupported`** (option (d)).
   - `cupel_core::Error` gains `Unsupported { fact, tool, message }`: a producer cannot make `fact` here, for a reason only the run shows.
   - The scheduler reports a check that ends in it as `skipped` with `reason: unsupported`, `fact`, `tool`, `lang`, and `see: cupel describe <check>`. The message goes into the check's notes as an error's would. It is not an error and does not make the run partial.
   - `engine.flake` returns it for a baseline whose runner's ordering is `Fixed`: fact `test-reruns`, the runner's tool (`cargo-nextest`), and "engine.flake does not rerun tool.cargo-nextest: it runs each test in its own process, so the order of the tests cannot change an outcome".
   - A baseline runner that is not registered at all stays `Error::InvalidValue`: that is a broken configuration. The engine's behaviour on every runner it reruns is unchanged.
2. **`static.clones`**: proposed, pending the owner's ruling (see Context and the plan's B.4 Notes).

## Consequences

- A nextest project, Cupel's own included, reads `tests.flaky` as skipped, with the fact it lacks, the runner's tool, and why. A gate rule on it is skipped, not failed, as for any `unsupported` skip. Cupel's dogfood still passes `--skip tests.flaky` to save the baseline run.
- `Skipped.tool` now names the tool an `unsupported` skip concerns as well as an `unavailable` skip's missing tool; the schema's field is unchanged.
- ADR-078's Consequences sentence "the engine does not rerun it and says so" now says so as a skip.
