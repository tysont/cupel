# CUPEL_DECISION_006: Per-test coverage for JavaScript and TypeScript

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P3.3

## Context

Design open question 1 (§14) asks whether JavaScript and TypeScript get a per-test coverage map (`PerTestMap`) or accept file-level impact permanently. Design §8.7 already leans one way: "JS/TS has no cheap per-test map, so no `PerTestMap` producer is registered for `ecma`; `tests --impacted` falls back to `ModuleGraph` impact and says so." P3.3 adds the first `PerTestMap` producers (Rust, Go, Python) and `tests --impacted`, so the question must be closed before the verb's fallback is written.

## Options

**V8 coverage per test process.** Node writes V8 coverage for a process to `NODE_V8_COVERAGE`. A per-test map needs one process per test: list the tests, then run each alone (`vitest run -t '<name>' <file>`, `jest -t '<name>' <file>`) with its own `NODE_V8_COVERAGE` directory, and map V8's byte ranges back through source maps (TypeScript, bundler transforms) to lines. Every test pays a runner start-up (often a second or more for vitest and jest, which transform and cache per run), the name filters are regular expressions over titles that are not unique, and the mapping of byte ranges through source maps is a second, fragile parser. Vitest's and jest's own coverage providers (v8 or istanbul) aggregate per run, not per test.

**Istanbul per test through a custom environment or reporter.** A jest environment or vitest setup file could snapshot `globalThis.__coverage__` between tests. It works only with Istanbul instrumentation (not V8), needs code installed into the user's runner configuration, and differs between jest and vitest; Cupel would ship and maintain a plugin per runner and version.

**File-level impact from `ModuleGraph`, permanently for now.** The language layer already resolves every import edge (`ModuleGraph`), and `TestConventions` says which files are tests. A test file is impacted when it is in the scope or imports a file in the scope, directly or through other files. It costs no test run, works for every runner, and over-selects only within a test file (every test in an impacted file runs) and through imports that do not reach the changed code.

## Decision

The third option. No `PerTestMap` producer is registered for the `ecma` family. `tests --impacted` asks the support matrix whether `PerTestMap` can be produced for each language in scope; where it cannot (`unsupported(per-test-map)` for JavaScript and TypeScript), and also where it can but its tool is missing or its run fails, the verb selects test files by `ModuleGraph` impact, names the reason, and echoes `per_test_map: false` for that language and for the report. The ready-to-run command for an `ecma` runner names the impacted test files (`vitest run <files>`, `jest <files>`).

## Consequences

- `describe matrix` shows `tests --impacted` as `unsupported(per-test-map)` for JavaScript and TypeScript; `describe fact per-test-map` lists no producer for them. Nothing in the verb names a language: the fallback follows from the registry, so a future `ecma` producer turns it off without touching the verb.
- The mutation engine (P3.4) wants `PerTestMap`; for JavaScript and TypeScript it runs the impacted test files instead of the tests covering a mutated line.
- Revisit when a runner exposes per-test coverage natively (both vitest and jest track it as a feature request), which would make the first option a parser rather than a process per test.
