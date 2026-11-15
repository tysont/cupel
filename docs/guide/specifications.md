<!-- ABOUTME: Trace validation, model checking, proofs, contracts, and requirement traceability. -->
<!-- ABOUTME: Part of Cupel's guide, linked from the README. -->
# Specifications and provenance

## Trace validation

`spec.trace` checks that what the implementation did is a behavior of its specification. The specification is a `spec` oracle in TLA+ (`cupel oracle add counter --kind spec --spec spec/Counter.tla --lang tla+ --write`), and the evidence is a trace the implementation logs while its tests run. The convention is a file format, so it needs no build step or library in any of the seven languages: when the variable `CUPEL_TRACE_DIR` is set, a test writes one `.jsonl` file per behavior into that directory, one JSON object per step, and writes nothing when it is unset.

```json
{"state": {"count": 0, "done": false}}
{"action": "Inc", "state": {"count": 1, "done": false}, "file": "counter.go", "line": 24}
```

`state` holds the values of the spec's variables after the step (a variable left out is left to the spec); `action` names the spec action the step took, `file` and `line` where it was logged, all three optional. Integers, strings, booleans, arrays (sequences), and objects (records) map to TLA+ values, and `{"$tla": "{1, 2}"}` is any TLA+ expression (a set, a model value). The producer reruns the suite once with `CUPEL_TRACE_DIR` pointing at a fresh `.cupel/cache/trace/` directory, then runs TLC (`tlc` on `PATH`, as tla-bin installs it) on a module that extends the spec and allows from each logged state only the spec's next-state relation (or the named action) to the next logged state. The spec's `INIT`, `NEXT`, and `CONSTANTS` come from the `.cfg` beside it, else `Init` and `Next`. On the Go fixture, whose `Bump` adds two but logs an `Inc`:

```text
$ cupel check --only spec.trace --format text
fail (1)
  spec.trace
    trace `bumps.jsonl` leaves spec/Counter.tla at step 3: `Inc` to {"count":3,"done":false} is not a step the spec allows from {"count":1,"done":false}
      fnd_c58eaf · trace.diverged · tier 5 · confidence 1
      at counter.go:29
      evidence: action "Inc" · oracle "counter" · previous {2} · spec "spec/Counter.tla" · state {2} · step 3 · steps 3 · tool "tool.tlc" · trace "bumps.jsonl" (full: cupel explain fnd_c58eaf)
      repro: cupel check --only spec.trace --oracle counter --paths counter.go
```

