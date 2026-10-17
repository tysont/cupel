// Sums for the perf.bench fixture.
function total(values) {
  return values.reduce((sum, value) => sum + value, 0);
}

module.exports = { total };
