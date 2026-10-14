const { scale } = require("./scale");

test("scales", () => {
  expect(scale(2, 3)).toBe(6);
});
