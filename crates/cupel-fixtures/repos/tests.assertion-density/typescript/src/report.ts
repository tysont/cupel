// Reports drawn from a ledger.
import { balance } from "./ledger";

/** One line naming `owner` and the balance of `entries`. */
export function statement(owner: string, entries: number[]): string {
  return `${owner}: ${balance(entries)}`;
}

/** The largest of `entries`, or undefined. */
export function largest(entries: number[]): number | undefined {
  return entries.length === 0 ? undefined : Math.max(...entries);
}
