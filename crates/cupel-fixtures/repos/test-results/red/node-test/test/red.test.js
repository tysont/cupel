// ABOUTME: The red-baseline overlay of the node:test fixture: one failing test whose assertion message is
// ABOUTME: "totalling 2 and 2".
const { test } = require("node:test");
const assert = require("node:assert/strict");
const { total } = require("../tally");

test("reports a wrong total", () => {
  assert.equal(total([2, 2]), 5, "totalling 2 and 2");
});
