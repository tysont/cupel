# CUPEL_DECISION_114: `meta.traceability`: requirement IDs from configuration, traced by the names of tests the range added

**Date:** 2026-10-17 · **Status:** accepted; §2 (a test names a requirement by its name only) and §1 (requirements come only from configuration) amended by [134](CUPEL_DECISION_134.md) (`REQ-<n>` markers and doc comments) · **Phase:** P7.6

## Context

Design §5.7: `meta.traceability` checks that "each requirement ID in the task spec (config or `--spec FILE`) maps to ≥1 test that fails without the change", kind `spec.untested-requirement`, cost standard. Open question 5 (§14) asks whether it needs a spec file format or accepts requirement IDs embedded in test names (`#REQ-12`), leaning toward both. Plan §2.5 reserves ADR-008 for this decision at P7.6 and the plan's P7 heading cites "(ADR-008)", but no ADR-008 file was ever written; that reference is stale, and this ADR is the decision.

Open: where the task's requirements come from, given that a check reads only facts and its own `[checks."<id>"]` table (§8.3.4); how a test is tied to a requirement; how "fails without the change" is established; and what happens in a repository of several languages.

## Options

**Where requirements come from.** (a) `[checks."meta.traceability"] requirements = [...]`: the check's own configuration, which the check already reads, per-language overrides included. (b) `--spec FILE`: a file the check cannot read (no file system in a check); it needs a producer that parses the file into a new fact kind (a catalog row, schema, and inventory, §8.3.1) and a CLI flag every verb would document. (c) Both. (a) is complete for an agent loop (the agent or its harness writes the IDs into `cupel.toml`); (b) adds a format to design and maintain without a user asking for one yet.

**How a test is tied to a requirement.** (a) The ID in the test's name, as `TestFrames` gives it: every language's producer already names every case, and the matching is language-neutral once case and separators are ignored (`test_req_12_parses`, `it("parses #REQ-12")`, Go's `TestReq12…` needs a separator). (b) A comment or docstring tag: not in `TestFrames`, so a shape change and conformance construct for every language. (c) A mapping in the spec file: needs (b) of the previous question.

**"Fails without the change."** (a) Test names only: any test naming the ID traces it, which a test that existed long before the change satisfies without showing anything about the change. (b) The base side of the range (`base_needs = [TestFrames]`, as `tests.weakening` reads it, ADR-056): a test that existed at the base commit passed there (the base is the green state the change starts from), so it cannot fail without the change; only a test the range added can. This is a necessary condition, computed from syntactic facts alone, with no run. (c) Mutation kills (`MutationOutcomes`' killing tests): a test that kills a mutant of the changed code depends on it; but `wants` are made whenever the depth allows, so every `standard` run of this check would start the mutation engine, and a kill is evidence about a mutant, not about the change. (d) Running the head's new tests against the base's code: the test itself, but an engine that builds the base with the head's tests, per language, a step of its own.

**Several languages.** A check runs once per language and cannot see the others' results; a requirement traced by a Python test would be reported untested for Rust in the same range.

## Decision

1. **Requirements are configured**, as `requirements`, a list of non-empty strings, in `[checks."meta.traceability"]`, the check's only setting (default `[]`, self-described, ADR-111); `[checks."meta.traceability".lang.<id>]` assigns requirements to one language. Any other key, or another shape, is a configuration error naming `requirements`. `--spec FILE` is not offered.

2. **A test names a requirement** when its `TestFrames` case name contains the ID as whole words: both lowercased, every run of characters other than letters and digits made one `_`, and `_<id>_` found in `_<name>_`. `REQ-12` matches `test_req_12_parses` and `parses #REQ-12`, not `test_req_120` or `testReq12`.

3. **A requirement is traced** when a test the range added names it: a head case that `tests.weakening`'s matching (same file, name, and repeat; else a move by name) pairs with no base case. The check declares `base_needs = [TestFrames]`, so it runs only under `--changed` or `--base` and is skipped `needs-range` otherwise; its matrix goldens record that skip.

4. **`meta.traceability`** (`cupel-checks/src/meta_traceability.rs`): tier 2, cost `standard` (as §5.7 says; the first version reads only syntactic facts, so it costs a parse), needs `TestFrames`, no wants, experimental. Each untraced requirement is `spec.untested-requirement` (`warn`, `write_test`, confidence 1, no location), ID by the requirement, titled by how many pre-existing tests name it, evidence `{requirement, base, tests: [{file, name, line, added}]}` listing the tests in scope that name it (all `added: false`). Score `{untested, unit "untested requirements", n: requirements}` and metric `untested`, summed across languages. No requirements configured: pass, no score, a note saying where to list them.

## Consequences

- An agent gets a finding per requirement it did not test, cheaply, at any depth from `standard`; the gate rule `"meta.traceability" = "none"` makes it a stop condition.
- (b) is necessary, not sufficient: a new test that would also pass on the base code traces its requirement. Running the range's new tests against the base's code (option (d)) is the follow-up that makes "fails without the change" exact; it would feed this check as a fact of base-side outcomes, with no change to the check's findings.
- An edited test that existed at the base (its expectations rewritten for the change) does not trace a requirement, even if it now fails without the change; the finding names it, so the reader sees why.
- In a range over several languages, each language reports the requirements its tests do not trace; per-language overrides of `requirements` keep each requirement with the language that tests it.
- A spec file (`--spec FILE`, or `requirements` read from a document) needs a producer and a fact kind of its own (an ADR, per §8.3.1) and is left until a format is asked for; the design's "(config or `--spec FILE`)" is met by the first half.
- The stale "(ADR-008)" in the plan is left as the plan wrote it; this ADR, not a file numbered 008, records the decision §2.5 reserved that number for.
