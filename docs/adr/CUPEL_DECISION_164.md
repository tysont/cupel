# CUPEL_DECISION_164: A mock outside every case is the file's, ECMA tests assert through their file's helpers, and Playwright's suites, steps, and hooks are not cases

**Date:** 2026-11-09 · **Status:** accepted · **Phase:** v0.2.6, Molten field trial Area 3 · Supersedes [027](CUPEL_DECISION_027.md)'s rule that "a mock outside every case belongs to every case in the file"

## Context

On the owner's Molten repository (Vitest and Playwright), `tests.quality` reported:
- 1,404 `test.mock-heavy`;
- 33 `test.no-assertion`;

and most were false.

1. **Module-level mocks were charged to every case.** ADR-027 gave a mock outside every case to every case in the file. A file that opens with `vi.mock("./embedding-scanner.js", () => ({ embeddingScan: vi.fn() }))` made each of its tests carry two doubles, including tests that call no mocked code (`isTrivialInput`). 516 of the 1,404 findings carried a module target.
2. **A case calling an asserting helper asserted nothing.** The generic test frames follow `@assertion.helper` calls into the file's functions (Rust, Go, Java). The ECMA query had no such capture. So `const before = (fn, first, second) => { expect(…) }`, called from a case, left the case with no assertion.
3. **Playwright's `test.describe`, `test.step`, and hooks were cases.** The case pattern took `test.<anything>(title, fn)`, and the symbol pattern made each such callback a named closure.

## Decision

1. **A mock outside every case is the file's.** `TestFile` gains `mocks`: the doubles created outside every case (a module-level `vi.mock` and its factory's `vi.fn`, a class-level `@Mock` field). They are installed for the whole file and are no case's. A case's `mocks` are those created inside it, or captured with it, as before. `tests.quality` reads only a case's own doubles, so `test.mock-heavy` says which tests build many doubles themselves.
2. **ECMA cases assert through their file's helpers.** `tests.scm` captures `@assertion.helper` on any call by a plain name, except the frameworks' own. The generic resolver (ADR-027) counts a call as an assertion only when it names a function of the file that asserts, whether a declaration or a `const` closure. It takes the class of that function's first strong assertion, else its first.
3. **Playwright's structure is not a case.**
   - In both `tests.scm` and `symbols.scm`, `test.<modifier>(title, fn)` is a case, or a case's closure, except for these modifiers: `describe`, `step`, `beforeEach`, `afterEach`, `beforeAll`, `afterAll`, `use`, `extend`, `fixme`.
   - `test.describe(…)` and `test.describe.<mode>(…)` are suites that namespace their cases, as `describe(…)` does.

## Consequences

- Molten: `test.mock-heavy` 1,404 → 35, `test.no-assertion` 33 → 2. Both remaining no-assertion findings are true.
- A file's module-level doubles are no longer in any per-test measure. A file mocking many modules is not reported as such. A file-level measure would be a new kind, and no repository has asked for one.
- In the fixtures, only the JavaScript and TypeScript goldens change: one `test.mock-heavy` finding whose second double was the file's module mock. The other languages' fixtures have no mock outside a case.
- A helper's own calls of helpers are still not followed (one level, ADR-027).
