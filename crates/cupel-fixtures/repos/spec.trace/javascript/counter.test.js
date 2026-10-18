const path = require("path");
const { Counter } = require("./counter");

/** The trace file `name` in the directory CUPEL_TRACE_DIR names, when it is set. */
function trace(name) {
  const dir = process.env.CUPEL_TRACE_DIR;
  return dir ? path.join(dir, name) : null;
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
