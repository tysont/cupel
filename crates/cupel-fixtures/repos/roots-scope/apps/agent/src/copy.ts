// ABOUTME: Outside every configured root: `total` clones tangle.ts's `tally`, and this file imports
// ABOUTME: tangle.ts, which imports it back, closing an import cycle no run reports (ADR-177).
import { classify } from "./tangle";

export function describe(label: string): string {
  return label.length > 0 ? label : String(classify(0, false, false));
}

export function total(values: number[], ceiling: number): number {
  let sum = 0;
  for (const value of values) {
    if (value > ceiling) {
      sum += value * 2;
    } else if (value < 0) {
      sum -= value;
    } else {
      sum += value;
    }
  }
  if (sum > 1000) {
    return sum / 2;
  }
  return sum;
}
