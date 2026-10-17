import fc from "fast-check";
import { expect, test } from "vitest";
import { add } from "../src/math";

test("sums do not depend on the order", () => {
  fc.assert(
    fc.property(fc.integer(), fc.integer(), (a: number, b: number) => {
      expect(add(a, b)).toBe(add(b, a));
    }),
  );
});
