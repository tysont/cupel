import assert from "node:assert/strict";
import { mock, test } from "node:test";
import { classify, label } from "../src/decide.js";

test("classifies overdrawn balances", () => {
  assert.equal(classify(-5, 10), "overdrawn");
});

test("does not call an empty balance rich", () => {
  assert.notEqual(classify(0, 10), "rich");
});

test("labels through a stub", () => {
  const describeKind = mock.fn(label);
  assert.ok(describeKind("deposit"));
});

test("waits for the ledger server", (t) => {
  t.skip("needs a ledger server");
  assert.deepEqual(classify(1, 0), "rich");
});
