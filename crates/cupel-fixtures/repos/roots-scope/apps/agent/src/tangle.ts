// ABOUTME: Outside every configured root: `classify` has cyclomatic complexity 15, `tally` is cloned in
// ABOUTME: copy.ts, and the two files import each other, none of which a run reports or counts (ADR-177).
import { describe } from "./copy";

export function classify(code: number, retry: boolean, quiet: boolean): string {
  let label = "unknown";
  if (code === 0) {
    label = "ok";
  } else if (code === 1 && retry) {
    label = "retry";
  } else if (code === 2 || quiet) {
    label = "quiet";
  } else if (code === 3) {
    label = "moved";
  } else if (code === 4 && !quiet) {
    label = "missing";
  } else if (code === 5) {
    label = "denied";
  } else if (code === 6 || retry) {
    label = "busy";
  } else if (code === 7) {
    label = "gone";
  } else if (code > 100 && code < 200) {
    label = "range";
  }
  return describe(label);
}

export function tally(items: number[], limit: number): number {
  let sum = 0;
  for (const item of items) {
    if (item > limit) {
      sum += item * 2;
    } else if (item < 0) {
      sum -= item;
    } else {
      sum += item;
    }
  }
  if (sum > 1000) {
    return sum / 2;
  }
  return sum;
}
