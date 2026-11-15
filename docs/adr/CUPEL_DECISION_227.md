# CUPEL_DECISION_227: Benchmark sources are test code: JMH sources, Rust `benches/`, and Go benchmarks

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 8 · Amends [027](CUPEL_DECISION_027.md) and [108](CUPEL_DECISION_108.md); extends [184](CUPEL_DECISION_184.md) §4 to benchmark code

## Context
Replaying FasterXML/jackson-core PR #1775 (3.x, maintainer-merged), `check --changed --gate` failed `"static.clones" = { pairs = "<= base" }`: clones went from 1 to 2, the new pair two benchmark methods of `jmh/src/main/java/tools/jackson/core/jmh/StringWriteBench.java`. jackson-core's JMH benchmarks are a Maven module of their own, `jmh/`, whose sources sit under `jmh/src/main/java` (not `src/test` or `src/jmh`) and import `org.openjdk.jmh.annotations.*`; Java's test globs are `src/test/**` and `**/src/test/**`, so the benchmark was source. NOTES Q-java-1: "should JMH benchmark sources … count toward static.clones.pairs? Recommend: no — treat them like tests (benchmarks repeat a harness per case by design)." Q-java-2: `clones.pairs <= base` with δ 0 fails any PR extending deliberately parallel implementations (#1790, the three parsers); recommend keeping the rule. The owner's rulings (2026-11-14): "JMH sources excluded from clones and quality like tests; same for Rust benches/ and Go benchmark functions. Q-java-2: clone-pair rule unchanged."

**How test code is left out today.** Every check that leaves tests out does so through one predicate, `TestConventions::is_test(file)`, true when one of the language's `[conventions] test_globs` matches the file:

- `static.clones`: a group whose members are all in test files is `info` (`shape: test-code`) and counts in none of `pairs`, `clusters`, or `max_jaccard` (ADR-184 §4);
- `static.crap` and `tests.coverage`: a test file's functions are not scored;
- `tests.mutation` (cupel-mutate's plan): a test file's functions are not mutated;
- `tests.assertion-density`, `gen.property`, `meta.oracle-independence`: a test file's functions are not the code under test;
- `static.api-surface`: a test file's items are not API (V9);
- `static.dead-code`: a test file roots its module.

`static.complexity` grades test code like any other and does not consult the conventions; "quality like tests" therefore means the list above, and benchmark functions stay in `static.complexity`, as test functions are.

**Rust and Go.** Rust's `test_globs` already hold `**/benches/**/*.rs` (Cargo builds each `benches/` file as a crate of its own that only `cargo bench` runs). Go benchmark functions, `func BenchmarkXxx(b *testing.B)`, can live only in `_test.go` files, which `**/*_test.go` matches. Both are test code today.

## Options
(a) Glob only: add `src/jmh/**` (the Gradle JMH plugin's source set) and `**/jmh/src/**` (a module named `jmh`). jackson's layout matches the second, but a benchmark module named otherwise (`benchmarks/`, `perf/`) would not, and a module named `jmh` that is not one would. (b) By content: a Java file that imports `org.openjdk.jmh` is a benchmark, wherever it is. It finds jackson's file, but a helper class of a benchmark module that imports nothing of JMH stays source. (c) Both: the Gradle source set by glob (every file in it is benchmark code by construction), and any other file by its JMH import. Chosen: (c).

## Decision
1. `[conventions]` gains `test_contains`, a list of strings: a file of the language whose text contains one of them is a test file. `GlobTestConventions` reads each file of the language the walk keeps (as `References` reads them, ADR-135) and adds each such file to `TestConventions::test_globs` as a glob matching exactly that path, so every check that consults `is_test` treats it as a test file, and the fact's shape is unchanged.
2. Java: `test_globs` gain `src/jmh/**` and `**/src/jmh/**`; `test_contains = ["org.openjdk.jmh"]`.
3. Rust's `**/benches/**/*.rs` and Go's `**/*_test.go` already cover their benchmarks; nothing changes for them, and tests pin both.
4. **Q-java-2.** `static.clones`' `pairs` gate rule and δ are unchanged: a PR that adds a clone pair to deliberately parallel implementations fails `pairs <= base`, and a project that duplicates by design relaxes its own `[gate.changed]`.

## Consequences
- jackson-core #1775's two new pairs in `StringWriteBench.java` are `info` test-code clones, so `pairs` stays at its base value; the PR's complexity finding (`UTF8JsonGenerator._writeStringSegment`, source) is unchanged.
- A JMH benchmark is no longer mutated, covered, CRAP-scored, or API; `static.complexity` still grades it.
- Reading every Java file's text adds one read per file to the `test-conventions` producer of a Java project; other languages configure no `test_contains` and read nothing more.
- Fixture: `crates/cupel-fixtures/repos/benchmark-sources/java`.
