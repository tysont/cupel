# CUPEL_DECISION_047: Suppressions: file format, expiry, and how they change an envelope

**Date:** 2026-10-13 · **Status:** accepted · **Phase:** P2.2

## Context

Design §6.5: `suppress <finding-id> --reason TEXT [--until REF|DATE]` writes `.cupel/suppressions.toml` (committable); suppressed findings still appear with `suppressed: true` and `severity: info`; expired suppressions reactivate and emit `suppression.expired`. §8.8 mirrors suppressions in the store. The file's format, the date format, what "until REF" means, which runs suppressions apply to, what a suppression does to the exit code, and what the expiry finding looks like are not specified.

## Options

**`--until REF`.** Expire when the ref exists (a tag being cut), or when the ref's commit is an ancestor of `HEAD` (the release or branch has been merged into what is being graded). The second covers the first for tags on the main line and also covers branches.

**Unknown finding IDs.** Accept any well-formed ID (lets a team pre-suppress; a typo silently suppresses nothing) or require the ID to be a finding the store has seen (a typo is exit 2).

**Exit codes.** Suppression never changes an exit code, or a suppressed problem stops counting toward the outcome it would have caused.

## Decision

1. **File.** `.cupel/suppressions.toml`, the one file `.cupel/.gitignore` keeps, written through the `Writer` (`--dry-run` shows its diff). Format:

   ```toml
   # Cupel suppressions: written by `cupel suppress <finding-id> --reason TEXT [--until DATE|REF]`.
   [[suppression]]
   finding = "fnd_7c2e1a"
   reason = "known flake"
   until = "2026-12-31"
   created = "2026-10-13"
   ```

   `until` is optional. Entries are sorted by finding ID; suppressing an already suppressed ID replaces its entry. Unknown keys and malformed IDs or dates are configuration errors (exit 2, subject `.cupel/suppressions.toml`). The store's `suppressions` table mirrors the file on every `suppress` and every recorded run; the file is the source of truth.
2. **Known findings only.** `suppress` requires a finding the store has recorded (in any run); otherwise exit 2 with subject `FINDING`. Its envelope carries that finding as last stored, marked suppressed.
3. **`--until`.** A value of the form `YYYY-MM-DD` is a date (UTC); anything else is a git ref. A date suppression is active through that date and expired from the next day. A ref suppression is expired once `git rev-parse --verify <ref>^{commit}` resolves and `git merge-base --is-ancestor <ref> HEAD` succeeds; while the ref does not resolve, or outside a git repository, it stays active. "Today" is the date of the run's `run.started`.
4. **Applying.** `cupel_core::apply_suppressions(&mut Envelope, &[Suppression], expired: impl Fn(&Until) -> bool)` changes, for each finding with an active suppression, `suppressed` to `true` and `severity` to `info`; recounts `summary.findings` and `total_findings`; sets `summary.suppressed`; and for each finding whose suppression has expired, leaves the finding as it is and appends one `suppression.expired` finding: check `suppress`, subject the suppressed finding's ID (ADR-016), `warn`, tier 0, confidence 1, action `review` ("Fix the finding, or suppress it again with a later --until"), evidence `{finding, reason, until}`. Already-suppressed findings are left alone, so applying twice changes nothing. Suppressions for findings absent from the envelope do nothing.
5. **Where.** The CLI applies suppressions to the envelope of every completed run of `init`, `doctor`, `check`, `survey`, and `tests`, before deltas, events, and the run record. Error envelopes and the bookkeeping verbs (`describe`, `explain`, `baseline`, `suppress`) are left as they are.
6. **Exit codes.** Suppressions do not change `status` or `exit_code`, with one rule: an envelope that exits 3 (environment) and has no unsuppressed `fail` finding left becomes `pass`, exit 0, because exit 3 means its fail findings are the environment problem (ADR-022 §5) and the user has accepted them. Gate evaluation (P2.9) ignores suppressed findings by reading severities after suppression.

## Consequences

- A suppression is reviewable in a pull request as one TOML entry with a reason.
- An expiry is visible: the reactivated finding plus a `suppression.expired` warning, until the entry is removed or renewed.
- Ref expiry costs two `git` calls per ref suppression per run; date expiry costs nothing.
