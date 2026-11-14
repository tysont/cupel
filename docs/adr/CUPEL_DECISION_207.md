# CUPEL_DECISION_207: A Go test asserts through a method it hands `t` when the package defines that method asserting, and not through a helper the package defines without an assertion

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.9, item V5 · Details [138](CUPEL_DECISION_138.md) §8 (a Go test that hands its `t` to a helper asserts)

## Context
ADR-138 made a Go call of a plain function name with `t`, `b`, or `tb` among its arguments an assertion: as the helper does when a test file of the package in scope defines it, and as custom and strong otherwise. Two shapes of Go's idiom fell outside it on hashicorp/raft:

- **A method helper.** All 8 `test.no-assertion` findings were tests of `fuzzy/` that end `cluster.Stop(t, time.Minute); cluster.VerifyLog(t, ac); cluster.VerifyFSM(t)`, where `fuzzy/cluster.go` (not a test file) defines `func (c *cluster) VerifyFSM(t *testing.T)` calling `t.Errorf`. A call through a selector was no helper call at all.
- **A setup helper.** A plain call such as `dir := scratch(t)` whose definition only calls `t.Helper()` and `t.TempDir()` counted as a strong assertion, so a test doing nothing else did not read as asserting nothing.

The resolver already sees every file of the language in scope (`resolve_helpers`), test files or not, and matches helpers by name within a directory (`directory_helpers`), which is a Go package. A selector call does not say whether its operand is a value of the package's type or another package (`testutil.Check(t, …)`): Go's syntax spells both `x.F(t)`.

## Options
**A selector call handing `t`.** (a) An assertion only when a function or method of that name in the package's files in scope asserts, as Python's `self.helper(…)` and ECMA's helpers resolve. (b) As a plain call: custom and strong unless resolved. Every `pkg.Setup(t)` of another package (`fstest`, a repository's `testutil`) would assert, hiding tests that assert nothing. (c) Resolve the operand's type: needs type information the queries do not have.

**A plain call naming a function the package defines without an assertion.** (a) No assertion: the package's files show what it does, and it does not fail the test. (b) Custom and strong as before: setup helpers keep hiding tests that assert nothing.

## Decision
1. **A selector call handing `t`, `b`, or `tb`** (other than on `t` itself and testify's and go-snaps' `assert`, `require`, `snaps`) is a helper call by its field name and not an assertion by itself. It asserts, with the class of the helper's first strong assertion, else its first, when a function or method of that name in the same directory, in any file in scope, asserts (Option a).
2. **A plain call handing `t`** asserts as ADR-138 says when the package defines an asserting function of that name, and is no assertion when the package's files in scope define that name only without an assertion (Option a). A name nowhere defined in scope stays custom and strong, so a `--changed` run that does not read the helper's file keeps reading the call as asserting.
3. The rule is the resolver's, for every language with `directory_helpers`: a call already read as an assertion that names only functions of its directory holding none is no assertion. Only Go reads a helper call as an assertion before resolving it, so only Go changes.
4. One level, as before: a helper's assertions through its own helper calls are not followed; a method helper that only calls another method helper asserts nothing.

## Consequences
- hashicorp/raft's 8 `test.no-assertion` findings in `fuzzy/` go away under a whole-tree run.
- A `--changed` run whose scope holds a test but not the file defining its method helper reads that test as asserting nothing (Python and ECMA helpers behave the same). A plain function helper stays custom and strong there.
- Names are matched without receiver types: two methods of one name in a package, one asserting, make every call of that name assert.
- A setup function of the package that tests hand `t` (`scratch(t)`, `setupDB(t)`) no longer counts as an assertion, so a test that only sets up is `test.no-assertion`.
