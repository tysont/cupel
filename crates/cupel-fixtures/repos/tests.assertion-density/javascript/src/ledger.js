// A ledger of signed amounts.

/** The sum of `entries`. */
export function balance(entries) {
  return entries.reduce((total, entry) => total + entry, 0);
}

/** Whether `entries` sum below zero. */
export function overdrawn(entries) {
  return balance(entries) < 0;
}
