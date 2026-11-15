# CUPEL_DECISION_232: clippy's E0554 is a note

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 5 (Q-rs-5) · Details [102](CUPEL_DECISION_102.md) (analyzers and their levels)

## Context
Validating v0.2.9 on tokio-rs/bytes, `static.analyze` reported three `analyze.clippy.E0554` findings, "`#![feature]` may not be used on the stable release channel", at line 1 of each nightly-only benchmark (`benches/buf.rs`, `benches/bytes.rs`, `benches/bytes_mut.rs`, which open with `#![feature(test)]`). rustc gives E0554 at level `error`, so each was a `fail` finding and failed the check (`out/bytes/survey.json`). bytes runs those benches on nightly; nothing in their code is wrong. NOTES Q-rs-5 recommended treating a target clippy cannot build on the active toolchain as an analyzer limitation.

The owner's ruling: "E0554 on nightly-only benches is info."

Open: how narrowly to apply it.

## Options
(a) **E0554 alone** is a note, at whatever level rustc gives it. The code means exactly "this target needs a nightly toolchain", wherever it occurs. (b) **Every error in a file with `#![feature]`.** On stable, rustc stops at the gate for bytes' benches, so (b) catches nothing (a) misses there; elsewhere it would also turn real errors (a type error in a nightly-only file that is wrong on nightly too) into notes. (c) **A new kind** (`analyze.target-unbuilt`, NOTES' suggestion): a new finding kind for one rustc code, and a gate rule or suppression written against `analyze.clippy.E0554` would stop matching. (d) **Mute the target** (leave benches out of `--all-targets`): would hide every other lint in a bench that does build.

Chosen: (a), the narrowest reading, keeping the kind the finding already has.

## Decision
The clippy reader gives every diagnostic coded `E0554` the analyzer level `note`, so `static.analyze` reports `analyze.clippy.E0554` at `info`. Every other code keeps rustc's level. `info` findings count in `diagnostics` but not in `errors`, and fail neither the check nor a `"static.analyze" = "none"` gate rule.

## Consequences
- bytes' three E0554 findings are `info`, and its `static.analyze` passes on what clippy can check.
- Clippy still cannot check a nightly-only target on stable; the note is where that is said. On a nightly toolchain E0554 does not occur and the target is linted as any other.
- Fixture: `crates/cupel-fixtures/repos/nightly-benches/rust` (a `#![feature(test)]` libtest bench), tested by `crates/cupel-fixtures/tests/nightly_benches.rs`.
