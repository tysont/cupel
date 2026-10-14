# CUPEL_DECISION_043: The matrix test, and evidence validated by a minimal in-house JSON Schema subset

**Date:** 2026-10-13 · **Status:** accepted · **Phase:** P2.1

## Context

Plan §2.8 and design §8.3.7 make the matrix test the arbiter of what works where: every `supported` (check, language) pair needs a passing golden, every `unsupported` pair a passing `skipped` assertion, and every check must run when given only its declared facts. ADR-012 adds that each emitted finding's `evidence` should be validated against the JSON Schema its check's manifest declares for the finding kind. The plan leaves open how the test is structured (so that deleting a golden demonstrably fails it) and which validator checks evidence.

## Options

**Validator.** The `jsonschema` crate implements every draft and keyword, but pulls in a large dependency tree (regex engines, URL and date-time parsing, optional HTTP resolution) for a test-only need, and validates far more than native evidence uses. A minimal validator covers exactly the keywords `schemars` emits for flat evidence structs: `type` (one or a list, `integer` meaning a number with no fraction), `properties`, `required`, `additionalProperties` (boolean or schema), `items`, `enum`, `const`, `minimum`, `maximum`, plus the annotations `$schema`, `title`, `description`, `format`, `default`, `examples`. The risk of a subset is silently accepting what it does not understand; refusing every other keyword turns that risk into a test failure.

**Structure.** Inline loops in a test file, or a library module in `cupel-fixtures` whose `verify(goldens)` returns every problem, so a test can point it at a copy of `golden/` with one file removed and assert the failure.

## Decision

- `cupel_fixtures::evidence::validate(schema, value)` implements the subset above and rejects any other keyword (`$ref`, `oneOf`, `pattern`, …) with an error naming it. When a check's evidence needs more, the validator grows with a test, or the decision is revisited with the plugin host (P5.1), which must validate plugin evidence at run time.
- `cupel_fixtures::matrix` enumerates `CheckRegistry × LanguageRegistry` through `SupportMatrix::compute`. For each `supported` pair it copies `repos/<lang>/` to a temporary directory, runs the built binary with `check --only <check> --format json`, requires exit 0 and no `skipped` entry, validates every finding of that check against its manifest schema (and refuses a kind the manifest does not declare), and compares the redacted envelope with `golden/<check>/<lang>.json`. For each other pair it runs the same command and requires a `skipped` entry with the matrix's reason, fact or tool, and language. A golden under a check directory that no supported pair claims is stale and fails.
- `verify(&Goldens)` returns every problem rather than stopping at the first; the test asserts the list is empty, and a second test proves that a golden copied away fails it.
- A separate in-process test builds each check's `Facts` from its manifest alone (producing exactly its `needs` and producible `wants` over the fixture) and runs it, so a check that reaches for an undeclared kind gets `UndeclaredFact` and fails.

## Consequences

- Adding a producer that flips a pair to `supported` fails the build until its golden exists; `UPDATE_GOLDENS=1 make test` writes it for review.
- No new dependency. Evidence schemas must stay within the subset (flat structs with scalar or array fields), which every P2 evidence shape planned so far satisfies.
- Goldens include every finding a check reports on a fixture, so growing a fixture repository changes that language's goldens; each change is reviewed like any golden diff.
