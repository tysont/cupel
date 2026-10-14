# CUPEL_DECISION_066: A `[gate.tree]` rule on a range-only check is a configuration error

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P2.10

## Context

ADR-060 evaluates `[gate.tree]` for tree, paths, and symbols scopes and marks a rule `skipped` (which fails the gate) when its check did not complete. A check with `base_needs` (ADR-056; today `tests.weakening`) is skipped with `needs-range` on every scope without a range, so a `[gate.tree]` rule on it fails every tree run, whatever the code. P2.9 recorded this as an open item ("rejecting it at load is a follow-up"); the project owner ruled that it is rejected at configuration load.

## Decision

`Config::validate_registered` rejects any `[gate.tree]` rule whose check declares `base_needs` with `ConfigError::TreeGateNeedsRange { check }`: exit 2, a `cli.usage` finding on `[gate.tree]` whose fix says to move the rule to `[gate.changed]`. `[gate.changed]` rules on such checks are unaffected.

## Consequences

- A configuration that could never pass a tree gate is caught before any check runs, on every verb that loads configuration with the registries.
- The `next_actions` rerun that ADR-063 added for a skipped tree rule on a `needs-range` check is no longer reachable from configuration; it stays for skipped checks in general.
