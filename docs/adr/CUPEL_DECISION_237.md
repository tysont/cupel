# CUPEL_DECISION_237: `matcher.loosened` reads a transition table of precisions, apart from tests.quality's strength

**Date:** 2026-11-16 · **Status:** accepted · **Phase:** v0.3.0, item 17 (the owner's ruling on the httpx #3773 replay miss) · Amends [057](CUPEL_DECISION_057.md) §3 and §5; leaves [223](CUPEL_DECISION_223.md) as it stands

## Context

ADR-057 counts a matched test's assertions by strength. A test whose strong assertions decreased while its weak ones increased is `matcher.loosened`. Strength is tests.quality's classification: whether an assertion isolates a value well enough to count as a real check.

ADR-223 made Python's `assert a in b` strong, because a membership pins the element it names. On the v0.3.0 replay, encode/httpx #3773 changed

```python
assert response.encoding == "ISO-8859-1"
```

to

```python
assert response.encoding in ("ISO-8859-1", "WINDOWS-1252")
```

In v0.2.9 that failed the gate's `tests.weakening = "none"`, a true positive: the test accepts a second value it used to reject. In v0.3.0 both sides were strong, so the counts matched and the PR passed. The same blind spot covers every strong assertion that pins less than one value: Jest's `toContain`, `toMatch`, `toMatchObject` and `toBeCloseTo`, `pytest.approx`, and the matchers Cupel did not list. Testify's `Contains`, `InDelta` and `IsType`, AssertJ's `isIn` and `isCloseTo`, and Rust's `assert_relative_eq!` all took the default strong class.

The owner ruled (2026-11-16, verbatim): "ADR-223 stands — `assert a in b` is strong in tests.quality, literal collection or not. The httpx #3773 miss is a tests.weakening layering bug: weakening must detect loosened transitions independently of tests.quality's isolation classification. Add a transition table to the matcher.loosened kind: equality → membership (any collection), equality → contains/startswith/regex, exact numeric → approximate/tolerance, specific value → any-style or type-only matcher, and the Jest/Vitest/Chai/pytest/Go/Rust equivalents. Fixture from httpx #3773 that fails the gate; a fixture where == becomes `in some_collection` fails the same way."

## Decision

1. **Precision, beside strength.** Each assertion in `TestFrames` carries a `precision`: how much of the expected value it pins. It is one of `exact`, `membership`, `partial`, `approximate`, `type` or `any`. A language table entry sets it with `precision = "…"`, and an entry without one is `exact`. Strength stays as it is, and tests.quality does not read precision. No assertion's kind or strength changes, except that chai's `closeTo` and `approximately` take the kind `equality` that `toBeCloseTo` has. They were unlisted, so their kind was `custom`.
2. **The transition table.** `matcher.loosened` fires on either transition and counts the larger:
   - **exact → weak**, as before: `min(strong lost, weak gained)`.
   - **exact → any looser form**: `min(exact lost, inexact gained)`, where inexact is every assertion that is not strong and `exact`.

   | From (`exact`) | To | Precision | Examples |
   | --- | --- | --- | --- |
   | equality | membership, of a literal or any other collection | `membership` | pytest `assert a in b`; Jest/Vitest `toContain`; Chai `include`, `contain`, `assert.include`; testify `Contains`; AssertJ `isIn` |
   | equality | contains, prefix, suffix, pattern, part of an object | `partial` | Jest/Vitest `toMatch`, `toMatchObject`, `expect.objectContaining`, `arrayContaining`, `stringContaining`, `stringMatching`; Chai `match`, `assert.match`; testify `Subset`, `Regexp`; AssertJ `startsWith`, `endsWith`, `matches`, `containsPattern`; Rust `assert_matches!` |
   | exact numeric | approximate, tolerance | `approximate` | pytest `approx`; unittest `assertAlmostEqual`; Jest/Vitest `toBeCloseTo`, `expect.closeTo`; Chai `closeTo`, `approximately`; testify `InDelta`, `InEpsilon`, `WithinDuration`; AssertJ `isCloseTo`; approx `assert_relative_eq!`, `assert_abs_diff_eq!`, `assert_ulps_eq!` |
   | specific value | type-only matcher | `type` | Jest/Vitest `expect.any(T)`; Chai `a`, `an`, `instanceOf`; testify `IsType`, `Implements`; AssertJ `isInstanceOf`, `hasSameClassAs` |
   | specific value | any-style matcher | `any` | `unittest.mock.ANY` (`assert x == ANY`); Jest/Vitest `expect.anything()` |
   | any strong | weak | (strength) | as ADR-057: truthiness, presence, inequality; this already covers `assert s.startswith(…)`, `assert isinstance(…)`, `assert!(s.contains(…))`, testify `True`, Jest `toBeInstanceOf`, and Clojure's unlisted predicates (`contains?`, `re-find`, `instance?`) |

   The testify entries include their `…f` forms. A Jest/Vitest equality matcher (`toBe`, `toEqual`, `toStrictEqual`, `toHaveBeenCalledWith` and its variants, `toHaveReturnedWith`) is keyed by the asymmetric matcher it is given: as its argument, as an element of an array argument, or as a property value of an object argument. It takes that matcher's precision.
3. **Evidence.** `matcher.loosened` adds `exact_base`, `exact_head`, and `looser`: the looser forms the test has more of at head, `weak` or a precision. The finding's summary names them: "…instead of membership checks". When `looser` is only `weak`, the title and summary are unchanged.
4. **Versions.** The `test-frames` fact goes to version 4 and `tests.weakening` to version 3, so cached facts and results from before this ADR are not reused.

## Consequences

- httpx #3773 fails the gate again. The fixtures in `crates/cupel-fixtures/tests/weakening.rs` cover it:
  - `httpx_3773_equality_to_membership_of_a_literal_tuple_fails_the_gate` (the PR's change on the PR parent's test)
  - `equality_to_membership_of_any_collection_fails_the_gate_the_same_way` (`in ENCODINGS`)
  - `each_loosened_transition_is_a_matcher_loosened_naming_the_looser_precision`: 42 transitions across pytest, unittest, Jest, Vitest, Chai, testify, Rust, AssertJ and clojure.test
  - `a_looser_assertion_made_exact_again_is_not_a_loosening`: the same 42 transitions reversed
- tests.quality reports exactly what it did. A test whose only assertions are memberships is still not `test.weak-assertion` (ADR-223).
- Like ADR-057, this compares counts, not positions. A change that drops one exact assertion and adds an unrelated membership in the same test is reported as loosened.
- A transition the table cannot see stays unreported:
  - a Go or Rust guard (`if !strings.Contains(…) { t.Fatal(…) }`): every guard is strong `exact` (ADR-221)
  - JUnit's `assertEquals(expected, actual, delta)`: the same name as the exact form
  - an asymmetric matcher nested deeper than one array element or object property
  - matchers no table lists, which keep the default strong `exact` class
