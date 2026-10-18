import { appendFileSync } from "node:fs";

/** Counts up and logs each step it takes to a trace file, when it has one. */
export class Counter {
  count = 0;
  done = false;

  constructor(private readonly trace: string | null) {
    this.log(null);
  }

  inc(): void {
    this.count += 1;
    this.log("Inc");
  }

  bump(): void {
    this.count += 2;
    this.log("Inc");
  }

  finish(): void {
    this.done = true;
    this.log("Finish");
  }

  private log(action: string | null): void {
    if (!this.trace) {
      return;
    }
    const step: Record<string, unknown> = { state: { count: this.count, done: this.done } };
    if (action) {
      step.action = action;
    }
    appendFileSync(this.trace, JSON.stringify(step) + "\n");
  }
}
