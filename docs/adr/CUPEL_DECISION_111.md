# CUPEL_DECISION_111: Check settings are self-described: `Manifest.settings`, rendered by `describe` and a generated README table

**Date:** 2026-10-17 · **Status:** accepted · **Phase:** P6

## Context

ADR-104 (the P6.5 extension drill) recorded a defect and left its fix for the P6 merge: a check's settings live in private constants (`Limits` for the integer ones, hand-written `Settings::read` functions for `static.clones`' threshold and `static.deps`' `distance_max` and `layers`), the `Manifest` has no `settings` field, so `cupel describe <check>` does not list them, and the README's Configuration paragraph names each check's settings and defaults by hand. Every new check with settings therefore adds a sentence to the README, and nothing keeps that sentence, the validation, and the defaults in agreement. The proposed fix was `settings: Vec<SettingSpec { name, default, summary }>` on `Manifest`. The `Manifest` is also the shape a check plugin prints from `describe` (ADR-088, ADR-089), so the change reaches the plugin protocol, every check's `describe.json` golden, and the run and plugin schema snapshots.

Open: the type of a default (integers, a fraction, and `layers`, a table with no default); whether plugins must declare the field; whether validation is rewritten to read the specs or keeps its constants; what `describe` shows; and what the README generates.

## Options

**The default's type.** (a) A string, rendered as written: loses the difference between `0.82` and `"0.82"` for an agent that writes the TOML. (b) A JSON value: integers and fractions are numbers, exactly what `[checks."<id>"]` accepts, and a setting with no default is `null`. Chosen: (b).

**Absent or empty.** (a) Always emitted, as ADR-013 wants for emitted types: every check's `describe.json` gains `"settings": []`, and the plugin schema, which requires every property, would require it of plugins while Cupel accepts its absence. (b) Required when parsing too: every plugin manifest printed before this change fails to parse, so every installed plugin breaks on upgrade. (c) Omitted when empty and `x-cupel-optional` in the schemas, as ADR-079 made `CheckResult.notes`: manifests without settings, native or plugin, are unchanged, the plugin schema says what Cupel accepts, and a reader treats an absent field as empty. Chosen: (c).

**Plugins.** A plugin that has settings declares them in the same shape. Cupel validates a plugin's declared settings only for duplicates, as it does its finding kinds and metrics; it does not refuse undeclared keys for a plugin, because plugins have always received their `[checks."<id>"]` table unchecked, and refusing keys now would break a plugin that reads settings it never declared.

**Validation.** (a) One generic validator in `cupel-core` that refuses any key a manifest does not declare before `plan`: one source, but it would also have to check value shapes, which differ (`threshold` in (0, 1], `layers` a table of `order` and `map`), so each check would still validate its own values, and plugins (above) would need an exception. (b) Keep each check's validation and derive its accepted names from the same declaration that fills the manifest: `Limits` carries each setting's summary beside its name and default and builds the specs (`Limits::specs`); `static.clones` and `static.deps` build theirs in one function each, and `Settings::read` takes the valid names from it, so the `KEYS` and `SETTINGS` name lists are gone. A registry-wide test pins the agreement for every native check: an unknown key is refused with exactly the declared names, and every declared default is itself accepted. Chosen: (b), the smaller change, with the test catching any check that declares one thing and accepts another.

**`describe` text.** A `settings` section after `metrics`, one row per setting, `name  default · summary` (a `null` default reads `none`), shown only for a check with settings; JSON is the manifest as it is.

**README.** (a) A column in the check table: the table is already seven columns wide, and settings carry summaries. (b) A generated `check-settings` section, one row per setting (check, setting, default, what it sets), replacing the hand-written sentences in the Configuration paragraph, with the prose around it kept. Chosen: (b).

## Decision

1. `cupel_core::check::Manifest` has `settings: Vec<SettingSpec>` (between `metrics` and `stability`; omitted when empty and parsed as empty when absent, marked `x-cupel-optional` in the run and plugin schemas, ADR-079), and `SettingSpec { name: String, default: serde_json::Value, summary: String }` is exported from `cupel_core`. `null` is the default of a setting whose absence turns its rule off (`static.deps`' `layers`).
2. Every native check fills it. `Limits::new` takes `(name, default, summary)` triples and `Limits::specs()` returns them as specs; each check sets `settings: <LIMITS>.specs()` (empty for the checks with no settings). `static.clones` (`Settings::specs`: `threshold` 0.82, `min_lines` 4, `min_nodes` 20) and `static.deps` (`settings()`: `distance_max` 0.7, `layers` null) build theirs and validate against the names in them. `cupel_checks::manifest::setting` builds one spec.
3. A check plugin may declare `settings` in the same shape; a plugin manifest that declares one name twice is refused (`setting `<name>` is declared twice`), as for finding kinds and metrics. The reference plugins declare none and need no change.
4. `cupel describe <check>` shows the settings: in JSON as the manifest's `settings`, in text as a `settings` section.
5. The README's per-check settings are the generated section `<!-- generated:check-settings -->`, rendered by `crates/cupel-cli/tests/readme.rs` from each check's `describe` output and rewritten by `make readme`; the staleness test covers it with the other three sections.

## Consequences

- A new check with settings declares them once, beside its defaults, and `describe`, the README, and the configuration error's list of valid names follow; no README sentence is written by hand.
- `crates/cupel-checks/tests/settings.rs` fails for a check whose declared settings and accepted keys differ, or whose setting has no summary.
- The `describe.json` goldens of the seven checks with settings (`perf.bench`, `static.clones`, `static.complexity`, `static.crap`, `static.deps`, `tests.assertion-density`, `tests.quality`) and the run and plugin schema snapshots gained the field; every other golden is unchanged, and plugin manifests printed before this change parse as before. `Manifest.settings` is the second optional property of an emitted type after `CheckResult.notes`.
- A plugin's settings are descriptive: Cupel lists them but does not refuse a key a plugin did not declare. Making that a configuration error would be a protocol change for every installed plugin and needs its own decision.
- Value shapes are still validated by each check (`threshold` in (0, 1], `layers`' table). A spec has no type or range field; one would be needed before a generic validator could replace the per-check code.
