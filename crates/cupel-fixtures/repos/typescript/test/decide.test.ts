import assert from "node:assert/strict";
import { test } from "node:test";
import { classify, parseCents } from "../src/decide";

test("classifies overdrawn balances", () => {
  assert.equal(classify(-5, 10), "overdrawn");
});

test("rejects empty amounts", () => {
  assert.throws(() => parseCents(""), /empty amount/);
});

test("waits for the ledger server", (t) => {
  t.skip("needs a ledger server");
  assert.ok(classify(1, 0));
});
