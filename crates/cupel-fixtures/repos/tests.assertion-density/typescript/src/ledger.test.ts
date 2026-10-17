// Dense tests: each pins every outcome it exercises.
import { balance, overdrawn } from "./ledger";

test("deposits and withdrawals add up", () => {
  expect(balance([])).toBe(0);
  expect(balance([5])).toBe(5);
  expect(balance([5, -3])).toBe(2);
  expect(balance([-3, -4])).toBe(-7);
});

test("a negative balance is overdrawn", () => {
  expect(overdrawn([-1])).toBe(true);
  expect(overdrawn([5, -6])).toBe(true);
  expect(overdrawn([])).toBe(false);
  expect(overdrawn([5, -5])).toBe(false);
});

test("order does not change the balance", () => {
  expect(balance([1, 2, 3])).toBe(6);
  expect(balance([3, 2, 1])).toBe(6);
  expect(balance([2, -2, 1])).toBe(1);
  expect(balance([-2, 1, 2])).toBe(1);
});
