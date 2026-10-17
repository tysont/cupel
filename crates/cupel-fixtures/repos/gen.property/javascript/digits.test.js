const fc = require("fast-check");
const { digits } = require("./digits");

test("digits counts decimal places", () => {
  fc.assert(
    fc.property(fc.integer({ min: 0, max: 109_999 }), (n) => {
      expect(digits(n)).toBe(String(n).length);
    }),
  );
});
