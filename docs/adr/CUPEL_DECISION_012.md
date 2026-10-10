# CUPEL_DECISION_012: Finding evidence is open JSON, schema'd by the emitting check's manifest

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P0.2

## Context

Plan P0.2 asks for "`Evidence` (tagged per kind)". Design §7.2 says evidence "is check-specific but schema'd per finding kind (`describe finding <kind>`)", and its example nests the evidence object directly under `"evidence"` with the finding's `kind` alongside it. The question is whether the per-kind shapes are Rust types in `cupel-core` or data owned by each check.

## Options

**A closed enum in `cupel-core`, adjacently tagged by the finding's `kind`.** `#[serde(tag = "kind", content = "evidence")]` reproduces the JSON shape exactly and gives every kind a compile-time type. But every new finding kind then edits `cupel-core`, so adding a check is no longer "one module plus goldens" (plan §2.8), and plugin checks — which declare their own kinds (design §8.6) — could never emit evidence the enum does not know.

**Open JSON object in `cupel-core`, per-kind schema in the check manifest.** `Evidence` is a JSON object. Each `Manifest` lists its finding kinds, and each kind carries the JSON Schema of its evidence. `describe finding <kind>` renders that schema; the plugin host already validates plugin output against the manifest (design §8.6) and can validate evidence the same way. Native checks build evidence from their own `Serialize + JsonSchema` structs, so they keep compile-time types locally.

## Decision

`Evidence` is a serde-transparent wrapper over a JSON object. `Manifest.findings[]` (`FindingKindSpec`) carries `kind`, the `ActionKind` the finding maps to, a summary, and `evidence`: the JSON Schema of that kind's evidence. "Tagged per kind" is satisfied by the finding's `kind` field selecting the manifest's schema.

## Consequences

- Adding a check or a plugin check never touches `cupel-core`.
- The envelope's JSON Schema (`describe schema run`) types `evidence` as an object; the precise shape per kind is discovered through `describe finding <kind>`.
- The matrix and golden tests (P2.1) should validate each emitted finding's evidence against its manifest schema; that validation lands with the first check.
