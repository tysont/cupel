# CUPEL_DECISION_212: A `try` whose block ends in a failure call is one throws assertion, and its handlers are no logic

**Date:** 2026-11-14 · **Status:** accepted · **Phase:** v0.2.9, item V15 · Amends [184](CUPEL_DECISION_184.md) (what `test.logic` counts)

## Context
On FasterXML/jackson-core (3.x), 168 of 279 `test.logic` findings were tests whose only logic was one `alternatives`, from JUnit's expected-exception idiom written out:

```java
try {
    base64Variant.decode("-%8en$9m=>$m", byteArrayBuilder);
    fail("Expecting exception: IllegalArgumentException");
} catch (IllegalArgumentException e) {
    assertEquals(Base64Variant.class.getName(), e.getStackTrace()[0].getClassName());
}
```

(`Base64VariantTest.java:36`; evidence `{"alternatives": 1, "branches": 0, "guards": 8, "logic": 1}`). It is `assertThrows` spelled out: the test passes only if the call throws the caught exception, and the code runs one path. ADR-184 counts every catch clause as an alternative, and guards absorb only branches, so no number of assertions excused it. `fail` was a weak truthiness assertion, as it is anywhere. Python has the same idiom with `pytest.fail` or `self.fail` before `except`.

Open: how `tests.quality`, which knows no language, tells this catch from a catch that handles an error the test did not expect.

## Options
(a) Count catches as branches, which guards absorb: then any try/catch in a test with an assertion is no logic, including one whose `try` falls through (`try { x = parse(); } catch (E e) { x = -1; }`), which is logic. (b) Drop the catch from the language's `decisions.scm` when its `try` ends in `fail`: the function's complexity would then depend on a test idiom, and cyclomatic and cognitive complexity are about control flow, not intent. (c) Classify the idiom in `tests.scm`: the `fail` call ending the `try` block is a `throws` assertion, and the assertion's span is the whole `try` statement, so the check can see that the handler belongs to it. The check then leaves out of `alternatives` each catch decision a throws assertion's span holds, as it already reads a loop holding an assertion as a table (ADR-184).

Chosen: (c). It keeps the check free of language names and the decisions unchanged, and the idiom has one assertion, not two.

## Decision
1. **Capture.** `tests.scm` may put `@assertion.extent` on a node enclosing the match's `@test.assertion`; the assertion's span is then that node's. The earliest pattern still wins per assertion node.
2. **Java.** A `try` statement with one or more catch clauses whose block's last statement is a call named `fail` (`fail(…)`, `Assert.fail(…)`, `Assertions.fail(…)`) makes that call the idiom `expected-exception`, `throws`/`strong` as `assertThrows` is, spanning the `try`. A `fail` elsewhere stays `truthiness`/`weak`.
3. **Python.** The same for a `try` with one or more `except` clauses whose block ends in `pytest.fail(…)` or `self.fail(…)`: `expected-exception`, `throws`/`strong` as `pytest.raises` is.
4. **The check.** `test.logic` leaves out a `catch` decision whose span a `throws` assertion of the case holds. Other decisions inside the `try` or its handlers (an `if` in a catch) count as before. Assertions in the handlers are assertions as before.

## Consequences
- The jackson-core idiom is no `test.logic`; a try/catch whose block does not end in `fail` still is.
- A `try` with a `finally` but no catch is not the idiom (it has no handler to excuse).
- A catch inside any throws assertion is left out, so a try/catch written inside the lambda of `assertThrows(…)` is not counted either; it was not seen, and such a lambda is the code under test.
- Fixture: `crates/cupel-fixtures/repos/expected-exception/java`.
