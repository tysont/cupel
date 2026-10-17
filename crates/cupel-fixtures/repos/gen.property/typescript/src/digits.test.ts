import fc from "fast-check";

import { digits } from "./digits";

test("digits counts decimal places", () => {
  fc.assert(
    fc.property(fc.integer({ min: 0, max: 109_999 }), (n: number) => {
      expect(digits(n)).toBe(String(n).length);
    }),
  );
});
