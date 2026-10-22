# CUPEL_DECISION_143: The follow-up items P9 leaves open, and why

**Date:** 2026-10-21 · **Status:** accepted · **Phase:** P9.F2

## Context

The P9 Definition of Done allows v0.2.0 with follow-up items still open only when an ADR defers each one with a reason. P8 left four engineering items that no P9 step closes: (9) the musl allocator's speed under rayon, (11) two release binaries no CI job runs, (14) the wrappers' Windows paths, and (15) the schema site's version. P9.4–P9.7 each recorded open items in the plan's notes. This ADR says, for each, what it is, why it waits, and what would reopen it, and sorts the P9.4–P9.7 items into deferrals and bugs; a bug is not deferred here, it stays on the follow-up list.

## Options

**Close every item before v0.2.0.** Items 9, 11, and 14 need hardware or runners Cupel does not have (an aarch64 Linux machine, an x86_64 Mac, Windows CI, a measured multi-core musl workload), and item 15 needs a release process that does not yet exist; closing them would hold the release on infrastructure, not on code.

**Defer each with the evidence that would reopen it.** The release ships what is verified, the gaps are named where a reader of the plan finds them, and each has a concrete trigger.

## Decision

Each item below is deferred, with what would reopen it.

### (9) The musl allocator's speed under rayon is unmeasured

The Linux release binaries are static musl builds (ADR-124), and musl's allocator is known to serialize under heavily threaded allocation. Cupel's parsing and fact production run on rayon, so a musl `cupel` may be slower than a glibc or macOS one on many cores. Nothing has measured it: the only Linux runs are CI's, which run debug test builds on glibc, and no benchmark of the release binary exists (`perf.bench` has no Cupel benchmark yet, ADR-129). It is deferred because it is a speed question, not a correctness one, and a change (a different global allocator such as mimalloc, which adds a C dependency to every target) should follow a measurement, not precede it. **Reopens when** a timing of `cupel survey` on the same mid-size tree with the x86_64 musl binary and a glibc build of the same commit on the same 8-core or larger Linux machine shows the musl binary more than 20% slower, or a user reports it; P9.9's external dogfood on Linux is the natural place to take that timing.

### (11) The aarch64 Linux and x86_64 macOS binaries are not executed in CI

`release.yml` cross-builds `aarch64-unknown-linux-musl` on an x86_64 runner and `x86_64-apple-darwin` on the arm64 macOS runner, and runs only the binaries the runner can execute (`runs: true` on the other three). GitHub's free tier offers no arm64 Linux runner for this repository and no Intel macOS runner that is not being retired, so neither binary runs before it is published. The build still checks the Linux binary is statically linked, and the release's install verification (P8.5, repeated for v0.2.0 in P9.10) runs both on real machines by hand. It is deferred because the remaining risk (a binary that builds but does not start) is caught by that verification before the release is announced. **Reopens when** GitHub offers an arm64 Linux or Intel macOS runner to the repository's plan, or a published binary for either target fails to start for a user; either makes the `runs: true` line the fix.

### (14) The wrappers' Windows paths are untested

The npm and pipx wrappers (ADR-125) pick `x86_64-pc-windows-msvc`, download the `.zip`, unpack `cupel.exe`, and cache it under `%LOCALAPPDATA%`; `crates/cupel-cli/tests/installers.rs` runs both wrappers against a fake release on Unix only, so the zip branch, the `.exe` name, and the Windows cache directory run nowhere. It is deferred because the Windows job that P9.8 adds ([ADR-137](CUPEL_DECISION_137.md)) compiles the workspace for Windows and runs only the tests that need no Unix tools (`make windows-test`: `cupel-serve` and the unit tests), and `installers.rs` is compiled only on Unix there (ADR-137 §5); making the wrapper tests run on Windows is test-harness work, not a change to the wrappers. **Reopens when** someone extends that job to the wrappers: split `installers.rs`'s wrapper tests from its `sh`-based install-script tests (the install script is POSIX `sh` and does not target Windows) into a file compiled on Windows, have the fake release serve a `.zip` holding a stand-in `cupel.exe`, and install `node` and `python3` on the runner; then the two wrapper tests run on Windows and this item closes. A Windows user reporting a wrapper failure reopens it at once.

### (15) The schema site serves `main`'s tip, not a release

