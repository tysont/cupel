# CUPEL_DECISION_046: Baselines, `--against`, and how deltas are computed

**Date:** 2026-10-13 · **Status:** accepted · **Phase:** P2.2

## Context

Design §6.5 says `baseline set [--from run:ID] --write` names "the current (or given) run" as baseline, `diff [--against run:ID|REF]` "compares two runs", and `export`/`import` serialize a baseline as JSON; §6.4 adds `--against baseline|REF|run:ID` ("default baseline if set") and `--since-baseline`; §7.1–7.2 give `run.baseline`, `checks[].delta`, `summary.new/resolved`, and `new_since_baseline`. Left open: which run "current" is when every verb keeps a run record (a `baseline show` is itself a run), what `set` does without `--write`, what a git ref resolves to, what "new" and "resolved" mean exactly, and what the envelope of `diff` holds.

## Options

**Which run is "current".** The most recent run of any verb (then `baseline set` right after `baseline show` would baseline the `show`), or the most recent run that graded something. Usage errors and internal errors grade nothing.

**`set` without `--write`.** The baseline lives in `.cupel/cupel.db`, which the `Writer` allows, so `--write` is not needed for the file system; but the design requires it, because a baseline changes what every later comparison uses. Without it: exit 2, or a preview as `init` gives (ADR-021 §5).

**A git ref.** Resolve the ref to a commit and use the latest stored run made at that commit (needs `run.git.sha`), or re-run at that ref (the design says deltas and `explain` never re-run).

## Decision

1. **Result runs.** A *result run* is a stored run of `init`, `doctor`, `check`, `survey`, or `tests` whose exit code is not 2 (usage) or 6 (internal). `describe`, `explain`, `baseline`, `suppress`, `oracle`, and `serve` runs, and runs that failed before doing their work, are never chosen by default. `baseline set` without `--from` uses the latest result run; `--from run:ID` may name any stored run.
2. **Names.** Baselines are rows `baselines(name, run_id)`; `--name` picks one (default `default`). `run.baseline` is `{id, name}`.
3. **Previews.** `baseline set` and `baseline import` change a baseline, so without `--write` they are previews like `init`'s: a `plan` envelope whose `would_write` has one entry for `.cupel/cupel.db` with a unified diff of the baseline row (`baselines/<name>`, old run ID to new), a `next_actions` entry with the `--write` command, and no run record. `--dry-run` gives the same plan. `export FILE` writes through the run's `Writer`, so a file outside `.cupel/` needs `--write` and `--dry-run` shows its diff; `show` and `diff` write nothing but their run record.
4. **`show`.** An envelope with `run.baseline` set and the baseline run's `checks` and `findings`; status `pass`, exit 0. No baseline of that name: exit 2 naming `baseline set --write`.
5. **References.** `--against` (and `diff --against`) accepts `baseline` (the `--name` baseline, default `default`), `run:<id>` (a stored run), or anything else as a git ref: `git rev-parse --verify <ref>^{commit}`, then the latest stored result run whose `run.git.sha` is that commit. An unknown run, an unset baseline, a ref git cannot resolve, and a ref with no stored run are usage errors (exit 2) with subject `--against`. Nothing is ever re-run. Without `--against`, `check` and `survey` compare with the `default` baseline when it is set and compute no deltas otherwise (`run.baseline` null, every `new_since_baseline` false).
6. **Deltas.** `cupel_core::deltas(current, against) -> Deltas` compares findings by `FindingId`: *new* are the current findings whose ID the reference lacks; *resolved* are the reference's findings whose ID the current run lacks. `Deltas::apply(&self, &mut current, baseline, vs)` sets each finding's `new_since_baseline`, `summary.new` and `summary.resolved` to the two counts, `run.baseline`, and `checks[].delta = {vs, value: current score − reference score}` for every check scored in both runs. `vs` is `baseline`, `run:<id>`, or the ref as written.
7. **`diff`.** Compares the latest result run (current) with the reference (default: the baseline). Its envelope's `findings` are the new findings (`new_since_baseline: true`) followed by the resolved ones (`new_since_baseline: false`, as stored in the reference run); `summary.new`/`resolved` count them; `checks` are the current run's with deltas; `run.baseline` names the reference. Status `pass`, exit 0: a diff reports, it does not gate.
8. **Export and import.** The file is the baseline run's complete envelope as pretty JSON with a trailing newline, the same bytes as its run record. `import` parses it as an `Envelope`, records the run in the store (replacing a stored run of the same ID), and points the named baseline at it.
9. **`--since-baseline`.** The store answers "which of these symbol hashes are new since run R" (`Store::symbols_new_since`, from `symbols.first_seen_run`). Turning that into a scope is scope resolution, which is P2.1's; the flag stays pending P2.1.

## Consequences

- `baseline set --write` right after any `doctor` or `check` does what a user means, whatever bookkeeping verbs ran in between.
- Refs work only for runs that recorded `run.git.sha`; until the run path fills `run.git` (P2.1 with ADR-002), a ref resolves to no stored run and is a usage error saying so.
- P2.1's `check` and P2.9's `survey` get deltas from the CLI's `against::compare` after their checks run; P2.9 ranks `next_actions` with `new_since_baseline`.
