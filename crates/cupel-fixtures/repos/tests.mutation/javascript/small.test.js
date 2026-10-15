// A small order: no discount; never the boundary of exactly 100.
const { discount } = require("./discount");

test("keeps a small order", () => {
  expect(discount(50, true)).toBe(50);
});
