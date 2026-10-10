# CUPEL_DECISION_023: Language-layer plumbing: erased production, registry-rendered `describe`, and the conformance inventory format

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P1.1

## Context

P1.1 builds `cupel-lang` and needs four things the design names but does not specify:

1. The conformance harness (design §8.3.7) must run "the producer" for each registered `(fact, language)` pair. `ProducerRegistry` stores producers type-erased and, after P0.2, exposes only metadata (the erased `produce` was deferred to P2.1's scheduler).
2. `describe language <id>`, `describe languages`, `describe fact <kind>`, and `describe matrix` (design §6.5) need payload shapes in the envelope's `description` field (ADR-018 left them to this step).
3. The inventory files `conformance/<fact>.toml` and `repos/<lang>/inventory.toml` have no format, and "the producer detects each construct" has no definition. Three agents will write entries against it in parallel.
4. Where JavaScript and TypeScript keep their shared queries, given that one query must compile against three grammars (JavaScript, TypeScript, TSX) and TypeScript needs patterns JavaScript's grammar rejects.

## Options

**Erased production.** (a) Have the harness depend on concrete producer types per language: defeats "iterate the registries". (b) Add `RegisteredProducer::produce` returning `serde_json::Value`: easy for the harness, but the P2.1 scheduler needs typed facts and would round-trip through JSON. (c) Return a small erased box, `Produced`, that can be downcast to the fact type or rendered as JSON: one method serves both the scheduler and the harness.

**Describe payloads.** (a) Reuse `Description::Command` with free-form maps: no schema. (b) Four typed variants in `cupel_core::Description`, each listing everything from the registries, with every property present (ADR-013).

**Detection.** (a) Per-fact Rust predicates written by each language agent: unreviewable drift. (b) A declarative expectation per construct (`expect`, a partial JSON object) matched against the produced fact, with a fixed per-fact rule for locating the entry a mapping names (`file` + `name`). Language-neutral, data-only, and the harness stays fact-aware but language-blind.

**Shared ECMAScript queries.** (a) One directory per language ID with duplicated queries. (b) `langs/ecma/` holds the implementation, `queries/`, and `tables.toml` for both IDs; TypeScript appends the files under `langs/ecma/queries/typescript/` to the shared ones, so TypeScript-only patterns never reach the JavaScript grammar.

## Decision

1. `RegisteredProducer::produce(ctx, scope, sink) -> Result<Produced>`; `Produced` carries the `FactKind`, `downcast`/`downcast_ref::<F>()`, and `to_json()`.
2. `Description` gains `language`, `languages`, `fact`, and `matrix` variants (`LanguageDescription`, `LanguagesDescription`, `FactDescription`, `MatrixDescription`). Languages are listed in ID order; facts in catalog order; a fact lists every registered language with its producers (possibly none), its `Fact::VERSION` and schema (null for kinds without a payload type yet), and its consumers. `SupportDescription` is tagged by `status`: `supported`, `unsupported { fact }`, `unavailable { tool }`, `unconfigured { key }`. The text renderer prints the matrix as a table whose cells read `supported`, `unsupported(<fact>)`, …; `make matrix` prints it.
3. The inventory format and the detection rule are as documented in the `cupel_fixtures::conformance` module docs: constructs carry `id`, `summary`, optional `within`, `expect`, and `relation`; mappings carry `file` and `name` (plus optional `expect` and `twin`) or `not-applicable = "<reason>"`. Unknown keys, unknown facts, unknown constructs, and empty reasons are errors. Only facts a language has a producer for are checked; every construct of those is required. A fixture file that produces a `lang.parse-error` fails conformance.
4. Besides conformance, every `(language, fact)` pair with a producer has a golden of the full fact over the language's fixture repository at `golden/facts/<lang>/<fact>.json` (ADR-003 mechanics); a golden for a pair without a producer is stale and fails.
5. `langs/ecma/` serves both ECMAScript IDs; `.js .jsx .mjs .cjs` parse with the JavaScript grammar (which includes JSX), `.ts .mts .cts` with TypeScript, `.tsx` with TSX.
6. Parse failures are `lang.parse-error` findings (check `lang`, subject = the file, `warn`, action `review`) emitted by every producer that parses the file; files with recoverable syntax errors are still extracted. De-duplicating them across producers is the scheduler's job (P2.1), by finding ID.

## Consequences

- P2.1's scheduler uses `Produced::downcast` and `Facts::insert`, with no new core API.
- Producers added in parallel by different agents touch the shared `describe` goldens (`describe/languages`, `describe/fact-symbols`); conflicts there are resolved by regenerating with `UPDATE_GOLDENS=1 make test`.
- A construct whose meaning cannot be expressed as "some located item contains these values" needs a new locator rule or a `relation`; that is a harness change, made once per fact, not per language.
