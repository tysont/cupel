// A quarter of `cents`, rounded down.
export function quarter(cents: number): number {
  return Math.floor(cents / 4);
}
