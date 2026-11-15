# CUPEL_DECISION_220: A loop that neither asserts nor branches builds inputs and is no test logic

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 1 · Amends [054](CUPEL_DECISION_054.md) and [184](CUPEL_DECISION_184.md) §2

## Context
The v0.2.9 validation replayed hashicorp/raft (Go). `tests.quality` reported 51 `test.logic` findings; 38 of them held loops only: setup loops building servers and inputs, and polling loops. 8 more held `if !reflect.DeepEqual(a, b) { t.Fatalf(…) }` equality guards, which are already guards. `configuration_test.go:301` (`TestConfiguration_encodeDecodePeers`) is the typical case: `for i := 0; i < 3; i++ { configuration.Servers = append(configuration.Servers, Server{…}) }` builds a three-server configuration, then the test encodes, decodes, and compares it once; the finding counted `loops: 1`. The validation recommended (NOTES Q-go-1): "a loop with no assertion and no branch inside (pure construction of fixtures) is not test.logic". The owner's ruling (2026-11-14): "setup loops are not test logic."

Today's rules (ADR-054, ADR-069, ADR-184 §2): a loop over a table written in place, or whose body holds one of the case's assertions, is table-driven (`tables`) and no logic; every other loop is logic (`loops`). A loop that asserts on each pass therefore already is no logic, and stays so. What the ruling leaves open is which of the remaining loops are setup.

## Options
(a) Every loop without an assertion is setup. Then no loop is ever logic, since a loop with an assertion is a table; and a loop that decides what it builds (`for i, r := range c.rafts { if i == 0 { server.Suffrage = Nonvoter } … }`, raft_test.go:105) would read as straight-line setup. (b) A loop without an assertion and without a decision inside it other than another such loop or a boolean operator: it runs the same statements on every pass, building inputs. A branch, a `case` arm, a conditional expression, an exception handler, or an early `return` inside it means the loop chooses what happens, which is logic as a branch in the body is. (c) Recognize setup by what the body does (appends, assignments, constructor calls): language-specific, and a check cannot name a language.

Chosen: (b). It is the validation's definition, needs only the decisions and assertions `tests.quality` already reads, and holds in every language.

## Decision
1. **A setup loop** is a loop of a test case that is not table-driven (ADR-069, ADR-184 §2: it iterates no table written in place, and its span holds none of the case's assertions) and whose span holds no branch, `case` arm, conditional expression, exception handler, or early return. A boolean operator in it (a loop condition `i < n && ok`) and a loop nested in it, itself a setup loop, leave it a setup loop; a nested loop holding a branch puts that branch in the outer loop's span too, so neither is setup. A loop whose decision carries no span cannot be placed and stays a loop.
2. A setup loop is no logic. `test.logic` counts `loops + alternatives + branches beyond the guards` as before, with setup loops left out of `loops`; its evidence gains `setup`, the number of setup loops, beside `loops` and `tables`.
3. A loop holding an assertion keeps today's classification: a table, no logic. A loop that branches without asserting stays a loop and logic, as does every branch inside it, counted as before.
4. The rule is language-free: it applies in every language `tests.quality` grades, since it reads only `Decisions` and `TestFrames`.

## Consequences
- raft's setup loops (`configuration_test.go:301` and the like) are no longer `test.logic`; a test whose only logic was such a loop has no finding. Polling loops that branch (`for { if done() { break } … }` holding an `if`) stay logic, as does raft_test.go:105's loop that sets one server's suffrage.
- Rust, Python, Java, JavaScript, TypeScript, and Clojure tests that build inputs in a loop (`for i in 0..3 { v.push(i) }`) are likewise no logic.
- `test.logic`'s evidence schema gains `setup` (the `describe tests.quality` golden changes).
- Fixture: `crates/cupel-fixtures/repos/setup-loops/go`.
