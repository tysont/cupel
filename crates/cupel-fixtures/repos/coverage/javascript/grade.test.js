const { classify, total } = require("./grade");

test("totals a list", () => {
  expect(total([1, 2, 3])).toBe(6);
});

test("classifies passing scores", () => {
  expect(classify(95)).toBe("A");
  expect(classify(60)).toBe("pass");
});
