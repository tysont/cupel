import { beforeEach, describe, expect, it, vi } from "vitest";
import { add, range } from "./math";

vi.mock("./legacy");

describe("add", () => {
  it("adds two numbers", () => {
    expect(add(1, 2)).toBe(3);
  });

  let base: number;

  beforeEach(() => {
    base = 1;
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

describe("range", () => {
  it("matches its snapshot", () => {
    expect([...range(0, 3)]).toMatchSnapshot();
  });

  /** A range is consumed whole (REQ-1). */
  it("is consumed by a spy", () => {
    const consume = vi.fn<(values: number[]) => void>();
    consume([...range(0, 2)]);
    expect(consume).toHaveBeenCalledTimes(1);
  });
});
