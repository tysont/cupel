# CUPEL_DECISION_206: A finding is resolved only when the compared run could have reported it again

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.9, item V1 · Details [046](CUPEL_DECISION_046.md), [131](CUPEL_DECISION_131.md), [142](CUPEL_DECISION_142.md)

## Context
ADR-046 §6 defines *resolved* as the reference run's findings whose ID the current run lacks. That holds when both runs looked at the same things. The v0.2.9 validation on hashicorp/raft set a baseline on a whole-tree `survey --depth quick` (268 findings), then ran `check --changed --gate` on a one-file pull request: `summary.resolved` was 268 and `baseline diff` listed every baseline finding as resolved. The changed run never looked at the files those findings are in, or never ran their checks. ADR-131 carries findings across symbol matches but still compares against the whole reference.

A run can fail to report a reference finding again for four reasons: its check did not run (absent from `checks`), ran but did not finish (`skipped`, `error`, or `partial`: cut short by the budget or a cancel), the finding's file is outside the run's scope, or the finding's symbol is in a file in scope but outside the run's symbols. The last is the common one: under a range only the symbols the changed lines touch are in scope (ADR-142), so a change that adds one function to a file leaves that file's other functions out, and their findings are not reported. `ScopeInfo` records counts only, so `baseline diff`, which compares stored runs, cannot tell which files or symbols a run covered.

Open: what "could have reported it" means for each reason, what to do with findings of no check (`doctor`, `init`, `cli`), what a run records so a stored run compares the same way, and whether score deltas follow.

## Options
**Which checks count as having run.** (a) Every check in `checks`. (b) Those that passed or failed; a `partial` check stopped before its targets were done, so a finding it did not report may simply not have been reached.

**How a run records its files.** (a) The files of the scope (`ScopeInfo.paths`), absent for a `tree` run, which covered every file. (b) A glob or root list: smaller, but a `--changed` scope is a list of files, not a pattern.

**How a run records its symbols.** (a) The symbols it kept, by display. A function the change deleted is in no head scope, so its finding would never resolve, though deleting a function is the plainest way to resolve its findings. (b) The symbols of the considered files it left out, by display: a finding on any other symbol of a considered file (one in scope, one edited, one deleted) can be resolved. Displays rather than structural hashes, because an edited function's hash changes and the reference finding carries the old one. (c) Hashes: miss every edit.

**Findings of no check.** `doctor`, `init`, `cli`, and `gate` findings name a `check` that is no entry of `checks`. (a) Never resolved, since no check of theirs ran. (b) Compared by ID as before, subject to the scope rules; the verb that reports them reports them whenever it runs.

## Decision
1. **Reach.** A reference finding the current run lacks is resolved only when the current run could have reported it again:
   - **Check:** when the finding's check is an entry of the reference run's `checks`, the current run has an entry for it with status `pass` or `fail`. A finding whose check is no entry of the reference's `checks` (a verb's own, such as `doctor.tool-missing`) needs no check (Option b).
   - **File:** when the current run's `scope.mode` is not `tree`, the finding has a `location` whose `file` is in the current run's `scope.paths`. A finding without a location is resolved only by a `tree` run. A run that recorded no `scope` (`doctor`, `baseline`) limits nothing.
   - **Symbol:** when the scope is not `tree`, the finding's `symbol.display` is not in the current run's `scope.symbols_left_out`.
   New findings are unchanged: a finding the current run reports and the reference lacks is new wherever it is.
2. **Scores.** `checks[].delta` is computed only for a check scored and with status `pass` or `fail` in both runs.
3. **What a run records.** `ScopeInfo` gains two optional properties, absent when empty (`x-cupel-optional`, as ADR-079 and ADR-111):
   - `paths`: for a mode other than `tree`, the scope's files together with the files its range deleted (`ResolvedFiles::considered`), sorted. A deleted file's findings are resolved: nothing can be reported in it again.
   - `symbols_left_out`: the displays of the symbols of the files in scope that the range (ADR-142) or the `--symbols` selectors left out, sorted (`Outcome::symbols_left_out`). Empty when neither narrows the symbols.
   `tests --impacted` records `paths` and no symbols, since it runs no checks.
4. **Where.** `cupel_core::deltas_matched` applies the rules from the current envelope alone, so `check --against`, `survey`, and `baseline diff` (which compares two stored runs) behave the same.

## Consequences
- `check --changed` against a whole-tree baseline reports as resolved only findings on the symbols its change touched or deleted, and in the files it deleted; `baseline diff` after it lists the same.
- A stored run recorded before this decision with a non-tree mode has no `paths`, so used as the current run it resolves nothing with a location. A reference run needs no new fields: the rules read only the current run's scope.
- An envelope of a `--paths` or `--symbols` run over a large tree lists every file in scope, and a `--symbols` run lists every other symbol's display; these modes are rare in agent loops, which use `--changed`.
- Symbols are named by display: two symbols of one display in one run's considered files (overloads) are left out or kept together.
- A check whose findings land outside its scope (an importer reached through the module graph) is still compared by the finding's own file: such a reference finding outside `paths` is not resolved by a scoped run, which errs toward reporting less as resolved.
