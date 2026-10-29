# CUPEL_DECISION_154: A check's needs are made within the run's depth, or the check is skipped for that language

**Date:** 2026-10-29 · **Status:** accepted · **Phase:** P10

## Context

The depth (`--depth`, `depth` in `cupel.toml`) selects the checks whose cost class is at or below it (design §5.8), and ADR-080 gates a producer's *wants* by the same depth: a want is made only when some producer of it estimates making it within the depth (ADR-083 §4). A check's *needs* had no such gate: the fact store makes a need with the first ready candidate whatever that producer costs.

The two disagree for `conc.race`. The check is `standard` (ADR-105: Go's `-race` and ThreadSanitizer run the suite once under a race detector), but for Rust its producers are `tool.tsan` (`standard`, nightly with a sanitizer target) and `tool.miri` (`deep`), and where only Miri is ready a `standard` run interprets every test of the workspace under Miri. On Cupel's own repository, `cupel check --changed --depth standard` (P10's inner loop) ran `cargo +nightly miri test` over `cupel-checks` for more than five hours (2026-10-29, P10 Notes "P10 — the depth of a check's needs"); the dogfood never saw it because `make dogfood` passes `--skip conc.race`. Every other producer classed `deep` (differential, metamorphic, bench, fuzz, linearizability, the per-test maps) serves only `deep` or `exhaustive` checks, or is a want.

## Options

1. **Reclassify `conc.race` as `deep`.** Go's and ThreadSanitizer's runs, which are one suite run each and belong at `standard`, would leave the default run too.
2. **Reclassify `tool.miri` as `standard`.** Miri interprets every test; a suite of minutes takes hours. The class would be false.
3. **Skip `conc.race` in Cupel's own `cupel.toml`.** Fixes Cupel's inner loop and no one else's: every Rust repository with a nightly Miri installed pays the same at `standard`.
4. **Gate needs by the depth, per language.** A check is skipped for a language, with a reason that says so, when the producer the store would choose for one of its needs estimates its cost beyond the run's depth; the check still runs for the languages whose producers are within it.

## Decision

Option 4.

1. **The rule.** When the scheduler splits the selected checks into tasks (`Prepared::add`), for each language the support matrix allows, it asks for each of the check's needs which producer the store would choose there (`Deciding::chosen`: the first ready candidate after pins and detection, exactly the store's own choice) and that producer's estimate (`Producer::estimate`, so a cached producer counts as its cache makes it). When an estimate's cost is above the *bound*, the check is skipped for that language and the producer is never run for it.
2. **The bound** is the greater of the run's depth and the check's own cost class. A check selected by depth has a cost within the depth, so the bound is the depth. A check selected by `--only` whatever its cost (design §5.8) keeps its own cost as the bound, so `cupel check --only gen.fuzz` at the default depth runs the fuzz producer as before, while `cupel check --only conc.race` at `standard` does not run Miri.
3. **The skip.** `SkipReason::Depth` (`depth` in the envelope and events), with `fact` the need and `see` the command that runs it, `cupel check --depth <the producer's cost> --only <check>`. A check skipped alike in every language has one entry naming no language (ADR-064). The dry run lists it under `would_skip` with the same reason.
4. **Unchanged.** The store's choice of producer, wants (ADR-080, ADR-083), joints (ADR-084), and every check's cost class. A need the store would make with a producer within the bound is made as before.

## Consequences

- At `standard`, `conc.race` runs for Go and, where ThreadSanitizer is the chosen producer, for Rust; where Miri is chosen, the check is `skipped: depth` with `fact: race-reports` and `see: cupel check --depth deep --only conc.race`. At `deep` nothing changes.
- The rule compares the store's *chosen* producer, not the cheapest ready one: where Miri precedes ThreadSanitizer in the candidate order and both are ready, `conc.race` is skipped at `standard` though ThreadSanitizer could have run. Choosing a cheaper candidate is a change to the store's choice (ADR-076) and is not made here.
- Only a check's direct needs are compared. A need whose own producer needs a fact only a deeper producer makes is not gated; no such chain exists among the registered producers.
- The envelope's `skipped.reason` gains one value; the schema snapshot changes additively.
