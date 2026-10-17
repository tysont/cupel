// Sparse tests: one assertion each, a quarter of the ledger tests' density, and the last one
// sets up a month of entries for its single assertion.
import { largest, statement } from "./report";

test("the statement names the owner", () => {
  expect(statement("ada", [5])).toBe("ada: 5");
});

test("the largest entry is found", () => {
  expect(largest([3, 9, 4])).toBe(9);
});

test("a month of entries ends in one statement", () => {
  const entries: number[] = [];
  entries.push(100);
  entries.push(-30);
  entries.push(100);
  entries.push(-30);
  entries.push(100);
  entries.push(-30);
  entries.push(100);
  entries.push(-30);
  entries.push(100);
  entries.push(-30);
  entries.push(100);
  entries.push(-30);
  entries.push(100);
  entries.push(-30);
  entries.push(100);
  entries.push(-30);
  entries.push(100);
  entries.push(-30);
  entries.push(100);
  entries.push(-30);
  const printed = statement("ada", entries);
  expect(printed).toBe("ada: 700");
});
