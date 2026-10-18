import { join } from "node:path";
import { Counter } from "./counter";

/** The trace file `name` in the directory CUPEL_TRACE_DIR names, when it is set. */
function trace(name: string): string | null {
  const dir = process.env.CUPEL_TRACE_DIR;
  return dir ? join(dir, name) : null;
}

test("counts then finishes", () => {
  const counter = new Counter(trace("counts.jsonl"));
  counter.inc();
  counter.inc();
  counter.finish();
  expect(counter.count).toBe(2);
});

test("bumps", () => {
  const counter = new Counter(trace("bumps.jsonl"));
  counter.inc();
  counter.bump();
  expect(counter.count).toBe(3);
});
