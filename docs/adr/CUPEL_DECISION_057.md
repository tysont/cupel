# CUPEL_DECISION_057: `tests.weakening`: matching tests across a range, the five kinds, and snapshots

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P2.7

## Context

Design §5.2 lists `tests.weakening` as a diff of the test suite against the base with kinds `assertion.removed`, `test.skipped`, `matcher.loosened`, `snapshot.rewritten`, and `test.deleted`, all `severity: fail`. It does not say how a test on one side is matched to the other, what counts as a loosened matcher, how a snapshot is tied to a test (`TestFrames` records a `Snapshot` assertion but not which file holds the snapshot), how a finding about a test that no longer exists is located and identified, or what the score and the gate key `tests.weakening = "none"` read. ADR-056 supplies the base-side `TestFrames` and the range's changed paths.

## Options

**Matching.** By file and name only misses a test moved to another file; by name alone confuses same-named tests in different files. Matching by file and name first, then by name among the head's unmatched tests, covers both; repeated names within a file (parameterized or shadowed) pair in order of appearance.

**Assertion strength.** Comparing assertions positionally is fragile (an inserted line shifts everything). Comparing counts — total, strong, weak — per matched test is robust and explains itself: fewer assertions is a removal; strong ones replaced by weak ones is a loosening.

**Snapshots.** Reading snapshot contents is impossible for a check and unnecessary: a snapshot file the range *modified* (it exists on both sides) is a rewritten expectation. Tying it to tests needs a path rule. The frameworks Cupel's languages use store snapshots in a `__snapshots__` or `snapshots` directory beside or below the test file (jest/vitest `__snapshots__/<file>.snap`, insta `snapshots/<module>__<test>.snap`, go-snaps `__snapshots__/<file>.snap`, syrupy `__snapshots__/<file>.ambr`, pytest-snapshot `snapshots/<file>/<test>/`), or use a `.snap`/`.ambr` extension. Putting the rule in each language's `TestConventions` would be cleaner but is a fact change across five languages for a heuristic no other check needs yet.

## Decision

1. **Scope.** `needs = [TestFrames]`, `base_needs = [TestFrames]` (ADR-056): it runs only under `--changed` or `--base` and is otherwise skipped with `needs-range`. Both sides cover only the range's changed files, so unchanged tests are never compared.
2. **Matching.** Each base test is matched to the head test with the same file and name (the n-th same-named test in a file to the n-th); a base test still unmatched is matched to the first unmatched head test with the same name in another file (a move); a base test still unmatched is deleted.
3. **Kinds** (all `severity: fail`, action `restore_test`, confidence 1.0):
   - `test.deleted`: a base test with no match. Located at its base file and line (the file may no longer exist).
   - `test.skipped`: a matched test that has a skip marker at head and had none at base.
   - `assertion.removed`: a matched test with fewer assertions at head than at base.
   - `matcher.loosened`: a matched test whose strong assertions decreased while its weak ones increased (`min(strong lost, weak gained)` assertions loosened). A test can be both `assertion.removed` and `matcher.loosened`.
   - `snapshot.rewritten`: a snapshot path (a path segment `__snapshots__` or `snapshots`, or extension `.snap`/`.ambr`) that the range modified. It is attributed to the head tests with a `Snapshot` assertion whose file's directory contains the snapshot and whose file stem or test name appears in the snapshot's path below that directory; when a test name appears, only those tests. If any attributed test is new (unmatched at base), the snapshot file may only have grown (jest and go-snaps append a new test's snapshot to the file's one snapshot file), and nothing is reported. One finding per snapshot path, located at the first attributed test, or at line 1 of the snapshot file when no changed test file claims it (the usual case: only the snapshot changed). Inline snapshots (`toMatchInlineSnapshot`) change the test file itself and are not detected.
4. **Identity.** Tests are not symbols (a `it("…")` callback has no name), so findings carry no `symbol`. IDs are `FindingId::for_subject(check, kind, subject)` with subject `<file>::<test name>` (`#<n>` appended for the n-th repeat, n ≥ 1), using the head file for matched tests and the base file for deleted ones, and the snapshot path for `snapshot.rewritten`. Repro: `cupel check --only tests.weakening --base <base commit>`.
5. **Evidence.** `test.deleted` `{test, file, line}` (base side); `test.skipped` `{test, reason}`; `assertion.removed` `{test, base, head}` (assertion counts); `matcher.loosened` `{test, strong_base, strong_head, weak_base, weak_head}` (flat, since evidence schemas take no `$ref`, ADR-043); `snapshot.rewritten` `{snapshot, tests}`.
6. **Score and metric.** Score `{value: findings, unit: "weakened", n: tests compared}`; languages combine by summing values. Metric `weakened` (the count) is the gate key; `tests.weakening = "none"` in `[gate.changed]` passes exactly when there is no finding. The check has no settings; any key under `[checks."tests.weakening"]` is a configuration error.

## Consequences

- An agent that deletes, skips, or waters down a test, or regenerates a snapshot, fails the gate at the change that did it, with the base location to restore from.
- Renaming a test is reported as `test.deleted` (a rename cannot be told from a delete plus an unrelated addition by name); moving it to another file is not.
- A jest/go-snaps snapshot file shared by several tests attributes the rewrite to every snapshot test of the file when no test name appears in the path; the finding is still one per file.
- Adding a snapshot test hides a rewrite of the same snapshot file in the same range; the alternative fails every legitimate new jest snapshot test.
- A range that changes only snapshot files and no source file has no language in scope (ADR-056 §5), so the check does not run; in practice a rewritten snapshot accompanies the code change that made it necessary, which brings its language into scope.
- The snapshot rule is path-based and language-neutral; a framework that stores snapshots elsewhere is missed until `TestConventions` carries snapshot locations (a fact change with its own ADR).
