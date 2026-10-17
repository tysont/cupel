# CUPEL_DECISION_117: Bounded model checking: the `ModelCheckResults` fact, Kani for Rust, and `conc.model-check`

**Date:** 2026-10-17 · **Status:** accepted · **Phase:** P7.5

## Context

Design §5.4: `conc.model-check` — "bounded model checking on implementation (Kani, CBMC, Stateright)", kind `model.counterexample`, cost `exhaustive`, tier 5, wrap; §12 names Kani for P7. The P7 facts the catalog (§8.3.2) names are `Traces`, `Histories`, `ProofResults`, and `ContractViolations`; none is a model checker's result unless `ProofResults` serves both this check and `spec.prove` (ADR-118 decides it does not), so this check needs a fact kind the catalog does not have, which plan §2.5 makes an ADR's decision. Still open: what a model checker's result is, how Kani is run and read, where a counterexample comes from, and what the other languages get.

## Options

**The fact.** (a) Share `ProofResults` with `spec.prove`: both are "verification results". But the registry selects one producer per (kind, language); with Kani and Verus (or Dafny) both producing it for Rust, one check would receive the other's evidence, and a bounded model check, which is sound only up to its bounds, would grade as tier 6 proof (or a proof as tier 5). (b) A kind of its own, `ModelCheckResults`, per harness: a model checker's unit is a harness (an entry point with nondeterministic inputs), its result a verdict per harness, the properties refuted, and concrete inputs; a verifier's unit is an obligation. (b).

**Counterexamples.** CBMC's traces (`--trace`) are long and tool-shaped. Kani's concrete playback (`-Z concrete-playback --concrete-playback=print`) prints, per refuted check, the values the harness drew, as comments, inside a ready unit test that replays them. Reading those keeps the evidence short, repeatable, and actionable (the test is the regression case).

**Other tools.** CBMC checks C, which is not a Cupel language; JBMC (CBMC's Java front end) would serve Java and, through bytecode, Clojure, but ships only as part of a CBMC source build or distribution packages of uneven age; Stateright is a Rust library whose model checker runs inside the project's own tests and would need a reporting convention of its own; Go, Python, JavaScript, and TypeScript have no bounded model checker in common use.

## Decision

1. **`ModelCheckResults` v1** (`cupel-core/src/model_check.rs`), a new catalog kind (§8.3.2 row): `{tool, runs: [RootRun], harnesses: [{root, name, location?, status: verified|failed|undetermined, refuted: [{description, function?, location?, counterexample?: {values, replay?}}]}]}`, harnesses by root, then name. `location` is where the harness is defined; a property's `location` is where it fails, `None` outside the repository. Conformance inventory `conformance/model-check-results.toml` (`verified`, `failed`, `undetermined`), checked by each producer's real-tool test (ADR-116 §2).

2. **`tool.kani`** (`cupel-adapt/src/model_check/kani.rs`): Rust only, needs `ProjectInfo`, cost `exhaustive`, ready with `cargo-kani` on `PATH` (install `cargo install --locked kani-verifier && cargo kani setup`). In each Rust root holding a `.rs` file in scope: `cargo kani --output-format terse -Z concrete-playback --concrete-playback=print --target-dir .cupel/cache/model-check/kani`. The reader takes each `Checking harness NAME...`, its `Failed Checks: DESC` with the `File: "f", line N, in FN` after it, its `VERIFICATION:- SUCCESSFUL|FAILED|…` verdict (anything else is undetermined), and each playback test, given to the refuted check its `/// Check for …` line names (else to the first without one), with the `// value` comments as `values`. A compiler error, or output without a harness and without Kani's "No proof harnesses" line, fails the root's run. A harness is located at the `fn` of its last path segment within three lines after a `kani::proof` attribute; one defined in an excluded file is dropped.

3. **`conc.model-check`** (`cupel-checks/src/conc_model_check.rs`): tier 5, `exhaustive`, needs `ModelCheckResults`, no settings. One `model.counterexample` per refuted property (`fail`, `fix_bug`, confidence 1) where it fails (else at its harness), titled with harness, property, function, and values, evidence `{tool, harness, description, function, values, replay}`, subject `root|harness|description`. Score `failing` harnesses over harnesses; metrics `harnesses`, `verified`, `failed`, `undetermined`, only when some root ran; notes for failed runs and undetermined harnesses. Combined by sum.

## Consequences

- `conc.model-check` is supported for Rust and `unsupported(model-check-results)` for Go, Python, JavaScript, TypeScript (no bounded model checker for them is in common use), Java and Clojure (JBMC deferred, below).
- Kani's first `cargo kani` run, or `cargo kani setup`, downloads its bundle (~350 MB with CBMC) and installs the nightly it builds with; its location follows `KANI_HOME`. The goldens hold Kani's check descriptions and playback tests, so CI pins 0.68.0.
- A harness's bounds (`#[kani::unwind]`) are the harness's: an unwinding assertion Kani cannot discharge is a refuted property like any other.
- Deferred: **CBMC** for C (no C language in Cupel), **JBMC** for Java and Clojure (a CBMC build or distribution package per platform, outside "installs in minutes" on both CI runners; its counterexample format differs), **Stateright** (its checker runs inside the tests; it needs a result convention like ADR-116's), and **Kani's `--harness-timeout`** (unstable; `--timeout` bounds the run instead).
