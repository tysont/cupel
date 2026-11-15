# CUPEL_DECISION_216: A run only its budget cut short may be the baseline

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 1 · Amends [195](CUPEL_DECISION_195.md) §4 and [198](CUPEL_DECISION_198.md); relies on [205](CUPEL_DECISION_205.md) §1

## Context
ADR-195 §4 lets `baseline set` and `baseline import` name only a run whose status is `pass`, and ADR-198 kept that. The v0.2.9 validation set baselines the way the owner's method says: `cupel survey --depth standard --budget 30m`, then `cupel baseline set --write`. On four of the eight repositories the survey ended `partial` (exit 5) only because the budget left work out, and `baseline set` refused it (`out/<repo>/baseline-set.json`: "The run … did not pass (status `partial`); a baseline names only a passing run on the default branch"), so those repositories had no baseline and `baseline diff` had no reference:

- httpx: `tests.coverage` skipped for the budget (`est_ms` 113 400) and `tests.mutation` cut short at the deadline;
- raft: `tests.mutation` cut short (851 of 1 309 mutants not run; the check's own note says "the run was cut short by --budget or --timeout");
- jackson-core: `tests.mutation` ran 923 of 10 594 mutants and `tests.coverage` was skipped for the budget;
- bytes: `tests.mutation` cut short at 30 minutes.

Every other non-passing entry in those surveys was a skip that never makes a run partial (`needs-range`, `unsupported`, `depth`). PR gates were unaffected, since a rule against `base` under `--changed` recomputes the base (ADR-175); only the baseline was lost.

The owner's ruling (Q-all-1): "a run whose only gaps are budget skips may set a baseline; rules on skipped checks get baseline.missing."

Open: what counts as a gap the budget made, since a check can be cut short by the budget, by `--timeout`, or by a cancellation, and the envelope does not always say which; and what the baseline holds for those checks.

## Options
1. What the budget made. (a) Only checks `skipped` with reason `budget`: raft and bytes, whose one gap is a `tests.mutation` cut short at the deadline, would still have no baseline. (b) Every `partial` run with a budget: a `--timeout` or a Ctrl-C under a budget would pass as the budget's. (c) A check skipped or cut short with reason `budget`, and a check cut short with no reason (a cancelled check, or mutants left `not-run`) when the run's `duration_ms` reached its `budget_ms`, the only time the budget's deadline can have cut it. Anything else is a gap the budget did not make: a `timeout` skip, a check cut short before the deadline or in a run with no budget, an errored check, or a `partial` run the budget left nothing out of.
2. What the baseline holds for those checks. (a) Their partial results, such as the score of the mutants that ran: a ratchet would then compare a full run with a sample. (b) Nothing: the gate's reference value already comes only from a check that ran to completion (`pass` or `fail`), so a rule against `base` on a check the baseline's budget skipped or cut short reads "the reference run did not report `<key>`", which ADR-205 §1 makes a `no reference:` skip with a `baseline.missing` finding at `info` that does not fail the gate.

## Decision
1. **(c).** `baseline set` and `baseline import` accept a run whose status is `pass`, or `partial` when every check that is not `pass` or `fail` is one of: skipped for a reason that never makes a run partial; skipped or cut short with reason `budget`; cut short with no reason in a run whose `duration_ms` is at least its `budget_ms`. At least one check must be a budget gap. The branch rule of ADR-195 §4 applies as before.
2. **Refusals** name the first gap the budget did not make: "The run … did not pass (status `partial`), and not only for its budget: `tests.flaky` was skipped (timeout); a baseline names only a passing run on the default branch", with "`<check>` was cut short before the budget ran out", "`<check>` was cut short, and the run had no budget", "`<check>` errored", or "the budget left out no check". A run with any other status is refused as before.
3. **(2b).** The baseline is the run as recorded; nothing is copied or filled in for the checks the budget left out. A rule against `base` on such a check goes through ADR-198's path: skipped, reason `no reference: the reference run did not report <key>`, one `baseline.missing` finding, and the gate passes if every other rule does. The rule compares again once a later baseline holds the check's result.

## Consequences
- The owner's method sets a baseline on httpx, raft, jackson-core and bytes. The survey itself still ends `partial` (exit 5): only what `baseline` accepts changes.
- A ratchet on a check that only a longer run completes (`tests.mutation.score >= base` on raft) stays skipped with `baseline.missing` until a run that completes it is made the baseline; `baseline diff` compares the findings the budgeted run did report.
- A `partial` check with no reason is credited to the budget only by the run's duration. A run under both `--timeout` and `--budget` whose timeout fires after the budget's deadline cannot tell the two apart; the budget fired first, so it is the budget's.
- Implemented in `cupel-cli/src/baseline.rs` (`unbudgeted`); the gate is unchanged. Tests: `cupel-cli/tests/baseline.rs` (`a_run_whose_only_gaps_the_budget_made_can_be_the_baseline`, `a_partial_run_with_a_gap_the_budget_did_not_make_is_refused`) and `cupel-cli/tests/gate.rs` (`a_rule_on_a_check_the_baseline_s_budget_left_out_is_skipped_with_baseline_missing`, a real `--budget 1ms` run made the baseline).
