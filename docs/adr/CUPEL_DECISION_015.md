# CUPEL_DECISION_015: Support-matrix resolution is transitive and reports unconfigured producers

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P0.2

## Context

Design §8.3.5 defines `supported` as "every fact in `needs` has at least one registered producer for `lang`", and plan P0.2 gives `Support::{Supported, Unsupported(FactKind), Unavailable(ToolId)}` computed by `SupportMatrix::compute(checks, producers, langs)`. Three gaps appear when writing it:

1. Producers have needs of their own (the mutation engine needs `MutationSites`, `CoverageMap`, `TestResults`). Read literally, a check is `supported` for a language whose only producer cannot run because an upstream fact has no producer there.
2. `Availability` has three states (`Ready`, `MissingTool`, `Unconfigured`), but `Support` has no state for `Unconfigured`.
3. Computing `Unavailable` requires evaluating `availability(ctx)`, so `compute` needs a `Ctx` the plan's signature omits.

## Options

For (1): direct-only resolution (cheap, matches the literal text, reports pairs as supported that the scheduler cannot satisfy) versus transitive resolution over producer needs (the matrix agrees with what the fact DAG in design §8.9 can actually build). For (2): fold `Unconfigured` into `Unavailable` (loses the config key and points `doctor` at an install that will not help) versus a fourth variant. For (3): pass `Ctx`, or split the matrix into a static part and an availability overlay.

## Decision

- Resolution is transitive. A fact is producible for a language when some producer covering that language has every one of its needs producible and is `Ready`. Needs are checked before availability, so a permanent gap (`Unsupported`) outranks a fixable one (`Unavailable`). When no producer succeeds, the reason reported is the first producer's in registry order. A dependency cycle among producers resolves as `Unsupported` of the fact that closes the cycle.
- `Support` gains `Unconfigured(String)` carrying the config key, so `doctor` can emit `action.kind: configure` instead of `install_tool`.
- `SupportMatrix::compute(checks, producers, langs, ctx)` takes the availability `Ctx`. Only `needs` participate; `wants` never affect support.

## Consequences

- `describe matrix` and the matrix test (P2.1) agree with what the scheduler can build.
- Renderers of the matrix must handle four states; the extra state is visible in `describe matrix` as `unconfigured(<key>)`.