A trace whose first state is not an initial state of the spec diverges at step 1. A trace that is not JSON lines of steps, or that TLC rejects (a variable or action the spec does not have), is a note, and so is a recording run whose tests failed or that recorded nothing. The result is against an oracle, so it is tier 5 only while `cupel oracle verify` holds for the spec; otherwise it is capped at tier 3 with `oracle.unverified` (see [`cupel oracle`](usage.md#cupel-oracle)). Quint and P specs are oracles TLC does not read, and the result says so. The gate keys are `spec.trace.diverged`, `traces`, and `unvalidated`.

## Model checking

`conc.model-check` runs Kani (`cargo kani`, with concrete playback) in each Rust project root and checks every `#[kani::proof]` harness over all the inputs its `kani::any()` values can take, within the harness's bounds. Builds go to `.cupel/cache/model-check/`. Each property an input breaks is a finding at the place it breaks, with the values, in the order the harness drew them, and the unit test Kani wrote to replay them:

```text
$ cupel check --only conc.model-check --format text
fail (2)
  conc.model-check
    harness `proofs::average_never_overflows` refutes `attempt to add with overflow` in `average` with 128, 128
      fnd_f4c6a6 · model.counterexample · tier 5 · confidence 1
      at src/lib.rs:3
      evidence: description "attempt to add with overflow" · function "average" · harness "proofs::average_never_overflows" · replay "/// Test generated for harness `proofs::…" · tool "tool.kani" · values [2] (full: cupel explain fnd_f4c6a6)
      repro: cupel check --only conc.model-check --paths src/lib.rs
    harness `proofs::at_stays_in_bounds` refutes `index out of bounds: the length is less than or equal to the given index` in `at` with 4ul
      …
```

A harness Kani cannot decide is a note, and so is a root that does not build. Kani needs `cargo install --locked kani-verifier` and `cargo kani setup`, which downloads its bundle and the nightly it builds with. The other languages have no model checker, and the matrix says `unsupported`. The gate keys are `conc.model-check.failed`, `harnesses`, `verified`, and `undetermined`.

## Proofs

`spec.prove` runs `dafny verify` over the `.dfy` files in each project root of every language (Dafny compiles to several of them, and its proofs stand on their own), and reports each obligation the verifier did not discharge, at its place:

```text
$ cupel check --only spec.prove --format text
fail (1)
  spec.prove
    proof fails at proofs/tally.dfy:24: a postcondition could not be proved on this return path
      fnd_993483 · proof.failed · tier 6 · confidence 1
      at proofs/tally.dfy:24:1-24:2
      evidence: message "a postcondition could not be proved on t…" · related [1] · tool "tool.dafny" (full: cupel explain fnd_993483)
      repro: cupel check --only spec.prove --paths proofs/tally.dfy

warn (2)
  spec.prove
    proof obligation open at proofs/tally.dfy:31: assume statement has no {:axiom} annotation
      fnd_6f1c03 · proof.obligation-open · tier 6 · confidence 1
      …
    proof obligation open at proofs/tally.dfy:35: This ensures clause is part of a bodyless method. Add the {:axiom} attribute to it or the enclosing method to suppress this warning
      …
```

An obligation the verifier could not prove is `proof.failed` (`fail`), with the places it relates (the postcondition a return path breaks). One the proofs assume (an `assume` without `{:axiom}`, a lemma or method without a body) or that ran out of time or resources is `proof.obligation-open` (`warn`): nothing refutes it, and nothing proves it either. A file that does not parse or resolve is a note. Dafny's releases bundle Z3; put `dafny` on `PATH`. The gate keys are `spec.prove.failed`, `open`, and `verified`.
## Contracts

`spec.contracts` reruns the suite once more through the baseline's runner, after the baseline passes, with the contract library's switches on and every property test given the run's seed and case budget (as `gen.property` gives them), and reads each failed test's output for the library's report. A project root whose manifests do not declare the library is not run again, and a note says nothing was checked.

| Language | Library | Contracts | Turned on for the run |
|---|---|---|---|
| Rust | the `contracts` crate | `#[requires]`, `#[ensures]`, `#[invariant]`, and their `debug_` and `test_` forms | debug assertions in every profile a test builds with (`CARGO_PROFILE_DEV_DEBUG_ASSERTIONS`, `CARGO_PROFILE_TEST_DEBUG_ASSERTIONS`), which `debug_*` contracts need |
| Python | icontract | `@require`, `@ensure`, `@invariant` | `ICONTRACT_SLOW=1`, which enables contracts declared `enabled=icontract.SLOW` |

A violation is placed at the contract (the attribute or decorator), names the function or class it is declared on, and keeps the library's message: the description and condition, and for icontract the values it saw, with addresses masked. Under `--changed` a violation is kept when its contract or its test is in a changed file. On the Rust fixture, whose `debug_*` contracts its own `[profile.dev]` leaves unchecked:

```text
$ cupel check --only spec.contracts --format text
fail (3)
  spec.contracts
    precondition of `share` violated in `ledger/src/lib.rs::tests::shares_nothing_among_no_parts`: a share needs at least one part: parts > 0
      fnd_1d32c0 · contract.violated · tier 5 · confidence 1
      at src/lib.rs:4
      action fix_bug (priority 1): A caller passed what the precondition rejects: fix the call the test makes, or the precondition if it is wrong, then rerun.
      repro: cupel check --only spec.contracts --paths src/lib.rs
    postcondition of `half` violated in `ledger/src/lib.rs::tests::halves_one`: half of a positive amount is positive: ret > 0
      fnd_4aa38d · contract.violated · tier 5 · confidence 1
      at src/lib.rs:10
    invariant of `post` violated in `ledger/src/lib.rs::tests::posts_three_entries`: an account holds two entries: self.entries.len() <= 2
      fnd_bedc57 · contract.violated · tier 5 · confidence 1
      at src/lib.rs:25
```

The evidence holds the `tool`, the contract's `kind` (`precondition`, `postcondition`, `invariant`), the `function`, the `message`, and the `test`. A run that did not complete in a root is a note. The gate key is `spec.contracts.violations`. Go, JavaScript, TypeScript, Java, and Clojure have no producer: their tests' failures do not say a contract failed (an `assert`, a Guava precondition, and a Clojure `:pre` all read as plain assertions), so the pair is `unsupported`.

## Requirements and test provenance

`meta.traceability` checks a change against the task's requirements. List their IDs in `cupel.toml`, under a per-language override when a repository's languages trace different ones:

```toml
[checks."meta.traceability"]
requirements = ["REQ-12", "REQ-13"]
```

A test traces a requirement when the range added it (no test of that name in that file at the base commit) and its name or its doc comment carries the ID as whole words, in any case and with any separator: `test_req_12_parses`, `it("parses week dates #REQ-12")`, `/// Parses week dates (REQ-12).`, but not `test_req_120` or `testReq12`. The doc comment is the language's: Rust's `///` lines or `/** */` above the test's attributes, Go's `//` lines above the function, a Python docstring, a JSDoc or Javadoc `/** */` block above the test, and Clojure's `;` lines above `deftest`. A test's body is not read. A test that existed before the change passed without it, so it cannot show the change meets the requirement; the finding lists such tests with `added: false`.

A `REQ-<n>` marker (`req` followed by a number as its own word, as in `#REQ-7`, `req_7`, `TestParse_REQ_7`) on a test the range added is a requirement too, when no listed ID matches it: the test traces it, and the check lists it as `spec.unlisted-requirement` (`info`, so no gate fails on it), whose evidence says `source: marker` where a listed requirement's says `source: config`. So a repository that writes requirement IDs only into its tests gets them traced without configuration, and one that lists them hears of a marker the task does not name (often a mistyped ID). Markers on tests that existed before the change add nothing. With no requirement listed or marked the check passes with a note. The gate keys are `meta.traceability` (none untested) and `meta.traceability.untested`.

`meta.oracle-independence` asks who wrote the evidence. Under a range, with a per-test coverage map (made at `--depth standard` for Rust, Go, and Python; see [Coverage](coverage-and-mutation.md#coverage)), it finds the tests that run each changed function. A function a test from before the change runs is `independent`; one only tests the range added run is `evidence.self-authored-only`: worth checking those tests' expectations against the task rather than the code. Without a map, it labels nothing and says why. A registered oracle's independence is its verification: a result against an unverified oracle is already capped at tier 3 with an `oracle.unverified` finding (see [`cupel oracle`](usage.md#cupel-oracle)). The gate keys are `meta.oracle-independence.independent` and `self_authored_only`.
