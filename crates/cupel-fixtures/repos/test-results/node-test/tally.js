// ABOUTME: Tally: a passing test suite for Cupel's runner producers, run by Node's built-in test runner.
// ABOUTME: `total` sums a list; test/tally.test.js tests it with node:test.
function total(values) {
  return values.reduce((sum, value) => sum + value, 0);
}

module.exports = { total };
