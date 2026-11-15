# CUPEL_DECISION_218: A snapshot rewritten beside a code change is still a `tests.weakening` failure

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 3 · Confirms [057](CUPEL_DECISION_057.md)

## Context
ADR-057 makes `snapshot.rewritten` a `tests.weakening` finding at `fail`: a stored snapshot that existed at the base commit and that the range changed, attributed to the changed tests it belongs to, and not reported when a test the snapshot belongs to is new (the snapshot may only have grown). `"tests.weakening" = "none"` in `[gate.changed]` then fails on it.

The v0.2.9 validation replayed clap-rs/clap PR #6526 (a zsh escaping fix the maintainers merged). `check --changed --gate` failed `tests.weakening = "none"` with five `snapshot.rewritten`, one per shell: `clap_complete/tests/snapshots/sub_subcommands.{bash,elvish,fish,ps1,zsh}`. The range changed the completion generator and, in the same commits, rewrote the snapshots that record its output. The validation recommended keeping the failure: the rewrite is the part of the change a reviewer must read, and clap's maintainers did read it before merging. clap passed 11 of 12 replayed PRs; #6526 was the only failure.

The question put to the owner (Q-all-3): should a snapshot rewritten in the same range as a change to the code it records be reported at `info` instead?

The owner's ruling: "snapshot rewritten in the same range as a code change stays a tests.weakening failure. Per-kind severity remains configurable."

## Options
(a) Report a snapshot rewritten beside a change to the code it records at `info`. Cupel cannot tell which code a snapshot records, so in practice this would be any range that changes non-test code. That is nearly every range, and the weakening would pass unreviewed exactly when it matters most, when the expected output moved with the code. (b) Keep `fail` whatever else the range changed (ADR-057 unchanged). A project that regenerates snapshots routinely takes `tests.weakening` out of `[gate.changed]` or suppresses the findings of a reviewed PR.

Chosen: (b), the owner's ruling.

## Decision
1. Nothing changes. `snapshot.rewritten` stays `severity: fail`, action `restore_test`, whatever else the range changed. `tests.weakening = "none"` fails on it, so clap #6526 fails the gate, as ADR-057 intends.
2. **The second sentence of the ruling.** No Cupel setting gives a finding kind its own severity: `[checks."<id>"]` holds a check's declared settings, and `tests.weakening` declares none that touch `snapshot.rewritten`. A project changes the outcome with what the gate and suppressions already offer: leaving `tests.weakening` out of `[gate.changed]` (or gating another check in its place), or `cupel suppress <finding-id>` for a reviewed rewrite. A suppressed finding is `info` and does not count against `none` (ADR-060). This ADR adds no per-kind severity setting. Adding one is a design change of its own and needs a ruling on how it is written.

## Consequences
- clap #6526 still fails the replay's gate, which is the intended result. The validation report counts it as a true positive.
- No test is added: no behavior changed. The existing tests already pin the behavior: `cupel-fixtures/tests/weakening.rs` (`changed_reports_every_weakening_on_the_branch_and_ties_the_snapshot_to_its_test`, `a_snapshot_file_that_grew_with_a_new_test_is_not_a_rewrite`) and `cupel-checks/tests/tests_weakening.rs` (the snapshot attribution tests). No test proves per-kind severity is configurable, because Cupel has no such setting.
- Open for the owner: the ruling says per-kind severity "remains configurable", and Cupel has no such setting today. If one is wanted, it needs its own ruling on where it is written (for example `[checks."<id>".severity]` keyed by kind) and on how it combines with suppressions and the gate.
