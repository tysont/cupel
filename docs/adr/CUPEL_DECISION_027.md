# CUPEL_DECISION_027: Test frames from queries, assertion idioms, and path-template test conventions

**Date:** 2026-10-10 · **Status:** accepted; the pairing candidates amended on 2026-10-25 (P9.9 group 1): every file of the language, not the scope's · **Phase:** P1.6

## Context

Plan P1.6 asks `QueryTestFrames` to report cases, assertions with semantic kind and strength, mocks, skips, setup blocks, and the framework, with "the idiom→kind mapping in `tables.toml`", and `GlobTestConventions` to provide `is_test(path)` and `test_files_for(source)` from globs. Several things are unspecified: how a skip or `#[should_panic]` that sits *outside* the function it marks (Rust attributes, Python decorators) attaches to its case; how an idiom that is not a callee name (`assert!(x.is_some())`, `assert x is not None`) reaches the table; what one `framework` string means for a file using two runners; what a module-level mock applies to; and how a test file is found for a source file when the convention is not a glob (Rust's inline `#[cfg(test)]` modules, integration tests named after modules).

## Options

**Attaching markers.** Containment alone misses attributes and decorators. A second query per marker would need a join key the query cannot produce. Treating every capture in a match that also captures `@test.case` as that case's lets one pattern say "this attribute, these attributes, then this function".

**Idioms.** Keying the table only by callee text cannot express `x is not None`. A capture whose name carries the key (`@assertion.idiom.is-present`) lets the query recognize the shape and the table give it meaning, with the earliest pattern winning so idioms precede generic assertion patterns.

**Conventions.** Globs alone (`*_test.go`) answer `is_test` but not `test_files_for`; path templates with a few placeholders (`{dir}/{stem}_test.go`, `{base}tests/{stem}.rs`), optional glob expansion against the scope, a stem filter (`lib.rs` is tested by every `tests/*.rs`), and a `contains` text check (a module with `#[cfg(test)]` tests itself) cover the five languages without code.

## Decision

- tests.scm captures are as documented in `cupel_lang::testframes`: captures in a match with `@test.case` belong to that case, others to the innermost containing case; a mock outside every case belongs to every case in the file; a case keeps its first skip; the file's framework comes from the earliest pattern with a `@framework.<name>` capture, else `[tests].framework`; files without cases are not reported.
- Assertion keys are the `@assertion.idiom.<key>` suffix, else the `@assertion.name` text with whitespace removed; `[tests.assertions]` maps keys to `{ kind, strength }`; unknown keys are `[tests].unknown_assertion` (default `custom`, `strong`). Assertions that pin one value (`assert_eq!`, `toBe`, `==`) are strong; truthiness, inequality, and presence checks are weak.
- `[conventions]` has `test_globs` and ordered `tests_for` rules `{ template, stems?, contains? }` with placeholders `{path} {dir} {stem} {ext} {base} {module}` (`{base}` and `{module}` split the path at its last `[namespace].source_roots` directory). Candidates must be files in scope. *Amended 2026-10-25.* Candidates are every file of the language the walk keeps (`.gitignore`, `[scope] exclude`, the ecosystem's skipped directories), as `References` reads them (ADR-135), and the scope's own files; the sources paired are still the scope's. A scope narrowed to sources (`check --paths src/lib.rs`) left every test out of the candidates, so the source paired with none and `gen.property` reported its functions `property.missing` though a property test of it existed (itertools' `Itertools` methods, tested by `tests/quick.rs`; ADR-096 §3 reports only when each of a file's tests is in scope, and with no pairing that held vacuously). On a tree scope the candidates are the scope's files, as before. `TestConventions::is_test` and `test_files_for` are methods on the core fact; `is_test` matches globs with `globset` (literal separators), which `cupel-core` now depends on.

## Consequences

- One framework per file: a file mixing two runners reports the one whose pattern is listed first; a check that needs per-case frameworks would need a fact change (and a `VERSION` bump).
- Conventions are path-based and cheap; they do not prove that a test file exercises the source file. Coverage-based pairing arrives with `PerTestMap` (P3.3).
