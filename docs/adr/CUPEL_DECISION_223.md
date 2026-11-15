# CUPEL_DECISION_223: Python's `assert a in b` is a strong assertion

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.3.0, item 4 · Amends [034](CUPEL_DECISION_034.md); the Python counterpart of [184](CUPEL_DECISION_184.md) and [200](CUPEL_DECISION_200.md)

## Context
The v0.2.9 validation on encode/httpx found `assert "x" in str(headers)` (`tests/test_auth.py:319`) classified as a weak `custom` assertion: the Python `membership` idiom (`assert a in b`, `assert a not in b`, ADR-034) was `custom`/`weak`. ADR-184 made Jest's and Vitest's `toContain` and `toMatch` strong (a membership or a pattern pins the element or text it names), and ADR-200 extended that to Chai and Jasmine. NOTES Q-py-3 recommended Python `assert a in b` strong too, for consistency. The owner's ruling (2026-11-14): "assert a in b is strong."

## Options
(a) Strong for both `in` and `not in`, the one `membership` idiom both spellings share today. But a negated membership rules out one element and allows every other, which is why `!=` is weak (ADR-034) and Jest's `expect(x).not.toContain(y)` is the weak `negated` idiom while `toContain` is strong (ADR-184). (b) Strong for `in`, weak for `not in`, as the JavaScript matchers split. (c) Also unittest's `assertIn` (and `assertNotIn`): the same check spelled as a method; the ruling names the `assert` statement, so they stay as they are until ruled on.

Chosen: (b).

## Decision
1. `assert a in b` is the Python idiom `membership`, `custom`/`strong`.
2. `assert a not in b` is the idiom `not-membership`, `custom`/`weak`, as `membership` was.
3. unittest's `assertIn` and `assertNotIn` keep `custom`/`weak`.

## Consequences
- A pytest test whose only assertions are memberships (`assert "x" in str(headers)`) is no longer `test.weak-assertion`; one asserting only `not in` still is.
- `assertIn` and `assert a in b` now classify differently; making the unittest spelling strong as well is a separate ruling.
