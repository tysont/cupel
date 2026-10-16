import { total } from "./ledger";
import { pad } from "./format";

export function report(entries: number[]): string {
  return pad(String(total(entries)), 10);
}
