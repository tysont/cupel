# CUPEL_DECISION_053: Layering rules for `static.deps`: configuration, matching, and violations

**Date:** 2026-10-13 · **Status:** accepted · **Phase:** P2.5

## Context

Design §6.7 shows the layering policy as `[checks."static.deps".layers]` with `order = ["domain", "app", "infra"]` ("infra may depend on app may depend on domain, not reverse") and `map = { domain = "src/domain/**", … }`. It does not say how globs match, what happens to a module that matches no layer or several, whether a layer may skip the one below it, how malformed rules are reported, or how many findings one violating import produces. The matrix test (ADR-043) runs every check without flags on a copy of the language's fixture repository, so goldens can only show layer violations if the fixture itself carries the rule.

## Options

**Strict or relaxed layering.** Strict layering allows only the layer directly below; relaxed layering allows any lower layer. The design's sentence lists only adjacent pairs, but it states the forbidden direction ("not reverse"), and relaxed layering is what Clean Architecture and most layer linters enforce by default.

**Glob semantics.** `--paths` already uses `globset` with `*` confined to a path segment and `**` crossing them (ADR-040); using the same semantics avoids two glob dialects in one tool.

**Overlapping and unmapped modules.** Reject overlapping globs (expensive to decide for globs), or take the first layer in `order` that matches. Treat unmapped modules as violations, or leave them unconstrained.

**Fixture configuration.** A `cupel.toml` in each language's fixture repository is discovered by the run the matrix test makes; it changes `run.config` in every golden of that repository. The alternative, a separate CLI test with `--config`, leaves the goldens without the finding kind the rule exists to report.

## Decision

- `[checks."static.deps".layers]` must have exactly `order` (a non-empty array of distinct, non-empty layer names, lowest first) and `map` (a table from each layer name to one glob). Every name in `order` must have a `map` entry and every `map` key must appear in `order`. Anything else is `ConfigError::CheckSetting` naming `layers`, `layers.order`, or `layers.map.<name>`, raised when the check plans, before any work runs (exit 2). The check's other setting is `distance_max` (ADR-052); unknown keys are errors.
- Globs are relative to the repository root and compiled by `globset` with `literal_separator`, as `--paths` globs are: `*` stays within a segment, `**` crosses them.
- A module's layer is the first layer in `order` whose glob matches it. Modules no glob matches are unconstrained.
- **Relaxed layering:** a module may import any module in its own layer or a lower one. A `File` edge whose source layer is lower than its target layer is a violation, reported once per (from, to) pair at the first such import. Evidence: `from`, `to`, `from_layer`, `to_layer`, `specifier`, `order`. Location: the import's line in `from`. Without `layers`, no module has a layer and nothing is a violation.
- Each fixture repository carries a `cupel.toml` with only `[checks."static.deps".layers]`, mapping the `deps_layers/` fixture directories, so the goldens show a violation.

## Consequences

- One glob per layer; a layer spanning several trees needs a brace pattern (`{src/domain/**,lib/domain/**}`). Arrays of globs are an additive change.
- External and unresolved imports are never layer violations: they have no path to match.
- Every golden of the five fixture repositories echoes the layers table under `run.config.effective.checks` and lists `cupel.toml` in `run.config.sources`.
