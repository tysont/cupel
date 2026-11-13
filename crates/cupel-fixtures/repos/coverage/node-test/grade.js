// Grade: a suite that covers total fully, classify partly, and refund not at all.
function total(values) {
  let sum = 0;
  for (const value of values) {
    sum += value;
  }
  return sum;
}

// A letter for score; the tests never reach the failing grade.
function classify(score) {
  if (score >= 90) {
    return "A";
  }
  if (score >= 50) {
    return "pass";
  }
  return "fail";
}

// What to refund of amount after days; no test calls it.
function refund(amount, days, member) {
  if (amount <= 0) {
    return 0;
  }
  if (days > 30) {
    return 0;
  }
  if (member && days < 7) {
    return amount;
  }
  if (member) {
    return amount / 2;
  }
  return amount / 4;
}

module.exports = { total, classify, refund };
