# CUPEL_DECISION_118: Deductive verification: the `ProofResults` fact, Dafny in every language, and `spec.prove`

**Date:** 2026-10-17 · **Status:** accepted · **Phase:** P7.5

## Context

Design §5.5: `spec.prove` — "deductive verification (Dafny, Verus, Prusti, Creusot, Frama-C, SPARK)", kinds `proof.failed` and `proof.obligation-open`, cost `exhaustive`, tier 6, wrap. §8.3.2 names `ProofResults` among the P7 facts without a shape. Plan P7.5 pairs this check with `conc.model-check` and asks whether one fact serves both. Still open: whether it does, what a proof result is, what "obligation open" means as opposed to "failed", which verifier to wrap first, and which languages it serves when the proofs are not written in the language of the code beside them.

## Options

**One fact or two.** One `ProofResults` for both checks would hand a bounded model checker's verdicts to a tier 6 check, and the registry's one producer per (kind, language) would make Kani and a Rust verifier (Verus, Prusti, Creusot) exclude each other. Two facts keep each check's evidence its own (ADR-117 adds `ModelCheckResults`); `ProofResults` stays as the catalog names it, for verifiers only.

**Failed versus open.** A verifier reports obligations it could not prove (an error: the code or the proof is wrong, or the proof is incomplete), obligations the proof assumes (`assume`, a lemma with no body, an `{:axiom}`), and obligations it gave up on (out of time or resources). Only the first is evidence of a defect; the other two are absences of proof. So `failed` is the first, `open` the other two: nothing refutes them, nothing proves them.

**The first verifier.** Verus needs its own Rust toolchain and builds from source; Prusti and Creusot pin old nightlies and are research-grade; Frama-C (C) and SPARK (Ada) serve languages Cupel does not have. Dafny ships self-contained releases with Z3 for Linux x64 and macOS arm64/x64 (`dotnet tool install` as an alternative), installs in seconds, and reports diagnostics as JSON lines (`--json-output`).

**Languages.** Dafny proofs are their own language; they verify algorithms that compile (Dafny targets Go, Java, JavaScript, Python, C#) or are hand-written in a project's language. (a) Restrict to Dafny's compile targets: arbitrary, since a proof of a hand-written algorithm is as valid beside TypeScript or Clojure. (b) Every language, with the `.dfy` files of each of its project roots: the proofs belong to the project, wherever its code is.

## Decision

1. **`ProofResults` v1** (`cupel-core/src/proof.rs`): `{tool, runs: [RootRun], files, verified, obligations: [{status: failed|open, message, location, related: [{message, location}]}]}`; `files` are the proof files verified (repository-relative, sorted), `verified` the declarations verified completely, obligations ordered by place then message (the verifier works in parallel). Conformance inventory `conformance/proof-results.toml` (`failed`, `assumed`), checked by each producer's real-tool test (ADR-116 §2).

2. **`tool.dafny`** (`cupel-adapt/src/proof/dafny.rs`): every language (`Langs::Any`), needs `ProjectInfo`, cost `exhaustive`, ready with `dafny` on `PATH`. In each project root of the language holding a scoped file, over its `.dfy` files outside excluded paths (a root without one is not run): `dafny verify --json-output --error-limit 0 <files>`. Diagnostics from the `Verifier`: an error is a `failed` obligation, or `open` when it says the verifier ran out of resources or time; a warning about an `assume` without `{:axiom}` or a body-less method's postcondition is `open`; other warnings are not obligations. An error from the parser or resolver fails the root's run, with each `file:line:col: message`. Messages are `defaultFormatMessage` with its `{i}` arguments filled; places are 1-based lines and columns; `verified` comes from the closing status line.

3. **`spec.prove`** (`cupel-checks/src/spec_prove.rs`): tier 6, `exhaustive`, needs `ProofResults`, no settings. `proof.failed` (`fail`, `fix_bug`) per failed obligation and `proof.obligation-open` (`warn`, `review`) per open one, at the obligation, evidence `{tool, message, related: [{message, file, line}]}`, subject `file|message|n` (n tells repeats of one message in a file apart). Status `fail` only for a failed obligation. Score `undischarged` (failed + open) over proof files; metrics `verified`, `failed`, `open`, only when some root ran; notes for failed runs, or that no proof file was found. Combined by sum.

## Consequences

- `spec.prove` is supported in all seven languages; it finds nothing to say in a project without `.dfy` files but that. In a repository whose root is a project of two languages, the same proofs are verified once per language, and their findings share IDs.
- An `{:axiom}` is trusted, as Dafny trusts it (it is not reported); `dafny audit` would list axioms and externs and is not run.
- The goldens hold Dafny's messages, so CI pins 4.11.0.
- Deferred: **Verus** (its own toolchain and a source build), **Prusti** and **Creusot** (pinned old nightlies, research-grade, and each a Rust producer that would compete with Dafny for Rust: selecting among them needs a `[lang.rust]` pin key for `ProofResults`), **Frama-C** (C) and **SPARK** (Ada), which serve languages Cupel does not have.
