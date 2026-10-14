// A balance kept in a module variable, which the tests in this file share.
let balance = 0;

// Passes only while no test before it has deposited: order-dependent.
test("balance starts at zero", () => {
  expect(balance).toBe(0);
});

// Leaves the shared balance changed for every test after it.
test("deposit changes balance", () => {
  balance += 5;
  expect(balance).toBeGreaterThanOrEqual(5);
});
