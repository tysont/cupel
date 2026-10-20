# CUPEL_DECISION_134: `meta.traceability`: `REQ-<n>` markers in test names and doc comments as a secondary source

**Date:** 2026-10-20 · **Status:** accepted; amends §1 and §2 of [114](CUPEL_DECISION_114.md) · **Phase:** P9.5

## Context

ADR-114 made configured requirement IDs (`[checks."meta.traceability"] requirements`) the only source of requirements and a test's name the only place it names one. Design open question 5 leans toward "both": requirement IDs embedded in tests (`#REQ-12`) as well as a list. The owner's P9 follow-up item 5 keeps configured IDs primary and makes `REQ-<n>` markers in tests the secondary source, with the test's doc comment or docstring counted as well as its name. Neither ADR-114 nor the design says:

1. Which spellings are a marker, per language, and where in a test they are read (name, doc comment, body).
2. What the check reports for a marker that names a requirement the configuration does not list.
3. Whether a configured requirement that a test names only in its doc comment is traced.
4. How the evidence says where a requirement came from.

`TestFrames` (ADR-114's only input) carries each case's name and nothing of its documentation, so reading doc comments is a change to the fact every language produces.

## Options

**Where a marker is read.** (a) Names only: no fact change, but Go, Java, and Rust test names are identifiers, where `TestParsesWeekDatesREQ12` cannot carry a whole-word ID and a separator in a Go or Java test name is unidiomatic. (b) Names and doc comments: every language has a place above (or, for Python, inside the top of) a test for prose; it needs a `doc` field on `TestCase` and a conformance construct per language. (c) Names, doc comments, and bodies: a body's strings and comments mention IDs for many reasons (a fixture's data, a `testing` label, a commented-out case), so a body mention is weak evidence and would match by accident.

**A marker no configuration lists.** (a) Ignore it: the secondary source then adds nothing beyond what configuration already says. (b) Treat it as a requirement of the task, traced by the test that carries it, and say so: a repository that writes IDs only into tests gets the check without configuration, and a repository that also configures them learns of a marker the task does not list (often a mistyped ID). (c) Treat every marker in scope as a requirement, including those on tests that existed before the change: an edited test file's older marked tests would then each report an untested requirement the task never asked for.

**How a listed marker requirement is reported.** A `CheckResult` has no structured place for per-requirement data other than findings. A `note` is prose and is for what a result could not take into account (ADR-079). An `info` finding is listed, carries evidence, and fails no gate (`none` counts `warn` and `fail`), as `perf.improvement` does.

## Decision

1. **`TestCase.doc`** (TestFrames version 3): the text of the case's doc comment or docstring as written (comment markers, quotes, and line breaks kept), `None` without one. Per language: Rust, the `///` lines or `/** … */` block directly above the test's attributes; Go, the `//` lines directly above `func TestX`; Python, the docstring (the body's first statement, a string); JavaScript and TypeScript, a `/** … */` block directly above the `it`/`test` call's statement; Java, a `/** … */` Javadoc directly above the method (its annotations are part of the method); Clojure, the `;` comment lines directly above the `deftest` or `defspec` form (`deftest` takes no docstring). `tests.scm` captures it as `@test.doc`, with `@doc.target` on the node it documents; it becomes the doc of a case another pattern found there, and makes no case (so roots and every other reader of `@test.case` are unchanged). The `test-frames` conformance inventory gains the construct `doc`, mapped in every language's fixture.
2. **A test names a requirement ID** when its name or its doc carries the ID as whole words, by ADR-114 §2's rule (lowercase; every run of characters other than letters and digits one `_`; `_<id>_` in `_<text>_`). A body is not read. A configured requirement that an added test names only in its doc is traced.
3. **A marker** is the word `req` followed directly by a word of digits in a test's name or doc, under the same rule: `#REQ-12`, `REQ-12`, `req_12`, `test_req_12_parses`, `TestParse_REQ_12`, `/// Parses week dates (REQ-12).`; not `REQ12`, `testReq12`, or `requirement 12`. Its ID is written `REQ-<digits>`, the digits as written.
4. **Sources.** Each requirement has a `source`: `config` for an ID configured in `requirements` (the check's table or a per-language override), `marker` for a marker on a test the range added whose ID matches no configured requirement. Markers on tests that existed at the base add no requirement. `spec` is not a value: ADR-114 §1 offers no `--spec FILE`; a spec file, when one is built, adds it.
5. **Reporting.** An untested requirement is `spec.untested-requirement` as before; only a `config` requirement can be untested, since a `marker` one is traced by the added test that names it. A `marker` requirement is `spec.unlisted-requirement` (`info`, action `configure`, confidence 1, no location, ID by the requirement), titled by how many tests the change added name it, summarised "list it under `requirements` if the task asks for it, or correct the marker". Both kinds' evidence is `{requirement, source, base, tests: [{file, name, line, added}]}`: `source` is added to `spec.untested-requirement`'s evidence, and the tests are every test in scope that names the requirement. The score's `n` and the plan count both sources; the metric `untested` and the status (fail only on an untested requirement) are unchanged. With no requirement from either source the check passes with ADR-114's note. The manifest's version becomes 2.

## Consequences

- A repository that writes `REQ-<n>` into its tests' names or doc comments gets traceability with no configuration, and every requirement the change touched is listed with its tests; configuring IDs still makes an untraced one a finding, which a marker cannot (nothing lists what the task asked for).
- A configured requirement can be traced from a test's doc comment, so Go, Java, and Rust tests keep idiomatic names.
- `TestFrames` grows by each documented case's doc text; caches of version 2 are invalidated. Every `tests.scm` gains doc patterns, and a language added later maps `doc` or marks it `not-applicable` with a reason.
- A requirement ID that is not `REQ-<n>` (`AUTH-3`, `#1234`) is still configured to be traced; only the `REQ-<n>` shape is recognised without configuration.
- A Clojure `^{:doc …}` on a `deftest`, a JUnit `@DisplayName`, and a `describe` block's title are not read; each is a new pattern, or a name source, when asked for.
