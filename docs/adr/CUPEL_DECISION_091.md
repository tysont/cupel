# CUPEL_DECISION_091: Results against an oracle: selection, `checks[].oracle`, the tier cap, and the `oracles` member

**Date:** 2026-10-16 · **Status:** accepted · **Phase:** P5.2

## Context

Design §6.5: "A `gen.differential` or `spec.trace` result against an unverified oracle is capped at tier 3 and carries `warn: oracle.unverified`." §6.4: `--oracle NAME # for gen.differential; default: first verified`. §7.1's check result has an `oracle` member, null until now. ADR-090 decides what an oracle is and when it is verified. Still open: how a check says it compares against an oracle, which oracle a run uses, where the cap is applied so that every consumer of the envelope (renderers, the gate, `next_actions`, the store, `explain`) sees the capped tier, which check the finding belongs to and how `describe finding` knows it, and where `oracle list` puts its answer in a fixed-key envelope (ADR-013).

No check in this step compares against an oracle; `gen.differential` (P5.3) is the first.

## Options

**Declaring oracle use.** A manifest field changes every `describe <check>` output and the plugin manifest schema P5.1 is extending; a `Check` trait method with a default (as `base_needs` and `spans_family` are) changes neither.

**Where the cap lives.** In each check (every check would repeat it, and could forget), in the scheduler (it would need the store's verification, which `cupel-core` does not read), or in the CLI's grading of a finished envelope, where suppressions, deltas, the gate, and `next_actions` are already applied in one place before anything renders or is stored.

**The finding's check.** The check that ran against the oracle (each such check would declare the kind; none exists yet, so `describe finding oracle.unverified` would fail until P5.3), or `oracle`, as `suppression.expired` belongs to `suppress`: one finding per oracle, whatever checks used it, because the fix — verify that oracle — is one action.

**Where `oracle` verbs report.** Info findings per oracle (findings are observations to act on, not a listing), `description` (reserved for `describe`), or a verb-specific member null for every other verb, as `explanation`, `rankings`, and `tests` are.

## Decision

1. **`Check::uses_oracle(&self) -> bool`**, default `false`: the check compares against the run's oracle. Plugins cannot declare it until the plugin manifest carries it (consequences).

2. **Selection** (`check` and `survey`, `cupel-cli/src/check.rs`). `--oracle NAME` must name a defined oracle, else exit 2 listing the defined names; it is checked whenever given. When a selected check uses an oracle, the run's oracle is `--oracle`, else the first verified oracle by name, else the first defined by name. With no oracle defined, each such check is not scheduled and is reported skipped with `reason: unconfigured` and `see: "cupel describe oracle add"`. `--oracle` loses its `pending_step`.

3. **`checks[].oracle`** is the run's oracle name on every result, that ran, of a check that uses one; null otherwise. No envelope member is added for it.

4. **The cap is applied when the envelope is graded** (`app.rs` `graded`, for `check` and `survey`, first: before suppressions, deltas, the gate, and `next_actions`; plans pass through). For each result whose `oracle` is not `verified` now (ADR-090 §5 — `unverified`, `changed`, or `unreachable`), `tier` becomes `min(tier, 3)`, and one finding per such oracle is added: kind `oracle.unverified`, check `oracle`, subject the oracle's name (ADR-016), severity `warn`, confidence 1, tier 0, action `verify_oracle` ("Run `cupel oracle verify NAME`"), evidence `{oracle, state, checks}` (the IDs of the results it capped, sorted). Severity counts and `total_findings` are recounted. A result against a verified oracle keeps the tier its check declares. `--tier N+` still selects by the declared tier; the capped result reports 3.

5. **`describe finding oracle.unverified`.** `cupel_core::oracle::finding_kinds()` lists the kinds Cupel attaches to a run outside any check's manifest — `oracle.unverified` — described with check `oracle` and stability `experimental`; `describe finding` and its list of valid kinds include them after the checks' own.

6. **`Envelope.oracles`**, null for every verb but `oracle` (ADR-013's fixed key set gains one member, as ADR-062 and ADR-077 did): `[{name, definition, state, hash, verified_at, verified_run}]`, sorted by name. `definition` is the `OracleConfig`; `state` is `verified | unverified | changed | unreachable`; `hash` is the identity now (null when unreachable); `verified_at` and `verified_run` come from the store's verification, null without one. `oracle list` reports every defined oracle; `verify`, the oracles it verified or tried to, as they stand afterwards; `add` and `remove`, the oracle added or removed (in a preview too). The text renderer prints an `oracles` block.

## Consequences

- Every envelope gains `"oracles": null`; goldens and snapshots written before this step change by that line.
- `gen.differential` (P5.3) declares `uses_oracle`, receives the oracle's definition (the check context gains it then), and needs nothing else for the cap: a capped result and its finding come from grading.
- `spec.trace` (P7) is capped the same way once it declares `uses_oracle`.
- Suppressing an `oracle.unverified` finding hides the warning but not the cap; only `oracle verify` restores the tier.
- Plugin checks (P5.1) cannot use an oracle until their manifest can say so; adding it is one field mapped onto `uses_oracle`.
