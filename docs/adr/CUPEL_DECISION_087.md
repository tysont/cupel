# CUPEL_DECISION_087: The `cupel-plugin` crate, its edges, and where the protocol types live

**Date:** 2026-10-16 · **Status:** accepted · **Phase:** P5.1

## Context

Design §8.2 lists `cupel-plugin` ("plugin protocol types; external-process host for check and producer plugins") without its edges, and plan §2.1 requires an ADR for any change to the crate graph. Plan P5.1 puts `Manifest` (`kind: check | producer`), `RunRequest`, and the event types in `cupel-plugin`. Two earlier decisions pull the other way: design §8.2 says `cupel-core` owns the types and generates the published JSON Schemas from them so `describe schema` cannot drift, and ADR-013 §9 made `cupel_core::schema::plugin()` the schema of `Manifest` and said it "grows to cover `RunRequest` and plugin events in P5.1". ADR-043 adds that the plugin host "must validate plugin evidence at run time", while the only evidence validator lives in `cupel-fixtures`, a test crate.

## Options

**Protocol types in `cupel-plugin`.** Matches the plan's wording. But `describe schema plugin` is served from `cupel_core::schema::PUBLISHED`, which cannot name a type in a crate that depends on it; the CLI would have to splice a second list of published schemas in, and `cupel-core`'s own tests of the published list and the `plugin` schema would move or shrink.

**Protocol types in `cupel-core`, the host in `cupel-plugin`.** The wire types (`PluginManifest`, `ProducerManifest`, `RunRequest`, `PluginEvent`, `PluginResult`, `PluginFact`) sit beside the types they embed (`Manifest`, `Finding`, `Plan`, `Score`, `FactKind`), and `schema::plugin()` grows exactly as ADR-013 §9 said. `cupel-plugin` holds everything that does something: discovery, the subprocess host, validation, and the `Check`/`Producer` adapters that enter the registries.

**The host inside `cupel-cli`.** No new crate, but the CLI would grow a subprocess protocol beside its verbs, and `cupel-fixtures` (the conformance suite and the matrix test) could not reach it without depending on the binary crate.

**Evidence validation.** Leave the validator in `cupel-fixtures` and validate plugin evidence only in tests, or move it to the host's crate so the host and the matrix test share one validator.

## Decision

1. **Types in `cupel-core`.** A new module `cupel_core::plugin` holds the protocol's wire types and `PROTOCOL_VERSION = 1`. `schema::plugin()` becomes the schema of `PluginMessage`, an untagged union of every message (`describe`'s manifest, the run request, `plan`'s reply, one `run` event), so one published schema documents the whole protocol (`$id` unchanged: `https://cupel.dev/schema/v1/plugin.json`). This is the plan's "`cupel-plugin`: `Manifest`, `RunRequest`, event types" placed where design §8.2 and ADR-013 §9 put types and schemas; the plan's crate keeps the behavior.
2. **Crate graph.** A new crate `cupel-plugin` → `cupel-core` only (plus `serde`, `serde_json`). `cupel-cli` → `cupel-plugin` (discovery per invocation, registration into the catalog). `cupel-fixtures` → `cupel-plugin` (the matrix test's registries include a fixture's plugins; the conformance suite drives the host). `cupel-plugin` names no language, check, or producer; `cupel-checks`, `cupel-lang`, `cupel-adapt`, and `cupel-mutate` do not depend on it.
3. **Evidence.** The evidence validator (ADR-043's keyword subset, unchanged in behavior) moves from `cupel-fixtures` to `cupel_plugin::evidence`; `cupel-fixtures` re-exports it, so the matrix test and the host validate with the same code. The host validates every plugin finding's evidence against the schema its manifest declares (ADR-089).
4. **Additive core changes the host needs**, each small and generic: `Facts::json` and `Upstream::json` (a fact as it serializes, so it can be written to a file), `Process::input` and `Process::terminate` (standard input, and SIGTERM before SIGKILL), `Cancellation::remaining`, `Error::Plugin`, `ConfigError::Plugin`, and `[plugins]` in `ConfigFile`.

## Consequences

- `describe schema plugin` documents the request and events agents and plugin authors need; its snapshot changes once, here.
- A plugin author in any language reads one schema; a Rust plugin author can depend on `cupel-core` for typed messages, which the Rust reference plugins deliberately do not, to show the protocol needs nothing but JSON.
- The graph keeps its shape: everything below `cupel-cli` still depends on `cupel-core` alone, except `cupel-mutate` (ADR-080), `cupel-serve` (ADR-085), and now `cupel-plugin`, all leaves over `cupel-core`.
- A later WASM tier (design §8.6, not v1) is one more host in `cupel-plugin` over the same types.
