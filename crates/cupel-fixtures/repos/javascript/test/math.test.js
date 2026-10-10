import { describe, expect, it } from "vitest";
import { add } from "../src/math.js";

describe("add", () => {
  it("adds two numbers", () => {
    expect(add(1, 2)).toBe(3);
  });
});
