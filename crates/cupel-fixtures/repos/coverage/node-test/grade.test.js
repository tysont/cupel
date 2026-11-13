// ABOUTME: The node-test coverage fixture's suite, on node:test and node:assert: it covers total fully,
// ABOUTME: classify partly, and refund not at all.
const test = require("node:test");
const assert = require("node:assert");
const { classify, total } = require("./grade");

test("totals a list", () => {
  assert.strictEqual(total([1, 2, 3]), 6);
});

test("classifies passing scores", () => {
  assert.strictEqual(classify(95), "A");
  assert.strictEqual(classify(60), "pass");
});
