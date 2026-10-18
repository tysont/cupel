const fs = require("fs");

/** Counts up and logs each step it takes to a trace file, when it has one. */
class Counter {
  constructor(trace) {
    this.count = 0;
    this.done = false;
    this.trace = trace;
    this.log(null);
  }

  inc() {
    this.count += 1;
    this.log("Inc");
  }

  bump() {
    this.count += 2;
    this.log("Inc");
  }

  finish() {
    this.done = true;
    this.log("Finish");
  }

  log(action) {
    if (!this.trace) {
      return;
    }
    const step = { state: { count: this.count, done: this.done } };
    if (action) {
      step.action = action;
    }
    fs.appendFileSync(this.trace, JSON.stringify(step) + "\n");
  }
}

module.exports = { Counter };
