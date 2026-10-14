export function double(n: number): number {
  return n * 2;
}

if (import.meta.vitest) {
  const { it, expect } = import.meta.vitest;

  it("doubles", () => {
    expect(double(2)).toBe(4);
  });
}
