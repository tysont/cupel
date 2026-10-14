const { total } = require("./tally");

test("totals a list", () => {
  expect(total([1, 2, 3])).toBe(6);
});

test.skip("totals a huge list", () => {
  expect(total(new Array(1000).fill(1))).toBe(1000);
});
