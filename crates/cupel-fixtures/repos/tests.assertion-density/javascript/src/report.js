// Reports drawn from a ledger.
import { balance } from "./ledger.js";

/** One line naming `owner` and the balance of `entries`. */
export function statement(owner, entries) {
  return `${owner}: ${balance(entries)}`;
}

/** The largest of `entries`, or undefined. */
export function largest(entries) {
  return entries.length === 0 ? undefined : Math.max(...entries);
}
