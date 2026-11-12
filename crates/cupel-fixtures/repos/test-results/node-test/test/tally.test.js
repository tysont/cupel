// ABOUTME: The node:test suite of the test-results fixture: one passing test and one skipped, in a
// ABOUTME: `describe` block, so a test's ID names its file, its suite, and its name.
const { describe, test } = require("node:test");
const assert = require("node:assert/strict");
const { total } = require("../tally");

describe("total", () => {
  test("totals a list", () => {
    assert.equal(total([1, 2, 3]), 6);
  });

  test("totals a huge list", { skip: true }, () => {
    assert.equal(total(new Array(1000).fill(1)), 1000);
  });
});
