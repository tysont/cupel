# CUPEL_DECISION_013: Output-contract details the design leaves open

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P0.2

## Context

Design §7 specifies the envelope, finding, and event shapes by example. Encoding the contract as types in P0.2 requires answers the examples do not give. Each is small; together they define how agents parse every Cupel output, so they are recorded once here.

## Options

Each point below had two realistic options; the one not chosen is named with its cost.

1. **Absent optional values.** Omit the key, or emit `null`. The design's example emits `"budget_ms": null`, `"skipped": null`, `"oracle": null` but omits `command` on a `next_actions` entry. Omitting keys makes agents probe for presence; always emitting them gives every object a fixed key set.
2. **`exit_code` encoding.** The design shows an integer; `ExitCode` also has names (`pass`, `usage`, …). Serialize as the integer, with names available through `describe`.
3. **Fields that do not apply to every verb.** `run.git`, `run.scope`, and `run.depth` are meaningless for `describe` or outside a git repository. Either invent placeholder values, or make them nullable.
4. **Per-check status.** The design shows `"status": "fail"` on a check result and a separate `skipped` object, but never lists the values. Reusing the envelope `Status` would allow `plan` on a check; a dedicated set can name `skipped`.
5. **Columns.** Design §6.1 rule 9 requires `end_line` always but says nothing about columns, and file-level findings have none.
6. **Enum value spelling.** `action.kind` is snake_case in the design; fact-kind and conformance construct names (`mutation-sites`, `early-return`, `arith-add-sub`) are kebab-case.
7. **Timestamps.** `run.started` is an RFC 3339 UTC string in the example. A typed timestamp needs a date-time crate no P0.2 code otherwise needs.
8. **Finding ID input encoding.** `blake3(check ∥ kind ∥ symbol.hash ∥ site_hash)` does not say how the parts are joined, which prefix forms are hashed, or what `site_hash` is for a finding about a whole symbol.
9. **Schemas for `config` and `plugin`.** `cupel_core::schema::{config, plugin}` are listed in P0.2, but the configuration model is P0.4 and the plugin request/event types are P5.1.

## Decision

1. Optional values are always emitted as `null`; every object has a fixed key set. The `next_actions` entries in the design example therefore gain `"command": null` / `"finding": null`.
2. `exit_code` serializes as the integer 0–6; names come from `ExitCode::name()`.
3. `run.git`, `run.scope`, `run.depth`, `run.budget_ms`, and `run.baseline` are nullable. `run.seed` is always present (design §6.3: always echoed).
4. Check results use `CheckStatus` = `pass | fail | partial | error | skipped`; `skipped` carries the reason in the `skipped` object.
5. `location.line` and `location.end_line` are required; `col` and `end_col` are nullable.
6. Enum values are kebab-case, except `action.kind`, which keeps the design's snake_case, and `config.sources`, which uses the literal `cupel.toml`.
7. `run.started` is a string documented as RFC 3339 UTC; the code that first produces it (P0.7) chooses the formatting crate.
8. The finding ID hashes the UTF-8 strings `check`, `kind`, `symbol.hash` (with its `sh_` prefix), and `site_hash` (empty when the finding concerns the whole symbol), each separated by a single NUL byte, and keeps the first three bytes as six lowercase hex digits after `fnd_`. NUL cannot occur in any of the parts, so the encoding is unambiguous. Findings with no symbol (CLI usage errors, `doctor`) need their own identity rule, decided in P0.3 where the first such finding appears.
9. `schema::plugin()` returns the schema of `Manifest` — the part of the plugin protocol that exists (`<plugin> describe`) — and grows to cover `RunRequest` and plugin events in P5.1. `schema::config()` is added in P0.4 with the configuration model.

## Consequences

- Agents can rely on key presence for every envelope object and branch on `null`.
- Snapshot-tested schemas pin these choices; changing one is a schema change visible in review.
- P0.3 must decide finding identity for symbol-less findings before emitting `cli.not-implemented`.