`pages.yml` publishes `schema/v1/*.json` from `main` on every push (ADR-127), so the URL an envelope names (`https://tysont.github.io/cupel/schema/v1/run.json`) serves the newest `main`, not the release that wrote the envelope. It is deferred because the versioned path is the contract and nothing yet reads an envelope with a schema from a different build: agents read the envelope of the binary they just ran, and Cupel's own tests validate against the schemas the same build emits. The gap is real, though: `v1` has grown by additions, and some added properties are `required` (ADR-132's `targets`, ADR-134's `doc`), so an envelope an older release wrote can fail `main`'s schema. **Reopens when** a consumer validates stored envelopes or envelopes from another release against the published URL, or a schema change cannot be made additive (that change is `schema/v2/`, a new path, and `v1` keeps being published from the last commit that had it); the fix is publishing `schema/<release>/` from the release workflow beside `main`'s `v1`.

### The P9.4–P9.7 open items

Deferred, each a limit of a check or fact that misses findings rather than inventing them, or a cosmetic gap; each reopens when an external dogfood (P9.9) or a user finds a finding it should have made:

- **P9.4 (1)** Clojure `:refer` names and Go's package-level imports give `dep.cycle` unnamed edges, so the cheapest cut falls back to component breaking and cycle order. Reopens with a Clojure or Go cycle whose suggested cut is wrong.
- **P9.4 (2)** Rust's edge names count expanded `use` paths, not statements: the cut is still a real edge, only its weight is coarse.
- **P9.4 (3)** `clone.pair` carries no `common_subtree` hint; a pair's two members are the extraction already. Reopens when a user asks for it; the change is one line in `StaticClones::hints`.
- **P9.4 (4)** The README's captured `check` example predates the hints. The README describes only what exists, and the example is real output of an earlier build; P9.10 recaptures it for v0.2.0.
- **P9.5 (1)** `--spec FILE` does not exist (ADR-114), so traceability has no spec-file source. Reopens with the first project that keeps requirements in a file Cupel should read.
- **P9.5 (2)** Clojure `^{:doc}`, JUnit's `@DisplayName`, `describe` titles, and JavaScript `//` comments above `it` are not read as test docs. Reopens per language when a project marks requirements there.
- **P9.5 (3)** A `REQ-n` marker on a test the range edited (not added) adds no requirement, by ADR-114's rule for edited tests; this is the rule, not a gap.
- **P9.6 (1)** Reachability is not followed from roots, so dead code only other dead code calls is found one layer at a time. Reopens with a dogfood report of a dead chain.
- **P9.6 (2)** An unresolved name matches every symbol of that name in the language: missed findings, never invented ones.
- **P9.6 (3)** Go, Java, and Clojure plain names resolve only within their file, and Clojure `:refer` lists are not captured. The ES aliased-import half is follow-up item 20 (P9.F1).
- **P9.6 (4)** A JavaScript `eval` lowers every JavaScript finding's confidence: conservative, never wrong.
- **P9.6 (5)** `References` parses every file of the language on each run (6 s for Cupel in a debug build). Reopens when a repository makes it the slowest production of a `--changed` run.
- **P9.7 (1)** Type 1 compares the normalized tree, so copies that differ only in what normalization erases (an annotation, an attribute, a modifier) read as type 1. Reopens if the file text becomes a fact.
- **P9.7 (2)** A cluster's `edits` repeat a removed subtree once per member that lacks it: verbose, not wrong.
- **P9.7 (3)** Only the Go and Java fixtures hold a type-1 pair; the type is computed by one language-free digest, so the other languages differ only in fixtures.

Closed by [ADR-142](CUPEL_DECISION_142.md), which narrows a range's scope to the symbols its changed lines touch: **P9.5 (4)**, **P9.6 (7)**, and the changed gate's half of **P9.7 (4)** (the pre-existing clone pairs and uncovered functions reported in files a range touched). The tree gate's half of P9.7 (4), `similarity::seeds` uncovered because it is a `const fn` evaluated at compile time, is measured by the tree's line ratio, which passes, and needs no rule change.

Not deferred: **P9.6 (6)** is a bug. Under a range, the dead-code check's module pass reads only the scope's import edges, so a `pub` item of a changed file that an unchanged file imports reads as `dead.export`: a finding invented, not missed (at `info` severity and low confidence, so no gate fails on it). It stays on the follow-up list as item (21). **P9.4 (5)**, the mutation slots that fill the disk, is follow-up item 19, closed by P9.F1.

## Consequences

- Every follow-up item is either closed by a P9 step or deferred here with a trigger, except (21), which a P9 step or the integrator fixes before v0.2.0 or defers in its own ADR with a reason.
- A deferral is reopened by evidence, not by a calendar: each trigger above is something a dogfood report, a user, or a runner change produces.
- Items 11 and 14 leave manual steps in P9.10's release verification: start the aarch64 Linux and x86_64 macOS binaries by hand, and run a wrapper on Windows if one is at hand.
