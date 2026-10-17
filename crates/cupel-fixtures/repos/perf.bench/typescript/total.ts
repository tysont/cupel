// Sums for the perf.bench fixture.
export function total(values: number[]): number {
  return values.reduce((sum, value) => sum + value, 0);
}
