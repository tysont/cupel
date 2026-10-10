# CUPEL_DECISION_017: How the CLI reports descriptions, usage errors, and internal errors

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P0.3

## Context

Design §6.1 rule 1 says every verb returns the §7.1 envelope, and P0.3 requires that every error path — including a command line clap cannot parse and a panic — produces a valid envelope on stdout. Writing the CLI exposes questions the design does not answer:

1. The envelope has no field for what `describe` returns. Later verbs add their own payloads too (`rankings` for `survey`, `tests` for `tests --impacted`, the dry-run plan of §6.8).
2. `describe schema <name>` is specified as "JSON Schema (draft 2020-12)": a bare schema document or an envelope?
3. `run.verb` is required, but `cupel` with no arguments or with an unknown verb has no verb.
4. Which severity, action kind, and evidence the CLI's own findings carry, and what the "bug-report payload" of exit 6 contains.
5. What to do when a format is requested whose renderer does not exist yet (`jsonl` is P0.5, `sarif` is P6.1), and which format a usage error renders in when the command line, including `--format`, did not parse.
6. How "both clap help and `describe` render from one registry" works when the step also asks for clap derive.
7. How to reach exit 6 deliberately in tests, and what `--help`/`--version` print.
8. Which entities are `stable` and which `experimental`.

## Options

For (1): emit verb payloads at the top level with `#[serde(flatten)]` (matches the design's sketches of `rankings`, but breaks the fixed key set of ADR-013 and makes the run schema open-ended), wrap them in one generic `result: any` field (fixed keys, but the schema says nothing), or add one nullable, typed top-level field per payload (fixed keys and a precise schema; each new payload is a schema change reviewed through the snapshot). For (2): a bare schema is easiest to feed a validator, but it would be the only output that is not an envelope, and its errors would be envelopes anyway; inside the envelope it is one `jq` path away. For (3): a placeholder verb would lie; `null` is honest. For (6): build clap entirely from the registry with the builder API (one source, but loses derive's typed parsing, which the step asks for), or keep derive for parsing and apply all human text from the registry at runtime, with tests that compare clap's model and help output against the registry.

## Decision

1. `Envelope` gains a nullable `description` field holding the `describe` payload, a tagged union over `cli`, `command`, `schema`, and `all`. Later verb payloads follow the same pattern: one nullable, typed top-level field each.
2. `describe schema <name>` returns an envelope whose `description` carries the schema's name, URL, stability, and the schema itself. The list of names comes from `cupel_core::schema::PUBLISHED`.
3. `run.verb` is nullable; it is `null` only when the command line names no valid verb.
4. CLI findings (ADR-016 identity, `check: "cli"`) have `severity: fail`, `confidence: 1`, `tier: 0`, no symbol or location. `cli.usage` maps to `action.kind: configure` with a summary naming the fix (the valid values, the suggested spelling, or the missing argument); its evidence names the argument and, when known, the bad value and the valid values. `cli.not-implemented` maps to `review` and names the plan step in `detail`. `cli.internal` (exit 6) maps to `review`; its evidence is `{"bug_report": {version, os, arch, message, location, args, report}}`, where `report` is the issue-tracker URL. The envelope `status` of all three is `error`.
5. A requested but unbuilt format yields exit 2 with `cli.not-implemented`, rendered as JSON. When the command line does not parse, the format is recovered from the raw arguments (`--format X`/`--format=X`, then `CUPEL_FORMAT`, then the TTY rule), falling back to the TTY rule if the recovered value is itself invalid.
6. Clap derive owns parsing. The registry owns every human-facing string and the described types, enum values, defaults, and conflicts; `about` and `help` text are applied to the derived `Command` from the registry before parsing. A unit test walks clap's command tree and requires it to match the registry exactly (names, value-taking, enum values, conflicts), and an integration test requires every `--help` page to mention exactly the registered flags. Defaults are described but not set on clap, so configuration precedence (P0.4) can see an unset flag.
7. Setting `CUPEL_DEBUG_PANIC=1` makes the CLI panic after parsing; it exists so tests can prove the exit-6 path and is deliberately absent from `describe`. `--help` and `--version` print clap's human text to stdout and exit 0; `--describe` is the machine-readable form. `cupel` with no arguments prints help to stderr and a `cli.usage` envelope to stdout, exit 2.
8. Exit codes are `stable` (their table is part of the design's contract). Verbs, flags, and schemas are `experimental` until 1.0.

## Consequences

- Agents parse one shape for every invocation, including failures, and can branch on `run.verb == null`.
- Adding a verb payload is a reviewed change to the run schema snapshot, never an untyped blob.
- `describe --all` is one envelope; tools wanting a bare schema extract `.description.schema`.
- Flags and verbs whose behavior lands in a later step are visible in `describe` with `pending_step` naming that step; removing the marker is part of finishing the step.
