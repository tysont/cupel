/** The sum of values, as proofs/tally.dfy proves `Total` computes it. */
function total(values) {
  return values.reduce((sum, value) => sum + value, 0);
}

module.exports = { total };
