import { clampPercent } from "./percent";

test("clamps above a hundred", () => {
  expect(clampPercent(250)).toBe(100);
});
