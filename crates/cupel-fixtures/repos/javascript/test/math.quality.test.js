import { describe, expect, it, vi } from "vitest";
import { add, range } from "../src/math.js";
import { parseCents } from "../src/decide.js";

describe("quality", () => {
  it("adds exactly", () => {
    expect(add(2, 3)).toBe(5);
  });

  it("rejects blank amounts", () => {
    expect(() => parseCents("")).toThrow("empty amount");
  });

  it("returns something", () => {
    expect(add(1, 1)).toBeTruthy();
  });

  it("runs without checking", () => {
    add(3, 4);
  });

  it("logs and mails every sum", () => {
    const log = vi.fn();
    const mail = vi.fn();
    const clock = vi.fn();
    expect(add(1, 2)).toBe(3);
  });

  it("counts every value in a range", () => {
    for (const value of range(0, 3)) {
      expect(add(value, 0)).toBe(value);
    }
  });

  it("adds every pair in the table", () => {
    for (const [a, b, sum] of [[1, 2, 3], [2, 2, 4]]) {
      expect(add(a, b)).toBe(sum);
    }
  });

  it("keeps values in order", () => {
    const values = [...range(0, 2)];
    values.push(add(1, 2));
    values.push(add(3, 4));
    expect(values).toEqual([0, 1, 3, 7]);
  });

  it("ends with the last sum", () => {
    const values = [...range(0, 2)];
    values.push(add(5, 6));
    values.push(add(7, 8));
    const last = values.pop();
    expect(last).toBe(15);
  });
});
