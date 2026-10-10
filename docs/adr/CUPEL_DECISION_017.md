# CUPEL_DECISION_017: Configuration layering, gate-key grammar, and registry validation

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P0.4

## Context

Design §6.7 gives a full `cupel.toml` example, the precedence order (flags > `CUPEL_*` env > `--profile` > `cupel.toml` > defaults), and the gate operators. Implementing it in P0.4 requires answers it does not give:

1. **Which settings are layered.** Precedence only makes sense for keys every layer can set. The design names `CUPEL_DEPTH` and `CUPEL_FORMAT` "e.g.", and its profiles set `depth`, `format`, and `budget`; the rest of the file (`[checks]`, `[gate]`, `[lang]`, …) has no flag or env form.
2. **The default output format.** Design §6.1 rule 2 auto-detects `format` from whether stdout is a terminal, which core cannot observe.
3. **Splitting a gate key.** `static.crap.max` is check `static.crap` plus metric `max`, but `tests.weakening` is a bare check. Until the check registry exists (P2.1) the split must be syntactic.
4. **Which operators apply to which keys.** `none` and `no-breaking` read as statements about a check's findings; `>= 0.80` needs a number to compare.
5. **What `run.config.sources` lists.** `ConfigEcho.sources` is a list of layers, not per-key provenance.
6. **How a per-language override combines** with the language-neutral check table (`[checks."static.deps".layers]` is nested).
7. **Where unknown check and language IDs are rejected** once registries exist, given `[checks."<id>"]`, `[lang.<id>]`, and gate keys are open maps today.
8. **Discovery edge cases:** a relative `--config`, a missing explicit file, and `--profile` naming a profile that does not exist.

## Options

For (1): layer every global flag (format, depth, budget, timeout, seed, color, install, …), or only the settings the design shows in a profile. Layering everything multiplies env vars nobody asked for and invites conflicting semantics (`--seed` is "recorded", not configured); the profile keys are the design's own evidence of what users configure per context.

For (3): a registry-driven split (impossible before P2.1, and ambiguous if one check ID were a prefix of another), or a fixed shape for check IDs. Every catalogued check (design §5) is `<group>.<name>`.

For (6): shallow merge (an override of `layers.order` would drop `layers.map`) or deep merge of tables with leaves and arrays replaced.

For (7): reject nothing until P2.1, or provide the validation now as a function over the registries that callers invoke once registries are populated.

## Decision

1. The layered **settings** are `depth`, `format`, and `budget`. Flags, env, a profile, and the top level of `cupel.toml` each supply an all-optional `Settings`; the first layer in precedence order that sets a key wins. Env vars are `CUPEL_` + the uppercased key (`CUPEL_DEPTH`, `CUPEL_FORMAT`, `CUPEL_BUDGET`), defined once on the `Setting` enum. `CUPEL_PROFILE` selects a profile when `--profile` is absent. Other `CUPEL_*` variables are ignored by configuration (`CUPEL_NO_DAEMON` belongs to P4); a recognized variable with an invalid value is a usage error naming the variable.
2. Defaults: `depth = standard`, `format = null`, `budget = null`. A null effective `format` means "auto-detect from stdout"; the CLI resolves it.
3. A check ID is exactly two dot-separated segments; a gate key is a check ID optionally followed by one metric segment. Segments match `[a-z][a-z0-9_-]*`. `[checks."<id>"]` keys use the same check-ID grammar.
4. Comparisons (`>= > <= < ==`) take a finite number, optionally suffixed `%`, and require a metric. `none` and `no-breaking` take no operand and require a bare check ID. Anything else is rejected at parse time.
5. `sources` lists, in precedence order, every layer that contributed: `flags` / `env` when they set at least one setting, `profile` when one is selected, `cupel.toml` when a file was loaded, and `defaults` when at least one effective setting fell through to its default.
6. Per-language overrides deep-merge into the check table: nested tables merge key by key, any other value (including arrays) replaces. The key `lang` inside `[checks."<id>"]` is reserved for these overrides and never reaches the check.
7. `Config::validate_registered(checks, langs)` rejects check IDs (in `[checks]` and gate keys), gate metrics not declared in the check's `Manifest.metrics`, and language IDs (in `[lang]`, `[roots]`, and per-check overrides) unknown to the registries, listing the valid values. The CLI calls it once registries are populated (checks P2.1, languages P1.1). Until then only syntax is validated.
8. `--config PATH` is resolved against the root (`--cwd`) when relative and must exist. Otherwise discovery walks from the root through its ancestors and takes the first `cupel.toml`; none found means defaults only. Selecting a profile that the file does not define, or selecting one with no file, is a usage error listing the defined profiles. Every configuration error maps to exit 2 and its message includes the file path; parse errors include line, column, and the valid keys at that level.

## Consequences

- Adding a layered setting is one `Settings` field, one `Setting` variant, and one top-level `cupel.toml` key; its env var follows automatically.
- Check IDs with three segments are impossible; a plugin check must choose a `<group>.<name>` ID.
- P2.9 receives typed rules whose operand shape is already guaranteed by the key shape.
- `[checks."<id>"]` values and `[lang.<id>]` values are open tables today; checks and producers validate their own keys once their manifests declare parameters.
