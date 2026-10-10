# CUPEL_DECISION_001: One `cupel-checks` crate with a module per check

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P0.1

## Context

The technical design originally sketched one crate per check (`cupel-check-complexity`, `cupel-check-mutation`, …). The implementation plan (§2.1) prescribes a single `cupel-checks` crate with one module per check instead. The workspace is created in P0.1, so the granularity has to be settled before any crate exists. What must be preserved either way is the extension rule from design §8.2–§8.3: a check depends on `cupel-core` alone and can never name a language, a tool, a file system, or a process.

## Options

**One crate per check (`cupel-check-*`).** The dependency boundary is per check, so a check that grows a heavy dependency does not burden the others, and each check's compile unit is isolated. The cost is a crate (manifest, `lib.rs`, lint config, CI surface) for every one of the ~25 catalogued checks, most of which are a few hundred lines of pure computation over facts. Each new crate is also a registration point the CLI must list, and cross-check helpers (score arithmetic, threshold parsing) need yet another shared crate or get duplicated.

**One `cupel-checks` crate, one module per check.** All native checks share one manifest whose only workspace dependency is `cupel-core`. The extension rule is enforced once: by that manifest, and by a crate-scoped clippy `disallowed_methods` rule forbidding `std::process` and `std::fs`. Adding a check is one module plus goldens. The cost is that a dependency added for one check is visible to all of them, and the crate recompiles as a whole.

**One crate per check group (`static`, `tests`, `gen`, …).** A middle ground, but groups do not correlate with dependencies: `static.analyze` wraps external tools through producers just like `tests.coverage`, and the group split would be arbitrary for the boundary that matters.

## Decision

One `cupel-checks` crate, one module per check, depending on `cupel-core` only. A check is split into its own `cupel-check-<name>` crate only when it acquires a heavy, check-specific dependency (a solver, a WASM runtime, a large parser) whose compile time or binary size would burden every other check. A split crate obeys the same rule: it depends on `cupel-core` alone.

## Consequences

- The extension rule is checked in exactly one `Cargo.toml` and one clippy configuration; a violation is a build failure, not a review comment.
- Registration of native checks is one function in `cupel-checks` that the CLI calls.
- Shared check-side helpers live as private modules in the same crate rather than in a new crate.
- If a check later needs a heavy dependency, splitting it out is a mechanical move plus a change to the crate graph, which per plan §2.5 needs its own ADR.
- `cupel-checks` itself is created in P2.1 with the first check; P0 creates only `cupel-cli` and `cupel-core`.
