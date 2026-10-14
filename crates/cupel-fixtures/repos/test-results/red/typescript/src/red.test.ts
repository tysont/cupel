// The red-baseline overlay: one failing test.
import { total } from "./tally";

test("reports a wrong total", () => {
  console.log("totalling 2 and 2");
  expect(total([2, 2])).toBe(5);
});
