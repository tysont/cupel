import { describe, expect, jest, test } from "@jest/globals";
import { discounted, tally } from "../pricing";

describe("discounted", () => {
  test("members pay less", () => {
    expect(discounted(100, true)).toStrictEqual(85);
  });

  test("tallies stay within bounds", () => {
    const spy = jest.spyOn(console, "log");
    expect(tally([1, -3]).overdrawn).toBeTruthy();
    expect(spy).toHaveBeenCalled();
  });
});
