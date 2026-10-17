# CUPEL_DECISION_096: `gen.property`: falsified and missing properties, their evidence, and `--scaffold` stubs from signatures

**Date:** 2026-10-16 · **Status:** accepted · **Phase:** P5.4

## Context

Design §5.3 gives `gen.property` the kinds `property.falsified` and `property.missing` and says it "can scaffold property stubs from signatures (`--scaffold`, writes only with `--write`)"; §6.4 adds `--cases N`; §7 asks that counterexamples be minimized and every finding carry a `repro`. ADR-095 decides how properties are found and run and what `PropertyResults` holds. Still open: what `property.missing` means, what each finding's evidence is, and how stubs get written when a check may not write outside its scratch directory and may not know a language, while a stub is code in one.

## Options

**`property.missing`.** (a) Every public function with no property test anywhere: on a project without property tests, every function. (b) A signature class, such as functions whose parameters and result are all values: needs types most languages' signatures do not carry. (c) A public function that takes arguments, in a source file whose tests (by `TestConventions`) hold no property case. It is decidable from facts every language has, it asks for a property where the file's tests are example-based only, and it is quiet for a file that has one. It cannot tell which property tests which function in a file; it is advice, so `info`.

**Stubs.** (a) The check renders them: it would have to know each language's syntax and framework. (b) The CLI renders them from the findings with the language layer, which owns the normalized signatures (`ApiSurface`), namespaces (`Symbols`), and its tables: a check stays language-blind, and the write goes through the run's `Writer`, which allows it only with `--write`.

**Previewing.** `init`, `baseline set`, and `oracle add` preview by recording writes when `--write` is absent and returning `status: plan` with `would_write`. `check --scaffold` must run the check first to know what is missing, so it cannot be a plan-only run.

## Decision

1. **`gen.property`**, tier 4, cost `standard`, needs `PropertyResults`, `Symbols`, `TestConventions`; wants `ApiSurface` (signatures for evidence and stubs). No settings (the budget is `--cases`). Metrics (scope): `properties` (property cases in scope), `falsified`, `missing`. Score: falsified properties (unit `falsified`, n = properties). Status `fail` when any property is falsified or failed; `property.missing` is `info` and never fails it.

2. **`property.falsified`** — one per property whose outcome is `falsified` or `failed`; severity `fail`, action `fix_bug`, at the case's span. Evidence `{test, framework, runner, counterexample, seed, cases}` — `counterexample` the framework's shrunk input (null for `failed`, confidence 0.6 instead of 0.95), `seed` null where the framework takes none. Title `` `name` is falsified by `counterexample` ``. `repro`: `cupel check --only gen.property --paths <test file> --seed <seed> --cases <cases>`. Finding IDs are by the case's file and name.

3. **`property.missing`** — for each function (`SymbolKind::Function`, `Visibility::Public`, `parameters ≥ 1`) in scope in a file that is not a test file, when no property case of `PropertyResults` is in one of the file's test files (`TestConventions::test_files_for`), and each of those test files is in scope (else whether one is a property test is unknown and nothing is reported). Severity `info`, confidence 0.5, action `add_property`, at the function. Evidence `{function, name, namespace, parameters, signature, frameworks}` — `signature` from `ApiSurface` (null when absent), `frameworks` from `PropertyResults`. The summary names `--scaffold`.

4. **`check --scaffold`** (a `check` flag): after the run, the CLI turns every `property.missing` finding into a stub through `cupel_lang::property_stubs`, which renders the language's `[property_stubs]` table (`tables.toml`): the stub file's path (from the source file's project root, directory, and stem), a header written once per new file, and one stub per function — a property test of the language's first framework, marked skipped until its property is stated (`@pytest.mark.skip`, `#[ignore]`, `test.skip`), with one generator per parameter (by the parameter's normalized type where the table maps it, else the table's default; a language whose table has no default stubs only functions whose every parameter type it maps). Stubs go to their files through the run's `Writer`: with `--write` they are written (appended to an existing file, skipping functions already stubbed there) and each file gets a `next_actions` entry to state its properties; without `--write` nothing is written, the envelope's `plan.would_write` carries each file's diff — the one field a run that is not a plan fills — and a `next_actions` entry repeats the command with `--write`. `--scaffold` with `--dry-run` plans the run and renders nothing.

5. **`--cases N`** is no longer pending: it reaches producers as `Focus::cases` (from `Job::cases`) and is the property engine's budget (ADR-095 §3).

## Consequences

- An agent's loop is: `property.falsified` → fix the code until the property holds for the counterexample; `property.missing` → `cupel check --only gen.property --scaffold --write`, then state each stub's property and remove its skip marker.
- `property.missing` is per file, not per function: a file with one property test reports none of its functions. Its quality depends on `TestConventions` pairing tests to sources; a source no convention pairs with a test file is reported (nothing tests it as a property).
- Stubs that need a type the language's table does not map (Rust structs, generics) are not rendered; the finding stays and says so through the absence of a `would_write` entry.
- `plan` on a real run's envelope is new; readers that took a non-null `plan` to mean `status: plan` must check `status`.
