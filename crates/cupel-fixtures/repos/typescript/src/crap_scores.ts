// Ledger helpers in each static.crap band when no test covers them: overdue is crap.high
// (cyclomatic 6, CRAP 42), clamp is crap.watch (cyclomatic 3, CRAP 12), and toCents is clean.

// Counts entries more than thirty days late and past the grace period; reversals (negative
// days) and voided entries (zero days) each cancel one.
export function overdue(days: number[], grace: number): number {
  let count = 0;
  for (const day of days) {
    if (day > 30 && day > grace) {
      count += 1;
    } else if (day < 0) {
      count -= 1;
    } else if (day === 0) {
      count -= 1;
    }
  }
  return count;
}

// Keeps an amount between low and high.
export function clamp(amount: number, low: number, high: number): number {
  if (amount < low) {
    return low;
  }
  if (amount > high) {
    return high;
  }
  return amount;
}

// Converts whole units to cents.
export function toCents(units: number): number {
  return units * 100;
}
