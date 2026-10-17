# CUPEL_DECISION_094: `gen.differential`: the check, `diff.divergence`, its skips, `--cases`, and its fixtures

**Date:** 2026-10-16 · **Status:** accepted · **Phase:** P5.3

## Context

ADR-093 builds the engine that makes `DivergenceResults`. Still open: the check's manifest and what it reports from that fact; the finding kind's name, severity, action, and evidence; what happens without an oracle, without entry points, with a `spec` oracle, and when nothing could be compared; how `--seed` and `--cases` reach a run and its repro; what ADR-091's "the check context gains [the oracle's definition]" becomes; whether `oracle verify` changes (ADR-090 said P5.3 would replace its `ref`/`command` comparisons); and how the matrix test can hold a golden for a check that needs an oracle and `[lang.<id>]` configuration, when it computes fixture support without that configuration and every unverified oracle adds an `oracle.unverified` finding the matrix refuses.

## Options

**The oracle in the check's context.** ADR-091 expected `CheckCtx` to carry the oracle's definition. The comparison runs in the engine (ADR-093 §2), which therefore receives the definition; the check needs only the oracle's name, kind, and commit for its evidence, and the fact carries them. Adding the definition to `CheckCtx` too would be a second, unused path.

**A `spec` oracle.** (a) Exclude `spec` oracles when the run chooses one for this check: selection (ADR-091 §2) is per run, not per check, and `spec.trace` (P7) will want exactly those. (b) Let the check skip itself when the run's oracle is a spec (ADR-048: a check may skip for its own reasons). (b).

**Nothing compared.** When every configured entry was out of scope or skipped, the check can skip, fail, or pass with a null score and notes. A skip hides the reasons (a skipped entry keeps no notes); failing reports a configuration problem as a code problem. `static.crap` without coverage already passes with a null score rather than guess (ADR-067).

**`oracle verify`.** Replacing its command smoke test with "same output as the implementation on the known inputs" makes verification depend on the code under test: an oracle that catches a bug could never be verified until the bug is fixed. Keeping verification about the oracle alone, and running the known inputs first in every comparison (ADR-093 §5), keeps both meanings.

**Fixtures in the matrix.** A ref oracle needs history, and fixtures are copied without `.git`; a command oracle runs anywhere. Support computed without the fixture's `[lang.*]` tables reports the engine `unconfigured` where a real run in that fixture runs it; verifying the fixture's oracle before its golden run is what a user does, and shows the tier-4 result.

## Decision

1. **`gen.differential`**: tier 4, cost `deep`, needs `DivergenceResults`, `Symbols`, and `NormTree`, `uses_oracle`, no settings, stability `experimental`. One finding per entry that diverged, located at the entry's function and identified by its structural hash (`FindingId::compute(check, kind, hash, None)`). Score `{value: divergent entries, unit: "divergent", n: entries compared}`; metrics `divergences` (entries that diverged) and `cases` (inputs compared), both summed across languages; when no entry was compared, the score is null and the metrics are empty, so a gate rule on them is `skipped` rather than passing on nothing (as `static.crap` without coverage, ADR-067). Status `fail` with a divergence. Each skipped entry is a note (`<entry> not compared: <reason>`).

2. **`diff.divergence`**: severity `fail`, confidence 1 (both sides were run on the same input and answered differently), action `fix_bug` ("Decide which answer is right for `input`: fix the implementation, or, if the change is intended, the oracle"), title `` `<function>` answers `<implementation>` but oracle `<name>` answers `<oracle>` for `<input>` ``. Evidence: `{entry, oracle: {name, kind, commit}, input, found, case, shrinks, implementation, reference}`, where `implementation` and `reference` are the two `Answer`s (ADR-093 §1) on the minimized `input`. Repro: `cupel check --only gen.differential --symbols <hash> --oracle <name> --seed <seed> --cases <cases>`, which regenerates the same inputs.

3. **Skips.** No oracle defined: not scheduled, skipped `unconfigured` with `see: cupel describe oracle add` (ADR-091 §2). No `differential_entries` for a language: the engine is `unconfigured` (`lang.<id>.differential_entries`), so the scheduler skips the language like any unconfigured producer. A language's program missing: `unavailable`. A `spec` oracle: the engine compares nothing and the check skips itself, `unconfigured` with `see: cupel describe gen.differential`. Entries configured but none compared: `pass`, null score, no metrics, the notes say why. Results against an unverified oracle are capped by grading (ADR-091 §4); the check only declares `uses_oracle`.

4. **The oracle's definition goes to producers, not to `CheckCtx`** (ADR-093 §2); this replaces ADR-091's expectation that the check context would carry it. The check reads the oracle's name, kind, and commit from the fact.

5. **`--cases N`** is the number of generated inputs per entry (default 100); it reaches producers as `Focus::cases` and loses its `pending_step`. `--seed` already reaches them (ADR-078). Neither is added to the envelope beyond `run.seed`.

6. **`oracle verify` is unchanged** (ADR-090 §3): it checks the oracle alone. "Same output as the implementation on N known inputs" is the comparison every `gen.differential` run makes first, on a command oracle's `inputs`.

7. **Fixtures and the matrix harness.** `repos/gen.differential/<lang>/` holds, per language, a module-level `quarter` function that rounds a division down where the shared command oracle (`oracle/quarter.sh`, POSIX shell arithmetic, which truncates) rounds toward zero, `[lang.<id>] differential_entries`, and `[oracles.quarter]` with known inputs on which both agree; the known divergence is the input `-1`. The matrix harness computes a fixture's support with the fixture's `[lang.*]` tables (support is per repository, ADR-079) for the matrix and for declared-facts runs, and runs `cupel oracle verify --all` in a fixture copy that defines oracles before the golden run. End-to-end tests of `ref` oracles build a git history in a temporary repository per language.

## Consequences

- `gen.differential`'s goldens show tier 4 and no `oracle.unverified` finding; a test without the verification shows the cap.
- Any fixture with `[lang.<id>]` keys now has them in the matrix's support computation, as in a real run.
- `--cases` reaches every producer; `gen.property` (P5.4) reads the same `Focus::cases`.
- A dry run's `would_run` entry for the check does not show the oracle or the case count (design §6.8's example does); the plan carries neither yet.
- Plugin checks still cannot use an oracle (P5.2 open item 4); a plugin producer of `DivergenceResults` would not receive the oracle either, since the plugin request carries no oracle.
