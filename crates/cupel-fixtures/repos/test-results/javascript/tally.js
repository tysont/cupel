// Tally: a passing test suite for Cupel's runner producers.
function total(values) {
  return values.reduce((sum, value) => sum + value, 0);
}

module.exports = { total };
