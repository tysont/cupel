const { summary } = require("../src/report.cjs");

jest.mock("../src/format.cjs", () => ({ money: (cents) => `${cents}` }));

test("joins one line per entry", () => {
  expect(summary([1, 2])).toEqual("1\n2");
});

xtest("formats currency symbols", () => {
  expect(summary([1])).toMatchInlineSnapshot();
});
