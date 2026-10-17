// The number of decimal digits in `n`. The loop stops at five digits, so six-digit numbers are
// miscounted: the bug the property finds once it runs enough cases.
function digits(n) {
  let count = 1;
  let limit = 10;
  while (n >= limit && count < 5) {
    count += 1;
    limit *= 10;
  }
  return count;
}

module.exports = { digits };
