# CUPEL_DECISION_090: Reference plugins, the conformance suite, and plugins in the matrix test

**Date:** 2026-10-16 · **Status:** accepted · **Phase:** P5.1

## Context

Plan P5.1 asks for one trivial check plugin and one trivial producer plugin in each of Rust, Python, and TypeScript under `crates/cupel-fixtures/plugins/`, a protocol conformance suite over them, property tests on round-trips, and, first, a test that "a producer plugin registered for a fixture language flips a `(check, lang)` pair from `unsupported` to `supported` in `describe matrix` and the matrix test then demands its golden". The matrix test (ADR-043) enumerates registries built from compiled-in code only; it has no notion of plugins. Today no native check is `unsupported` anywhere (every check's needs have a producer for every language, `describe matrix`), so the only gap the matrix shows is `PerTestMap` for `javascript` and `typescript`, which the `tests --impacted` row reports (ADR-077). Fixture repositories are copied to temporary directories before every run, so a fixture cannot point at a plugin outside itself by a relative path.

## Options

**What the reference plugins do.** (a) A `CoverageMap` producer reading a committed LCOV file, as the plan suggests: every language already has native `CoverageMap` producers, so it would turn `unconfigured` cells into `supported` but flip no `unsupported` pair. (b) A `PerTestMap` producer reading a committed LCOV file with one `TN:` section per test (LCOV's own per-test record): it fills the one real gap, for `javascript` and `typescript`, and a check plugin that needs `PerTestMap` is `unsupported` there without it.

**Plugins in the matrix test.** (a) Register every reference plugin globally in the test's registries: every native pair's support would change in every fixture, and the goldens would depend on Python and Node being installed. (b) A fixture repository registers plugins the way a user's repository does, in its `cupel.toml`; the matrix test builds each fixture's registries with that fixture's plugins, and a plugin check has pairs exactly where it has its own fixture, `repos/<check>/<lang>/` (ADR-074's per-check fixtures).

**Reaching the plugin from a copied fixture.** (a) Copy the plugin's source into the fixture: a second copy to keep in step. (b) A symbolic link in the fixture to `crates/cupel-fixtures/plugins/`, followed when the harness copies the fixture.

## Decision

1. **Reference plugins.** Under `crates/cupel-fixtures/plugins/{rust,python,typescript}/`, each language has a check plugin and a producer plugin with the same behavior:
   - `cupel-produce-ref-<lang>` provides `per-test-map` v2 for `javascript` and `typescript` (ID `plugin.ref-<lang>`) by reading `coverage/per-test.lcov` (or `[lang.<id>] per_test_lcov`) relative to the root: each `TN:` section is a test, each `DA:<line>,<hits>` with hits above zero a covered line of the section's `SF:` file; the map's runner is `tool.vitest`.
   - `cupel-check-ref-<lang>` (check `ref.untested-<lang>`, tier 2, cost `quick`) needs `per-test-map` and reports `ref.untested-file` (warn, `write_test`, evidence `{file}`) for each file in scope that no test executes; its scope metric `untested` counts them, its score is the share of files some test executes, and its status is `fail` when any file is untested.
   - Rust plugins are binary targets of `cupel-fixtures` depending only on `serde_json`; Python plugins use the standard library and run as `python3 <file>`; TypeScript plugins use Node's standard library and run as `node <file>.ts` (Node's type stripping, on by default from 22.18 and 23.6). Finding IDs are `fnd_` and six hex digits of FNV-1a over `check`, `kind`, and the file joined by NUL, so all three languages agree.
2. **Conformance suite** (`cupel-fixtures/tests/plugin_conformance.rs`): the same cases run against each language's pair through `cupel_plugin`'s host, exactly as the CLI does — manifests validate and register; `plan` answers; the producer's fact equals the expected map; the check's result and findings are the expected ones on a committed fact; plus host-side refusals, with small purpose-written plugins in the test's temporary directory: an undeclared finding kind, invalid evidence, a fact of the wrong version or shape, a non-zero exit without a result (`error`), a result then a non-zero exit (accepted, noted), and SIGTERM then SIGKILL under a cancelled run. A language whose interpreter is not installed reports `SKIPPED: <tool> not installed` and is not silently passed.
3. **Matrix test.** `Registries::for_fixture(dir)` adds the plugins a fixture's `cupel.toml` configures (no `PATH` plugins), discovered as the binary discovers them. `fixture_pairs` keeps every native pair, computed in its fixture's registries, and adds, for every directory `repos/<check>/<lang>/` whose `cupel.toml` registers a plugin check with ID `<check>`, the pair `(<check>, <lang>)`; a `supported` pair then needs `golden/<check>/<lang>.json` like any other, and the stale-golden rule counts plugin checks as registered. The first such fixture is `repos/ref.untested-python/typescript/`: TypeScript sources, a committed `coverage/per-test.lcov`, a `plugins` link to `../../../plugins`, `[scope] exclude = ["plugins/**"]`, and `[plugins]` naming the Python check and producer. Its pair is `supported` only because the producer plugin is registered; without it, `unsupported(per-test-map)`.
4. **Copies follow links.** `copy_repo` copies what a symbolic link points to, so a fixture's copy carries real files; the daemon-parity twin copies the copy.

## Consequences

- The matrix test now fails when a plugin fixture's support changes without its golden, which is how a plugin pair earns its place, exactly as a native producer does (ADR-043).
- `python3` must be installed for the plugin fixture's golden, as runners are for theirs; CI installs Python. Without it the matrix test reports the pair skipped with a `SKIPPED:` line.
- With the Python producer plugin registered, `tests --impacted` serves TypeScript in that fixture; native support for JavaScript and TypeScript per-test maps is still open (ADR-006, ADR-077).
- The reference plugins carry `ABOUTME` headers like every Cupel source file; as fixture content under `crates/cupel-fixtures/`, the dogfood does not grade them (ADR-065).
