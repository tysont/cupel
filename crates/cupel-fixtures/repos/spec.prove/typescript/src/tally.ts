/** The sum of values, as proofs/tally.dfy proves `Total` computes it. */
export function total(values: number[]): number {
  return values.reduce((sum, value) => sum + value, 0);
}
