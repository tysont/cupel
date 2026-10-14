// Tally: a passing test suite for Cupel's runner producers.
export function total(values: number[]): number {
  return values.reduce((sum, value) => sum + value, 0);
}
