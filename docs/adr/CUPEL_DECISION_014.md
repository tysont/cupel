# CUPEL_DECISION_014: Syntactic fact payloads are scope-wide lists joined by `SymbolKey`

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P0.2

## Context

P0.2 asks for payload types for the ten syntactic facts (design §8.3.2). The catalog lists what each fact means but not its shape: whether a payload covers one file or a whole scope, and how per-symbol facts (`Decisions`, `NormTree`, `MutationSites`, `Roots`) refer to the symbols in `Symbols`. The obvious join key, `Symbol.hash`, is derived from `NormTree`, so `NormTree` itself cannot be keyed by it, and `Decisions` would depend on hashing having run.

## Options

**Per-file payloads.** One fact per file mirrors the per-file content-hash cache key, but `Producer::produce(ctx, scope, sink) -> F` returns one value per scope, and every check would iterate a collection of facts instead of one fact.

**Scope-wide payloads keyed by structural hash.** Stable across edits, but circular for `NormTree` and forces an ordering on every producer.

**Scope-wide payloads keyed by `SymbolKey { file, line, col }`.** The symbol's start position within one run is unique and available to every language producer without hashing. It is not stable across edits — that is what `Symbol.hash` is for — but facts are always joined within a single run, never across runs.

## Decision

Each syntactic fact payload covers the whole scope a producer was asked for. Symbol-level entries reference symbols by `SymbolKey`, the 1-based start position of the symbol's span; `SymbolDef::key()` derives it. `NormTree` entries carry the symbol's `SymbolHash`, which is the bridge from run-local keys to stable identity. Positions use a shared `Span { line, col, end_line, end_col }` (1-based). Semantic enums are the kebab-case spellings of the design's semantic names (`early-return`, `arith-add-sub`, …), which coincide with the conformance construct names except where a construct is named for its syntax (the `if` construct is a `branch` decision).

## Consequences

- A check joins facts with a map from `SymbolKey`; anything persisted across runs (store, baselines, finding IDs) uses `SymbolHash` only.
- Per-file caching (design §8.3.6) happens inside producers and the store, below the payload shape.
- The shapes are first fixed by P1 producers and their conformance suites; a change there bumps the fact's `VERSION`, and adding or removing a fact kind still requires an ADR (plan §2.5).
