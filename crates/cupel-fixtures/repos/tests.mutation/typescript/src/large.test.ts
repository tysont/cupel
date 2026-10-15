// A large member order: the discount applies.
import { discount } from "./discount";

test("discounts a large member order", () => {
  expect(discount(200, true)).toBe(190);
});
