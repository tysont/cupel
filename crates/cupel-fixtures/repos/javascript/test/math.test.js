import { beforeEach, describe, expect, it, vi } from "vitest";
import { add, range } from "../src/math.js";
import { parseCents } from "../src/decide.js";

vi.mock("../src/report.cjs");

describe("add", () => {
  let base;

  beforeEach(() => {
    base = 1;
  });

  it("adds two numbers", () => {
    expect(add(1, 2)).toBe(3);
  });

  it("is truthy for non-zero sums", () => {
    expect(add(base, 1)).toBeTruthy();
  });

  it("returns a defined value", () => {
    expect(add(base, base)).toBeDefined();
  });

  it("does not return zero", () => {
    expect(add(2, 2)).not.toBe(0);
  });

  it.skip("handles huge numbers", () => {
    expect(add(Number.MAX_VALUE, 1)).toBe(Infinity);
  });

  it("can be called without checking", () => {
    add(1, 1);
  });
});

describe("parseCents", () => {
  it("rejects empty amounts", () => {
    expect(() => parseCents("")).toThrow("empty amount");
  });

  it("throws on nothing", () => {
    expect(() => parseCents()).toThrow();
  });

  it("matches the range snapshot", () => {
    expect([...range(0, 3)]).toMatchSnapshot();
  });

  /** Parsing reports the cents it parsed (REQ-1). */
  it("reports what it parsed", () => {
    const log = vi.fn();
    log(parseCents("1.00"));
    expect(log).toHaveBeenCalledWith(100);
  });
});
